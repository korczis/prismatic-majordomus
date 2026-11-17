//! The latest reading of a source, handed over at once.
//!
//! [`CapabilityExecutor::execute`](super::executor::CapabilityExecutor::execute) answers a
//! question: it runs the handler, or returns an answer byte-identical to what the handler
//! would return (`project.cache-is-invisible`). A page a person is looking at asks for
//! something else. It wants what is known *now*, at once, with how old that is, and it
//! wants the answer to catch up behind it. That is a [`read`], and it never passes itself
//! off as an answer: a [`Latest`] always carries when it was read, whether the repository
//! has moved since, and whether a newer reading is on its way.
//!
//! # What a read costs
//!
//! One lookup in this process's memory. The handler, git and the file system are touched
//! only by a worker of this module, off the path of the request:
//!
//! - **The source was read in the repository as it is now.** The reading is returned
//!   immediately. When it is older than [`REVALIDATE`], a worker reads it again behind the
//!   caller, so a source the repository does not determine (health, the mesh, the forge)
//!   stays close to the present without any reader waiting for it.
//! - **The repository has moved since.** A worker reads the source again and the caller
//!   waits for it for at most [`Patience::moved`]. A fast source is therefore current on the
//!   same request; a slow one is returned as it was, marked [`Freshness::Stale`] and
//!   `refreshing`. Every other source this process has been asked for lately is read again
//!   at the same moment, so the next screen a reader opens is ready before it is asked for.
//! - **The source has never been read here.** There is nothing to hand over, so the caller
//!   waits for the first reading, for at most [`Patience::cold`], and is told
//!   [`Freshness::Pending`] past that.
//!
//! One worker reads a source at a time, however many callers ask. The readings to take wait
//! in one queue, drained by at most [`WORKERS`] threads, so a change of the repository that
//! sends every remembered source to be read again starts no more threads than that. A
//! reading a caller is waiting for goes to the front of the queue and the ones nobody is
//! waiting for go to the back, so a reader never waits behind the background. Under the
//! workers the executor's own cache and single flight still apply: a worker whose source
//! did not change is answered from the cache and costs nearly nothing.
//!
//! # Switching it off
//!
//! `MAJORDOMUS_CAPABILITY_CACHE=off` makes every read run its handler and wait for it, as
//! it makes every `execute` do. [`PATIENCE_SWITCH`] set to `wait` keeps the store and makes
//! every read wait for a current reading, which is what a test that changes a repository
//! and then reads a page needs in order to be exact.
//!
//! # A reading, from first to known
//!
//! ```
//! use majordomus_cli::capability::readings::{read, Freshness, Patience};
//! use majordomus_cli::synthetic::SyntheticRepository;
//! use serde_json::json;
//!
//! let repo = SyntheticRepository::small().unwrap();
//! let ctx = repo.context().unwrap();
//!
//! // never read here: the caller waits for the first reading, and it is current
//! let first = read(&ctx, "plan.status", json!({}), Patience::WAIT);
//! assert_eq!(first.freshness, Freshness::Current);
//! assert!(first.value.is_ok() && first.read_at.is_some());
//!
//! // known now: handed over from memory, with the age it has, and no handler runs for it
//! let ran = ctx.executor.handler_invocations();
//! let again = read(&ctx, "plan.status", json!({}), Patience::PAGE);
//! assert_eq!(again.value, first.value);
//! assert!(again.age.is_some() && !again.catching_up());
//! assert_eq!(ctx.executor.handler_invocations(), ran);
//!
//! // a source that cannot answer is a reading that says why, never a missing one
//! let unknown = read(&ctx, "no.such.capability", json!({}), Patience::WAIT);
//! assert!(unknown.value.is_err());
//! assert_eq!(unknown.freshness, Freshness::Current);
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use super::executor::{bypassed, canonical_json};
use super::handler::Context;

/// How old a reading of the repository as it is now may grow before a worker reads its
/// source again behind the caller.
pub const REVALIDATE: Duration = Duration::from_secs(1);

/// How many sources are read at once by this module's workers, all sources together.
pub const WORKERS: usize = 4;

/// How long a source nobody asked for is remembered, and so read again when the repository
/// moves. A page left open asks for its sources every minute, so its sources stay; a screen
/// nobody has looked at for five minutes stops costing anything when the repository changes.
pub const REMEMBERED: Duration = Duration::from_secs(300);

/// The most sources remembered at once; the one asked for longest ago is dropped first.
pub const MAX_SOURCES: usize = 512;

/// The environment variable that sets how long a read waits: a number of milliseconds for
/// a source whose repository has moved, or `wait` to wait for a current reading every time.
pub const PATIENCE_SWITCH: &str = "MAJORDOMUS_READING_PATIENCE_MS";

/// How long a read waits for a worker before it returns what it has.
///
/// ```
/// use majordomus_cli::capability::readings::Patience;
/// use std::time::Duration;
///
/// // a page waits a minute for a first reading and 50 ms for a moved repository
/// assert_eq!(Patience::PAGE.cold, Duration::from_secs(60));
/// assert_eq!(Patience::PAGE.moved, Duration::from_millis(50));
/// // an exact read waits for as long as it takes
/// assert_eq!(Patience::WAIT.moved, Duration::MAX);
/// let brief = Patience { cold: Duration::from_millis(5), moved: Duration::ZERO };
/// assert!(brief.moved < Patience::PAGE.moved);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patience {
    /// For a source that has never been read in this process.
    pub cold: Duration,
    /// For a source read before the repository last moved.
    pub moved: Duration,
}

impl Patience {
    /// Wait for a current reading, however long it takes.
    pub const WAIT: Patience = Patience {
        cold: Duration::MAX,
        moved: Duration::MAX,
    };

    /// What a page is served with: a first reading is waited for up to a minute, because
    /// there is nothing else to show, and a reading the repository has moved past for 50
    /// ms, half of the 100 ms a warm page is allowed (the owner's UI-performance pack).
    pub const PAGE: Patience = Patience {
        cold: Duration::from_secs(60),
        moved: Duration::from_millis(50),
    };

    /// The patience of this process: [`PATIENCE_SWITCH`] when it is set, and otherwise
    /// [`Patience::PAGE`]. The crate's own unit tests wait, so that a test which renders a
    /// screen asserts on a current reading and never on a timing.
    ///
    /// ```
    /// use majordomus_cli::capability::readings::{Patience, PATIENCE_SWITCH};
    ///
    /// // what the switch says when it is set and means something, a page's patience otherwise
    /// let set = std::env::var(PATIENCE_SWITCH).ok();
    /// let expected = Patience::parse(set.as_deref()).unwrap_or(Patience::PAGE);
    /// assert_eq!(Patience::of_process(), expected);
    /// ```
    pub fn of_process() -> Patience {
        let set = std::env::var(PATIENCE_SWITCH).ok();
        match Patience::parse(set.as_deref()) {
            Some(patience) => patience,
            None if cfg!(test) => Patience::WAIT,
            None => Patience::PAGE,
        }
    }

    /// What a value of [`PATIENCE_SWITCH`] means, or `None` when it means nothing: `wait`
    /// is [`Patience::WAIT`], and a number is that many milliseconds for a repository that
    /// has moved, with a first reading waited for no less than a page waits.
    ///
    /// ```
    /// use majordomus_cli::capability::readings::Patience;
    /// assert_eq!(Patience::parse(Some("wait")), Some(Patience::WAIT));
    /// let twenty = Patience::parse(Some(" 20 ")).unwrap();
    /// assert_eq!(twenty.moved.as_millis(), 20);
    /// assert_eq!(twenty.cold, Patience::PAGE.cold, "a first reading is still waited for");
    /// assert_eq!(Patience::parse(Some("soon")), None, "not a number and not `wait`");
    /// assert_eq!(Patience::parse(None), None);
    /// ```
    pub fn parse(value: Option<&str>) -> Option<Patience> {
        let value = value?.trim();
        if value == "wait" {
            return Some(Patience::WAIT);
        }
        let millis: u64 = value.parse().ok()?;
        Some(Patience {
            cold: Patience::PAGE.cold.max(Duration::from_millis(millis)),
            moved: Duration::from_millis(millis),
        })
    }
}

/// How a reading stands against the repository as it is now.
///
/// ```
/// use majordomus_cli::capability::readings::{read, Freshness, Patience};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// use std::time::Duration;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// // a caller with no patience at all is told the first reading is pending, or handed it
/// // when the worker was quicker than the caller: never anything in between
/// let none = Patience { cold: Duration::ZERO, moved: Duration::ZERO };
/// let first = read(&ctx, "plan.status", serde_json::json!({}), none);
/// assert!(matches!(first.freshness, Freshness::Pending | Freshness::Current));
/// // and a caller that waits is handed a current one; nothing here moved the repository,
/// // so nothing is `Freshness::Stale`
/// let waited = read(&ctx, "plan.status", serde_json::json!({}), Patience::WAIT);
/// assert_eq!(waited.freshness, Freshness::Current);
/// assert_ne!(waited.freshness, Freshness::Stale);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Read in the repository as it is now.
    Current,
    /// Read before the repository last moved.
    Stale,
    /// Never read in this process; the first reading has not arrived.
    Pending,
}

/// The latest reading of one source.
///
/// ```
/// use majordomus_cli::capability::readings::{read, Freshness, Latest, Patience};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let latest: Latest = read(&ctx, "plan.status", serde_json::json!({}), Patience::WAIT);
/// // what the source answered, how long that took, when it was read and how it stands
/// assert!(latest.value.is_ok());
/// assert!(latest.read_at.is_some() && latest.age.is_some());
/// assert_eq!(latest.freshness, Freshness::Current);
/// assert!(latest.took >= std::time::Duration::ZERO);
/// ```
#[derive(Debug, Clone)]
pub struct Latest<V = Value> {
    /// What the source answered, or why it could not. `Err` for a pending reading too.
    pub value: Result<V, String>,
    /// How long the executor took to produce this reading.
    pub took: Duration,
    /// When it was read; `None` while pending.
    pub read_at: Option<SystemTime>,
    /// How long ago that was; `None` while pending.
    pub age: Option<Duration>,
    /// How it stands against the repository as it is now.
    pub freshness: Freshness,
    /// Whether a worker is reading the source again at this moment.
    pub refreshing: bool,
}

impl<V> Latest<V> {
    /// A reading taken just now, by the caller itself.
    fn now(value: Result<V, String>, took: Duration) -> Latest<V> {
        Latest {
            value,
            took,
            read_at: Some(SystemTime::now()),
            age: Some(Duration::ZERO),
            freshness: Freshness::Current,
            refreshing: false,
        }
    }

    /// Whether a reader should be told that what they see is about to be replaced: the
    /// reading is not of the repository as it is now.
    ///
    /// ```
    /// use majordomus_cli::capability::readings::{read, Freshness, Patience};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let mut latest = read(&ctx, "plan.status", serde_json::json!({}), Patience::WAIT);
    /// assert!(!latest.catching_up(), "a current reading is not about to be replaced");
    /// latest.freshness = Freshness::Stale;
    /// assert!(latest.catching_up());
    /// latest.freshness = Freshness::Pending;
    /// assert!(latest.catching_up());
    /// ```
    pub fn catching_up(&self) -> bool {
        self.freshness != Freshness::Current
    }
}

/// What the first reading of a source says while it has not arrived.
pub const PENDING: &str = "the first reading of this source has not arrived yet";

/// The state of the repository a reading was taken in: the context's reading of it, the
/// registry built from it, and how many times this process has written to it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct World {
    reading: u64,
    fingerprint: String,
    writes: u64,
    /// The generation the source follows beyond the repository (`crate::change`), zero for
    /// a source that follows none: part of one source's world, never of the repository's.
    followed: u64,
}

impl World {
    fn of(ctx: &Context) -> World {
        World {
            reading: ctx.reading,
            fingerprint: ctx.registry.fingerprint().to_string(),
            writes: ctx.executor.writes(),
            followed: 0,
        }
    }

    /// The world a reading of `id` is taken in: the repository's, and the generation the
    /// source follows when it follows one, so that a reading from before that generation
    /// moved is stale like one from before the repository moved.
    fn of_source(&self, id: &str) -> World {
        World {
            followed: crate::change::generation_of(id).unwrap_or(0),
            ..self.clone()
        }
    }
}

#[derive(Debug)]
struct Outcome {
    /// The answer as the executor's cache shares it: reading a source again whose answer is
    /// cached copies nothing, and a caller copies it only after the slot's lock is released.
    value: Result<Arc<Value>, String>,
    world: World,
    at: Instant,
    wall: SystemTime,
    took: Duration,
}

#[derive(Debug)]
struct State {
    input: Value,
    outcome: Option<Outcome>,
    flying: bool,
    asked: Instant,
}

#[derive(Debug)]
struct Slot {
    state: Mutex<State>,
    done: Condvar,
}

type Key = (String, String);

/// One reading to take: the source, its slot, and the repository it is to be taken in.
struct Job {
    ctx: Context,
    id: String,
    landing: Landing,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// The readings waiting to be taken, and how many workers are taking them.
#[derive(Debug, Default)]
struct Queue {
    jobs: VecDeque<Job>,
    workers: usize,
}

/// The latest readings of one executor.
#[derive(Debug, Default)]
pub(crate) struct Readings {
    slots: Mutex<HashMap<Key, Arc<Slot>>>,
    world: Mutex<Option<World>>,
    queue: Mutex<Queue>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Readings {
    /// How many sources are remembered.
    pub(crate) fn remembered(&self) -> usize {
        lock(&self.slots).len()
    }

    /// The slot of a source, made when it is first asked for. Sources nobody has asked for
    /// in [`REMEMBERED`] are forgotten here, and the oldest past [`MAX_SOURCES`].
    fn slot(&self, id: &str, input: &Value) -> Arc<Slot> {
        let key = (id.to_string(), canonical_json(input));
        let mut slots = lock(&self.slots);
        if let Some(slot) = slots.get(&key) {
            return Arc::clone(slot);
        }
        slots.retain(|_, slot| {
            let state = lock(&slot.state);
            state.flying || state.asked.elapsed() < REMEMBERED
        });
        while slots.len() >= MAX_SOURCES {
            let oldest = slots
                .iter()
                .filter(|(_, slot)| !lock(&slot.state).flying)
                .min_by_key(|(_, slot)| lock(&slot.state).asked)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => slots.remove(&key),
                None => break,
            };
        }
        let slot = Arc::new(Slot {
            state: Mutex::new(State {
                input: input.clone(),
                outcome: None,
                flying: false,
                asked: Instant::now(),
            }),
            done: Condvar::new(),
        });
        slots.insert(key, Arc::clone(&slot));
        slot
    }

    /// Note the world a caller is in. `true` when it is not the one last noted: the
    /// repository has moved, and everything remembered is to be read again.
    fn moved_to(&self, world: &World) -> bool {
        let mut noted = lock(&self.world);
        if noted.as_ref() == Some(world) {
            return false;
        }
        let first = noted.is_none();
        *noted = Some(world.clone());
        !first
    }

    /// Every remembered source that is not being read and was not read in `world`.
    fn behind(&self, world: &World) -> Vec<(String, Arc<Slot>)> {
        lock(&self.slots)
            .iter()
            .filter(|((id, _), slot)| {
                let state = lock(&slot.state);
                !state.flying
                    && state.asked.elapsed() < REMEMBERED
                    && state
                        .outcome
                        .as_ref()
                        .is_some_and(|o| o.world != world.of_source(id))
            })
            .map(|((id, _), slot)| (id.clone(), Arc::clone(slot)))
            .collect()
    }
}

/// The latest reading of `id` asked with `input`: what this process knows now, returned
/// within `patience`, with a worker reading the source again behind the caller whenever
/// what is known is not current. See the module for what each case costs.
///
/// ```
/// use majordomus_cli::capability::readings::{read, Patience};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// use serde_json::json;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let reading = read(&ctx, "plan.status", json!({}), Patience::WAIT);
/// // a reading holds what `execute` answers: it is the same source, asked the same way
/// assert_eq!(reading.value.ok(), ctx.execute("plan.status", json!({})).ok());
/// // one source per input: asked again it is remembered, not read a second time
/// read(&ctx, "plan.status", json!({}), Patience::PAGE);
/// assert_eq!(ctx.executor.readings_remembered(), 1);
/// ```
pub fn read(ctx: &Context, id: &str, input: Value, patience: Patience) -> Latest {
    let latest = read_shared(ctx, id, input, patience);
    Latest {
        value: latest.value.map(Arc::unwrap_or_clone),
        took: latest.took,
        read_at: latest.read_at,
        age: latest.age,
        freshness: latest.freshness,
        refreshing: latest.refreshing,
    }
}

/// The same reading, with the answer as the slot shares it rather than a copy of it: what a
/// screen that only reads the answer asks, so that a large one is copied for nobody.
pub(crate) fn read_shared(
    ctx: &Context,
    id: &str,
    input: Value,
    patience: Patience,
) -> Latest<Arc<Value>> {
    if bypassed() {
        let started = Instant::now();
        let value = ctx.execute(id, input).map_err(|e| e.to_string());
        return Latest::now(value.map(Arc::new), started.elapsed());
    }
    let readings = &ctx.executor.readings;
    let repository = World::of(ctx);
    let moved = readings.moved_to(&repository);
    // this source's world: the repository's, and the generation it follows if it follows one
    let world = repository.of_source(id);
    let slot = readings.slot(id, &input);

    let wait = {
        let mut state = lock(&slot.state);
        state.asked = Instant::now();
        // the executor may already hold the answer for this state of the repository: a source
        // whose cache is keyed by what did not change, or one another caller just asked. Then
        // it is current at once, and no worker is needed for it.
        let behind = state.outcome.as_ref().is_none_or(|o| o.world != world);
        if behind {
            if let Some(value) = ctx.executor.cached_shared(ctx, id, &state.input) {
                state.outcome = Some(Outcome {
                    value: Ok(value),
                    world: world.clone(),
                    at: Instant::now(),
                    wall: SystemTime::now(),
                    took: Duration::ZERO,
                });
            }
        }
        let (due, wait) = match &state.outcome {
            None => (true, patience.cold),
            Some(o) if o.world != world => (true, patience.moved),
            Some(o) => (o.at.elapsed() > REVALIDATE, Duration::ZERO),
        };
        // a caller that will wait for the reading goes first; one handed the reading it has
        // does not, and neither does the background
        let urgent = !wait.is_zero();
        if due && !state.flying {
            state.flying = true;
            drop(state);
            work(ctx, id, Arc::clone(&slot), world.clone(), urgent);
        } else if due && urgent {
            // already queued, behind readings nobody waits for: this caller moves it forward
            drop(state);
            hurry(readings, &slot);
        }
        wait
    };
    if moved {
        for (other, behind) in readings.behind(&repository) {
            let mut state = lock(&behind.state);
            if state.flying {
                continue;
            }
            state.flying = true;
            drop(state);
            let theirs = repository.of_source(&other);
            work(ctx, &other, behind, theirs, false);
        }
    }

    let deadline = Instant::now().checked_add(wait);
    let mut state = lock(&slot.state);
    loop {
        let current = state.outcome.as_ref().is_some_and(|o| o.world == world);
        if current || !state.flying || wait.is_zero() {
            break;
        }
        state = match deadline {
            // a wait too long to be a moment in time is a wait without an end
            None => slot.done.wait(state).unwrap_or_else(|e| e.into_inner()),
            Some(deadline) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                slot.done
                    .wait_timeout(state, left)
                    .unwrap_or_else(|e| e.into_inner())
                    .0
            }
        };
    }
    let refreshing = state.flying;
    let (value, took, read_at, age, freshness) = match &state.outcome {
        None => (
            Err(PENDING.to_string()),
            Duration::ZERO,
            None,
            None,
            Freshness::Pending,
        ),
        Some(o) => (
            o.value.clone(),
            o.took,
            Some(o.wall),
            Some(o.at.elapsed()),
            if o.world == world {
                Freshness::Current
            } else {
                Freshness::Stale
            },
        ),
    };
    // a caller's own copy, when it wants one, is made once the slot is free again, so that a
    // large answer copied for one page holds up neither the worker landing a reading nor
    // another page
    drop(state);
    Latest {
        value,
        took,
        read_at,
        age,
        freshness,
        refreshing,
    }
}

/// Queue a reading of the source of `slot`, to be filed under `world`, and make sure a
/// worker will take it. The slot is already marked as being read; whatever happens to the
/// job, the mark is cleared ([`Landing`]), so a handler that panics leaves a reading that
/// says so and never a source nobody reads.
fn work(ctx: &Context, id: &str, slot: Arc<Slot>, world: World, urgent: bool) {
    let job = Job {
        ctx: ctx.clone(),
        id: id.to_string(),
        landing: Landing {
            slot,
            world,
            landed: false,
        },
    };
    let readings = &ctx.executor.readings;
    let start = {
        let mut queue = lock(&readings.queue);
        if urgent {
            queue.jobs.push_front(job);
        } else {
            queue.jobs.push_back(job);
        }
        let start = queue.workers < WORKERS;
        if start {
            queue.workers += 1;
        }
        start
    };
    if !start {
        return;
    }
    let executor = Arc::clone(&ctx.executor);
    let drain = move || {
        let readings = &executor.readings;
        loop {
            let job = {
                let mut queue = lock(&readings.queue);
                match queue.jobs.pop_front() {
                    Some(job) => job,
                    None => {
                        queue.workers -= 1;
                        return;
                    }
                }
            };
            take(job);
        }
    };
    // a process that cannot start a thread takes the reading on the caller's, which is what
    // it did before this module existed
    if let Err(refused) = std::thread::Builder::new()
        .name("mj-reading".into())
        .spawn(drain)
    {
        tracing::warn!("no thread for a reading of a source: {refused}");
        let job = {
            let mut queue = lock(&readings.queue);
            queue.workers -= 1;
            queue.jobs.pop_front()
        };
        if let Some(job) = job {
            take(job);
        }
    }
}

/// Move the queued reading of `slot` to the front of the queue: a caller is now waiting for
/// it. A reading a worker has already taken is no longer queued, and is left as it is.
fn hurry(readings: &Readings, slot: &Arc<Slot>) {
    let mut queue = lock(&readings.queue);
    if let Some(at) = queue
        .jobs
        .iter()
        .position(|job| Arc::ptr_eq(&job.landing.slot, slot))
    {
        if let Some(job) = queue.jobs.remove(at) {
            queue.jobs.push_front(job);
        }
    }
}

/// Take one reading: ask the executor, and land what it answered in the slot. A panic of
/// the handler is caught here, so the worker goes on to the next job.
fn take(job: Job) {
    let Job {
        ctx,
        id,
        mut landing,
    } = job;
    let input = lock(&landing.slot.state).input.clone();
    let started = Instant::now();
    let asked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.executor
            .execute_shared(&ctx, &id, input)
            .map_err(|e| e.to_string())
    }));
    let value = asked.unwrap_or_else(|_| Err("the source panicked while answering".to_string()));
    landing.land(value, started.elapsed());
}

/// What a worker owes its slot: an outcome, and the mark cleared.
struct Landing {
    slot: Arc<Slot>,
    world: World,
    landed: bool,
}

impl Landing {
    fn land(&mut self, value: Result<Arc<Value>, String>, took: Duration) {
        let mut state = lock(&self.slot.state);
        state.outcome = Some(Outcome {
            value,
            world: self.world.clone(),
            at: Instant::now(),
            wall: SystemTime::now(),
            took,
        });
        state.flying = false;
        self.landed = true;
        drop(state);
        self.slot.done.notify_all();
    }
}

impl Drop for Landing {
    fn drop(&mut self) {
        if !self.landed {
            self.land(
                Err("the source panicked while answering".into()),
                Duration::ZERO,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A source that follows a change generation of its own (the blackboard, ADR 0176) is
    /// current until that generation moves, is then handed over as stale and read again, and
    /// a caller that waits is given the reading of the new generation. A source that follows
    /// no such generation is not disturbed by it.
    #[test]
    fn a_followed_source_goes_stale_when_its_generation_moves() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let input = json!({ "signals": 0 });
        let first = read(&ctx, "blackboard.snapshot", input.clone(), Patience::WAIT);
        assert_eq!(first.freshness, Freshness::Current);
        let plan = read(&ctx, "plan.status", json!({}), Patience::WAIT);
        assert_eq!(plan.freshness, Freshness::Current);

        crate::change::BLACKBOARD.advance();
        let behind = read(&ctx, "blackboard.snapshot", input.clone(), Patience::PAGE);
        assert!(
            behind.freshness == Freshness::Stale || behind.freshness == Freshness::Current,
            "{:?}",
            behind.freshness
        );
        if behind.freshness == Freshness::Stale {
            assert!(behind.refreshing, "a stale reading is being read again");
            assert!(
                behind.read_at.is_some() && behind.age.is_some(),
                "it keeps its age"
            );
        } else {
            // current only as a new reading, never as the one from before the change
            assert_ne!(
                behind.read_at, first.read_at,
                "the reading before the change"
            );
        }
        // Other tests of this process announce peers and append to the journal, and every such
        // move advances the same shared generation. A wait that one of them overtakes is handed
        // a reading already one move behind, so the caller waits again, as a reader of a busy
        // blackboard would; alone, the first wait is current (2026-10-21, 3524 tests in parallel).
        let mut caught = read(&ctx, "blackboard.snapshot", input.clone(), Patience::WAIT);
        for _ in 0..50 {
            if caught.freshness == Freshness::Current {
                break;
            }
            caught = read(&ctx, "blackboard.snapshot", input.clone(), Patience::WAIT);
        }
        assert_eq!(caught.freshness, Freshness::Current);
        let plan = read(&ctx, "plan.status", json!({}), Patience::PAGE);
        assert_eq!(
            plan.freshness,
            Freshness::Current,
            "an unrelated source stays current"
        );
        assert_eq!(crate::change::generation_of("plan.status"), None);
        assert!(crate::change::generation_of("blackboard.snapshot").is_some());
    }
}
