//! Where executions live while this process does: the snapshots, the retained event
//! history, and the subscribers.
//!
//! Everything is bounded. A store that grew with the work it observed would be this
//! executable's own memory leak, so the number of retained executions, the number of
//! events kept per execution and the depth of a subscriber's queue all have a limit, and
//! each limit has a defined behaviour when it is reached rather than an unwritten one.
//!
//! State moves in one place. [`ExecutionStore::publish`] applies an event to the snapshot
//! under the lock, refusing a transition [`ExecutionState::may_move_to`] does not allow,
//! assigns the sequence, retains the event and hands it to the subscribers — so a client
//! reading the snapshot and a client reading the stream can never disagree about what
//! happened, only about how much of it they have seen.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::event::{EventPayload, ExecutionEvent};
use super::model::{
    Actor, Execution, ExecutionError, ExecutionId, ExecutionState, IdempotencyKey, ProgressView,
    RepositoryRef, StepState, StepView,
};

/// What the store keeps, and how much of it.
///
/// The defaults are for a development server a person is watching: enough history that a
/// browser reload during a long run loses nothing, and little enough that a process left
/// running for a week is not holding a transcript of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many executions are remembered. The oldest finished one is forgotten first; an
    /// execution that is still running is never forgotten.
    pub max_executions: usize,
    /// How many events are retained per execution. The oldest are dropped and the
    /// snapshot says so, so a client that asks for history knows it did not get all of it.
    pub max_events: usize,
    /// The longest a single log line may be; longer is cut with a marker.
    pub max_log_chars: usize,
    /// How many events a subscriber may be behind before it is told to resynchronise
    /// instead of being waited for.
    pub max_subscriber_queue: usize,
    /// How many executions may be running at once. Beyond it, an execution waits queued.
    pub max_running: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_executions: 200,
            max_events: 2_000,
            max_log_chars: 2_000,
            max_subscriber_queue: 512,
            max_running: 4,
        }
    }
}

/// What a subscriber receives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// One event, in order.
    Event(Arc<ExecutionEvent>),
    /// The subscriber fell behind and events were dropped rather than buffered. It must
    /// read the snapshot and the history again from the sequence it last saw.
    Lagged {
        /// How many events were dropped.
        dropped: u64,
    },
}

/// A page of an execution's retained history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPage {
    /// The events, oldest first.
    pub events: Vec<Arc<ExecutionEvent>>,
    /// The sequence of the last event in this page; the cursor for the next.
    pub last_sequence: u64,
    /// Whether more events follow this page right now.
    pub more: bool,
    /// Whether events before this page were dropped by the bound.
    pub truncated: bool,
}

/// Which executions a subscriber wants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// Every execution this process runs.
    All,
    /// Only these.
    Only(Vec<ExecutionId>),
}

impl Filter {
    /// Does this filter want events about `id`?
    pub fn wants(&self, id: &ExecutionId) -> bool {
        match self {
            Filter::All => true,
            Filter::Only(ids) => ids.contains(id),
        }
    }
}

/// The outcome of asking to cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The token is set and a cancelling event was published.
    Requested,
    /// It had already been asked for.
    AlreadyRequested,
    /// It had already finished; nothing was done.
    AlreadyFinished(ExecutionState),
    /// No such execution.
    Unknown,
}

/// What an execution is when it is created: everything about it that does not move.
///
/// ```
/// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
/// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
/// let store = ExecutionStore::new(Limits::default());
/// let seed = || Seed {
///     capability: "executions.demonstrate",
///     title: "Demonstrate an execution",
///     input: serde_json::json!({ "steps": 1 }),
///     cancellable: true,
///     actor: Actor::reported(4242),
///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
/// };
/// let id = ExecutionId::fresh();
/// store.adopt(id.clone(), seed(), 4242).unwrap();
/// let held = store.get(&id).unwrap();
/// assert_eq!(held.capability, "executions.demonstrate");
/// assert_eq!((held.state, held.actor.pid), (ExecutionState::Queued, Some(4242)));
/// ```
#[derive(Debug, Clone)]
pub struct Seed<'a> {
    /// The capability's canonical id.
    pub capability: &'a str,
    /// Its title.
    pub title: &'a str,
    /// The input, with every sensitive value already replaced.
    pub input: Value,
    /// Whether asking it to stop will do anything.
    pub cancellable: bool,
    /// Who asked for it.
    pub actor: Actor,
    /// Which repository it runs against.
    pub repository: RepositoryRef,
}

struct Subscriber {
    id: u64,
    filter: Filter,
    tx: std::sync::mpsc::SyncSender<Delivery>,
    dropped: u64,
}

/// How often a process that runs an execution and reports it to a server says so, at the
/// least, while nothing else happens: the reporter's heartbeat.
pub const REPORT_HEARTBEAT: Duration = Duration::from_secs(2);

/// How long a reported execution may go without a report before this store stops believing
/// it is still going: fifteen heartbeats. A `majordomus run` that was killed sends nothing
/// more, and an execution shown as running for ever would be a history no writer reaches.
pub const REPORT_SILENCE: Duration = Duration::from_secs(30);

/// The outcome of one report from the process that runs an execution.
///
/// ```
/// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
/// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
/// let store = ExecutionStore::new(Limits::default());
/// let seed = || Seed {
///     capability: "executions.demonstrate",
///     title: "Demonstrate an execution",
///     input: serde_json::json!({ "steps": 1 }),
///     cancellable: true,
///     actor: Actor::reported(4242),
///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
/// };
/// let id = ExecutionId::fresh();
/// store.adopt(id.clone(), seed(), 4242).unwrap();
/// // the reporter's own sequence: 1 is its `created`, already held here
/// use majordomus_cli::execution::ReportOutcome;
/// let outcome: ReportOutcome = store.report(&id, vec![(2, EventPayload::Started)]).unwrap();
/// assert_eq!((outcome.applied, outcome.through, outcome.cancel_requested), (1, 2, false));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOutcome {
    /// How many of the reported events were applied here.
    pub applied: usize,
    /// The reporter's sequence this store holds everything up to: where its next report
    /// starts. Behind what was sent when the report had a gap in it.
    pub through: u64,
    /// Whether somebody asked this process to stop the execution. Only the process that
    /// runs it can, so the reporter is told and cancels its own.
    pub cancel_requested: bool,
}

/// Why an execution was not adopted from a report.
///
/// ```
/// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
/// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
/// let store = ExecutionStore::new(Limits::default());
/// let seed = || Seed {
///     capability: "executions.demonstrate",
///     title: "Demonstrate an execution",
///     input: serde_json::json!({ "steps": 1 }),
///     cancellable: true,
///     actor: Actor::reported(4242),
///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
/// };
/// use majordomus_cli::execution::{ActorKind, AdoptError};
/// let own = ExecutionId::fresh();
/// let mut mine = seed();
/// mine.actor = Actor::of(ActorKind::Http);
/// store.create(
///     own.clone(), mine.capability, mine.title, mine.input, mine.cancellable, mine.actor,
///     mine.repository,
/// );
/// assert_eq!(store.adopt(own.clone(), seed(), 4242), Err(AdoptError::Own(own)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdoptError {
    /// The id names an execution this process runs itself; a report cannot write into it.
    #[error("{0} is an execution this process runs itself, not one another process reports")]
    Own(ExecutionId),
}

/// What a store keeps about an execution another process runs and reports here.
struct Reported {
    /// The reporter's sequence of the last event taken: where its next report resumes.
    through: u64,
    /// When it last reported.
    heard: Instant,
    /// The process that runs it.
    pid: u32,
}

struct Record {
    snapshot: Execution,
    events: VecDeque<Arc<ExecutionEvent>>,
    /// The sequence of the oldest retained event; 1 while nothing has been dropped.
    first_retained: u64,
    cancel: Arc<AtomicBool>,
    cancel_requested: bool,
    started: Option<Instant>,
    /// Present when the work runs in another process that reports it here.
    reported: Option<Reported>,
    /// The digest of the input a keyed start was sent with: what a repeated start with the
    /// same key must match to be answered this execution.
    digest: Option<String>,
}

/// What an idempotency key is unique within: one repository's one capability. The same key
/// sent to two capabilities, or by two repositories to one process, names two requests.
type KeyScope = (String, String, IdempotencyKey);

/// The scope a keyed execution's key is held under; `None` for one started without a key.
fn scope_of(snapshot: &Execution) -> Option<KeyScope> {
    let key = IdempotencyKey::parse(snapshot.idempotency_key.as_deref()?).ok()?;
    Some((
        snapshot.repository.id.clone(),
        snapshot.capability.clone(),
        key,
    ))
}

struct Inner {
    records: BTreeMap<ExecutionId, Record>,
    order: VecDeque<ExecutionId>,
    subscribers: Vec<Subscriber>,
    /// Which execution each idempotency key started, for as long as that execution is held:
    /// the entry goes when [`ExecutionStore::evict`] forgets the record, and with it the
    /// only thing a repeated start could be answered with.
    keys: HashMap<KeyScope, ExecutionId>,
}

/// What a keyed start is held to: the key the client sent, and the digest of the input it
/// sent with it (the engine takes it over the capability's input in canonical JSON, so two
/// inputs equal as data are one input whatever order a client wrote their keys in).
///
/// The digest never leaves the store: it is not in the snapshot, the events or any answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idempotency {
    /// The key.
    pub key: IdempotencyKey,
    /// The digest of the input the key was sent with.
    pub digest: String,
}

/// What [`ExecutionStore::create_once`] did.
#[derive(Debug, Clone)]
pub enum Admission {
    /// A new execution was recorded; this is its cancellation token.
    Created(Arc<AtomicBool>),
    /// The key had already started this execution with the same input: nothing new was
    /// recorded, and this is the execution's snapshot as it stands.
    Existing(Box<Execution>),
}

/// A key sent again with a different input: refused, because answering the first execution
/// would tell the client its second request ran when it did not, and starting a second one
/// would make the key mean nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the idempotency key '{key}' already started {existing} of {capability} with a different \
     input; send the same input to be answered {existing}, or a new key to start another \
     execution"
)]
pub struct KeyConflict {
    /// The key.
    pub key: String,
    /// The capability it was sent to.
    pub capability: String,
    /// The execution the key already started.
    pub existing: ExecutionId,
}

/// The executions of one process.
pub struct ExecutionStore {
    limits: Limits,
    inner: Mutex<Inner>,
    next_subscriber: AtomicU64,
}

impl ExecutionStore {
    /// A store with these limits.
    pub fn new(limits: Limits) -> Self {
        ExecutionStore {
            limits,
            inner: Mutex::new(Inner {
                records: BTreeMap::new(),
                order: VecDeque::new(),
                subscribers: Vec::new(),
                keys: HashMap::new(),
            }),
            next_subscriber: AtomicU64::new(1),
        }
    }

    /// The limits this store was built with.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // a poisoned lock means a thread panicked while holding it. The snapshot it was
        // editing is one execution's, the rest of the store is intact, and refusing every
        // further execution because one handler panicked would turn a contained failure
        // into an outage.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record a new execution and publish its first event. Returns its cancellation token.
    ///
    /// The execution is created queued: what runs it decides when it starts.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        id: ExecutionId,
        capability: &str,
        title: &str,
        input: Value,
        cancellable: bool,
        actor: Actor,
        repository: RepositoryRef,
    ) -> Arc<AtomicBool> {
        let (record, created) = self.record(
            &id,
            Seed {
                capability,
                title,
                input,
                cancellable,
                actor,
                repository,
            },
            None,
            None,
        );
        let cancel = Arc::clone(&record.cancel);
        self.hold(&mut self.lock(), id.clone(), record);
        self.publish(&id, created);
        cancel
    }

    /// Record a new execution unless its idempotency key already started one (I4000).
    ///
    /// Without a key this is [`Self::create`]. With one, the key is looked up and the
    /// record inserted under one hold of the lock, so two starts sent at once with one key
    /// record one execution, never two: the second is answered the first's snapshot when
    /// its input digest is the same, and refused with [`KeyConflict`] when it is not. A key
    /// is held for as long as the execution it started is: once the bound forgets a
    /// finished execution, its key starts a new one.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, ActorKind, Admission, ExecutionId, ExecutionStore};
    /// use majordomus_cli::execution::{Idempotency, IdempotencyKey, Limits, RepositoryRef, Seed};
    /// let store = ExecutionStore::new(Limits::default());
    /// let seed = || Seed {
    ///     capability: "executions.demonstrate",
    ///     title: "Demonstrate an execution",
    ///     input: serde_json::json!({ "steps": 1 }),
    ///     cancellable: true,
    ///     actor: Actor::of(ActorKind::Http),
    ///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
    /// };
    /// let keyed = |digest: &str| Some(Idempotency {
    ///     key: IdempotencyKey::parse("click-1").unwrap(),
    ///     digest: digest.into(),
    /// });
    /// let first = ExecutionId::fresh();
    /// assert!(matches!(store.create_once(first.clone(), seed(), keyed("a")), Ok(Admission::Created(_))));
    /// match store.create_once(ExecutionId::fresh(), seed(), keyed("a")).unwrap() {
    ///     Admission::Existing(e) => assert_eq!(e.id, first),
    ///     Admission::Created(_) => panic!("a repeated start started a second execution"),
    /// }
    /// let refused = store.create_once(ExecutionId::fresh(), seed(), keyed("b")).unwrap_err();
    /// assert_eq!(refused.existing, first);
    /// assert_eq!(store.len(), 1);
    /// ```
    pub fn create_once(
        &self,
        id: ExecutionId,
        seed: Seed<'_>,
        idempotency: Option<Idempotency>,
    ) -> Result<Admission, KeyConflict> {
        let (record, created) = self.record(&id, seed, None, idempotency.as_ref());
        let cancel = Arc::clone(&record.cancel);
        {
            let mut inner = self.lock();
            if let Some(scope) = scope_of(&record.snapshot) {
                let held = inner
                    .keys
                    .get(&scope)
                    .and_then(|existing| inner.records.get(existing));
                if let Some(held) = held {
                    return if held.digest == record.digest {
                        Ok(Admission::Existing(Box::new(held.snapshot.clone())))
                    } else {
                        Err(KeyConflict {
                            key: scope.2.to_string(),
                            capability: scope.1,
                            existing: held.snapshot.id.clone(),
                        })
                    };
                }
                inner.keys.insert(scope, id.clone());
            }
            self.hold(&mut inner, id.clone(), record);
        }
        self.publish(&id, created);
        Ok(Admission::Created(cancel))
    }

    /// Hold an execution another process runs and reports here, so that this process's
    /// listing, snapshot, history and live channel show it as they show their own.
    ///
    /// Nothing runs here: what the record says is what the reporter says, through
    /// [`Self::report`]. Adopting an id already held as a reported execution changes
    /// nothing, so a reporter may send its description with every report. An id this
    /// process runs itself is refused.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
    /// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
    /// let store = ExecutionStore::new(Limits::default());
    /// let seed = || Seed {
    ///     capability: "executions.demonstrate",
    ///     title: "Demonstrate an execution",
    ///     input: serde_json::json!({ "steps": 1 }),
    ///     cancellable: true,
    ///     actor: Actor::reported(4242),
    ///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
    /// };
    /// let id = ExecutionId::fresh();
    /// store.adopt(id.clone(), seed(), 4242).unwrap();
    /// store.adopt(id.clone(), seed(), 4242).unwrap(); // again: nothing changes
    /// assert!(store.is_reported(&id));
    /// assert_eq!(store.get(&id).unwrap().event_count, 1, "one `created` event");
    /// ```
    pub fn adopt(&self, id: ExecutionId, seed: Seed<'_>, pid: u32) -> Result<(), AdoptError> {
        {
            let inner = self.lock();
            if let Some(record) = inner.records.get(&id) {
                return match record.reported {
                    Some(_) => Ok(()),
                    None => Err(AdoptError::Own(id)),
                };
            }
        }
        let (record, created) = self.record(
            &id,
            seed,
            Some(Reported {
                // the reporter's first event is its own `created`, which this store has
                // just published for itself
                through: 1,
                heard: Instant::now(),
                pid,
            }),
            None,
        );
        self.hold(&mut self.lock(), id.clone(), record);
        self.publish(&id, created);
        Ok(())
    }

    /// Apply what the process running an execution reports, in its own sequence: each
    /// event at most once and in order, so a report sent twice or cut short is harmless.
    ///
    /// An event after a gap is not applied; the answer's `through` says where the reporter
    /// is to resume. An event this store's state machine refuses is passed over rather than
    /// believed, as one from a handler of this process would be. `None` when the id is not
    /// a reported execution of this store.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
    /// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
    /// let store = ExecutionStore::new(Limits::default());
    /// let seed = || Seed {
    ///     capability: "executions.demonstrate",
    ///     title: "Demonstrate an execution",
    ///     input: serde_json::json!({ "steps": 1 }),
    ///     cancellable: true,
    ///     actor: Actor::reported(4242),
    ///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
    /// };
    /// let id = ExecutionId::fresh();
    /// store.adopt(id.clone(), seed(), 4242).unwrap();
    /// let started = store.report(&id, vec![(2, EventPayload::Started)]).unwrap();
    /// assert_eq!((started.applied, started.through), (1, 2));
    /// // sent twice: nothing more is applied
    /// assert_eq!(store.report(&id, vec![(2, EventPayload::Started)]).unwrap().applied, 0);
    /// // after a gap: nothing is applied, and the answer says where to resume
    /// let done = EventPayload::Completed { output: serde_json::json!(1) };
    /// assert_eq!(store.report(&id, vec![(4, done)]).unwrap().through, 2);
    /// assert_eq!(store.get(&id).unwrap().state, ExecutionState::Running);
    /// assert!(store.report(&ExecutionId::fresh(), Vec::new()).is_none(), "not a reported one");
    /// ```
    pub fn report(
        &self,
        id: &ExecutionId,
        events: impl IntoIterator<Item = (u64, EventPayload)>,
    ) -> Option<ReportOutcome> {
        let mut published = Vec::new();
        let outcome = {
            let mut inner = self.lock();
            let mut through = inner.records.get(id)?.reported.as_ref()?.through;
            let mut applied = 0usize;
            for (sequence, payload) in events {
                if sequence <= through {
                    continue;
                }
                if sequence != through + 1 {
                    break;
                }
                through = sequence;
                let state = inner.records.get(id).map(|r| r.snapshot.state);
                let said_already = match &payload {
                    // the record was created here, with its own first event
                    EventPayload::Created { .. } => true,
                    // a stop this process asked for, which the reporter now repeats
                    EventPayload::Cancelling { .. } => state == Some(ExecutionState::Cancelling),
                    _ => false,
                };
                if said_already {
                    continue;
                }
                if let Some(event) = self.publish_locked(&mut inner, id, payload) {
                    applied += 1;
                    published.push(event);
                }
            }
            // the record was found reported above, under this same lock: it is still there
            inner.records.get_mut(id).map(|record| {
                record.reported.iter_mut().for_each(|reported| {
                    reported.through = through;
                    reported.heard = Instant::now();
                });
                ReportOutcome {
                    applied,
                    through,
                    cancel_requested: record.cancel_requested,
                }
            })
        };
        for (event, deliveries) in published {
            traced(&event, deliveries);
        }
        outcome
    }

    /// Fail every reported execution that is not finished and has not been heard from for
    /// longer than `silence`, and answer which. The process that ran it was killed or lost
    /// its way to this one; what it did afterwards is unknown here, which the error says.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
    /// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
    /// let store = ExecutionStore::new(Limits::default());
    /// let seed = || Seed {
    ///     capability: "executions.demonstrate",
    ///     title: "Demonstrate an execution",
    ///     input: serde_json::json!({ "steps": 1 }),
    ///     cancellable: true,
    ///     actor: Actor::reported(4242),
    ///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
    /// };
    /// use std::time::Duration;
    /// let id = ExecutionId::fresh();
    /// store.adopt(id.clone(), seed(), 4242).unwrap();
    /// store.report(&id, vec![(2, EventPayload::Started)]).unwrap();
    /// assert!(store.abandon_silent(Duration::from_secs(30)).is_empty(), "heard just now");
    /// std::thread::sleep(Duration::from_millis(20));
    /// assert_eq!(store.abandon_silent(Duration::from_millis(5)), vec![id.clone()]);
    /// let failed = store.get(&id).unwrap();
    /// assert_eq!(failed.state, ExecutionState::Failed);
    /// assert_eq!(failed.error.unwrap().code, "unavailable");
    /// ```
    pub fn abandon_silent(&self, silence: Duration) -> Vec<ExecutionId> {
        let mut published = Vec::new();
        let abandoned = {
            let mut inner = self.lock();
            let silent: Vec<(ExecutionId, u32, u64)> = inner
                .records
                .iter()
                .filter(|(_, r)| r.snapshot.state.is_active())
                .filter_map(|(id, r)| {
                    let reported = r.reported.as_ref()?;
                    let quiet = reported.heard.elapsed();
                    (quiet > silence).then(|| (id.clone(), reported.pid, quiet.as_secs()))
                })
                .collect();
            let mut abandoned = Vec::new();
            for (id, pid, quiet) in silent {
                let payload = EventPayload::Failed {
                    error: ExecutionError {
                        code: "unavailable".into(),
                        message: format!(
                            "process {pid}, which ran this execution and reported it here, has not reported for {quiet} s; what it did afterwards is unknown to this server"
                        ),
                        suggestion: Some(
                            "the terminal that ran it has the outcome; `majordomus run` reports to the server only while it is running".into(),
                        ),
                        correlation_id: id.to_string(),
                    },
                };
                // an active execution may always fail, so the event is published; the
                // id is named abandoned only when it was
                let event = self.publish_locked(&mut inner, &id, payload);
                abandoned.extend(event.is_some().then_some(id));
                published.extend(event);
            }
            abandoned
        };
        for (event, deliveries) in published {
            traced(&event, deliveries);
        }
        abandoned
    }

    /// Is this an execution another process runs and reports here?
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, EventPayload, ExecutionId, ExecutionState};
    /// use majordomus_cli::execution::{ExecutionStore, Limits, RepositoryRef, Seed};
    /// let store = ExecutionStore::new(Limits::default());
    /// let seed = || Seed {
    ///     capability: "executions.demonstrate",
    ///     title: "Demonstrate an execution",
    ///     input: serde_json::json!({ "steps": 1 }),
    ///     cancellable: true,
    ///     actor: Actor::reported(4242),
    ///     repository: RepositoryRef { name: "r".into(), id: "id".into(), branch: None },
    /// };
    /// let reported = ExecutionId::fresh();
    /// store.adopt(reported.clone(), seed(), 4242).unwrap();
    /// assert!(store.is_reported(&reported));
    /// assert!(!store.is_reported(&ExecutionId::fresh()), "nothing is held under that id");
    /// ```
    pub fn is_reported(&self, id: &ExecutionId) -> bool {
        self.lock()
            .records
            .get(id)
            .is_some_and(|r| r.reported.is_some())
    }

    /// A new record, queued, and the `created` event to publish for it once it is held and
    /// the lock is let go. Built outside the lock: nothing in it reads the store.
    fn record(
        &self,
        id: &ExecutionId,
        seed: Seed<'_>,
        reported: Option<Reported>,
        idempotency: Option<&Idempotency>,
    ) -> (Record, EventPayload) {
        let Seed {
            capability,
            title,
            input,
            cancellable,
            actor,
            repository,
        } = seed;
        let cancel = Arc::new(AtomicBool::new(false));
        let now = crate::peers::rfc3339(std::time::SystemTime::now());
        let snapshot = Execution {
            id: id.clone(),
            capability: capability.to_string(),
            title: title.to_string(),
            state: ExecutionState::Queued,
            created_at: now,
            started_at: None,
            finished_at: None,
            duration_ms: None,
            input: input.clone(),
            output: None,
            error: None,
            progress: None,
            steps: Vec::new(),
            diagnostics: Vec::new(),
            last_sequence: 0,
            event_count: 0,
            events_truncated: false,
            cancellable,
            actor,
            repository,
            correlation_id: id.to_string(),
            idempotency_key: idempotency.map(|i| i.key.to_string()),
        };
        let created = EventPayload::Created {
            capability: capability.to_string(),
            title: title.to_string(),
            input,
        };
        let record = Record {
            snapshot,
            events: VecDeque::new(),
            first_retained: 1,
            cancel,
            cancel_requested: false,
            started: None,
            reported,
            digest: idempotency.map(|i| i.digest.clone()),
        };
        (record, created)
    }

    /// Hold a record and evict what the bound asks. Its `created` event is not published
    /// here: the caller does that once the lock is let go.
    fn hold(&self, inner: &mut Inner, id: ExecutionId, record: Record) {
        inner.records.insert(id.clone(), record);
        inner.order.push_back(id);
        self.evict(inner);
    }

    /// Forget the oldest finished executions until the bound holds. A running execution is
    /// never forgotten, however old: the bound protects memory, and dropping the record of
    /// work that is still happening would protect nothing and lose the only handle on it.
    fn evict(&self, inner: &mut Inner) {
        while inner.records.len() > self.limits.max_executions {
            let Some(position) = inner.order.iter().position(|id| {
                inner
                    .records
                    .get(id)
                    .is_some_and(|r| r.snapshot.state.is_final())
            }) else {
                break;
            };
            if let Some(id) = inner.order.remove(position) {
                if let Some(gone) = inner.records.remove(&id) {
                    // the key goes with the execution it started: the store answers a
                    // repeated start for as long as it holds the execution, and no longer
                    if let Some(scope) = scope_of(&gone.snapshot) {
                        if inner.keys.get(&scope) == Some(&id) {
                            inner.keys.remove(&scope);
                        }
                    }
                }
            }
        }
    }

    /// Apply an event to an execution: move its state, update its snapshot, retain it, and
    /// hand it to every subscriber that wants it.
    ///
    /// Returns the sequence the event was given, or `None` when there is no such execution
    /// or the payload asks for a transition the state machine does not allow — which is
    /// how a second `completed` after a `cancelled` is refused rather than believed.
    pub fn publish(&self, id: &ExecutionId, payload: EventPayload) -> Option<u64> {
        let (event, deliveries) = {
            let mut inner = self.lock();
            self.publish_locked(&mut inner, id, payload)?
        };
        traced(&event, deliveries);
        Some(event.sequence)
    }

    /// [`Self::publish`] under a lock the caller already holds, for a caller whose own
    /// decision has to be taken in the same instant as the transition it leads to.
    fn publish_locked(
        &self,
        inner: &mut Inner,
        id: &ExecutionId,
        payload: EventPayload,
    ) -> Option<(Arc<ExecutionEvent>, usize)> {
        let limits = self.limits;
        let record = inner.records.get_mut(id)?;
        if let Some(next) = payload.implies_state() {
            let current = record.snapshot.state;
            if current == next {
                // saying the same thing twice is not a transition and not an error
            } else if !current.may_move_to(next) {
                tracing::debug!(
                    execution_id = %id,
                    from = current.as_str(),
                    to = next.as_str(),
                    "an execution event was refused: the state machine does not allow the move"
                );
                return None;
            }
        }
        let sequence = record.snapshot.last_sequence + 1;
        apply(record, &payload, sequence, limits);
        let event = Arc::new(ExecutionEvent::new(id.clone(), sequence, payload));
        record.events.push_back(Arc::clone(&event));
        while record.events.len() > limits.max_events {
            record.events.pop_front();
            record.first_retained += 1;
            record.snapshot.events_truncated = true;
        }
        let deliveries = inner.fan_out(id, &event);
        Some((event, deliveries))
    }

    /// The snapshot of one execution.
    pub fn get(&self, id: &ExecutionId) -> Option<Execution> {
        self.lock().records.get(id).map(|r| r.snapshot.clone())
    }

    /// Every execution this process remembers, newest first, optionally narrowed.
    pub fn list(
        &self,
        state: Option<ExecutionState>,
        capability: Option<&str>,
        limit: usize,
    ) -> Vec<Execution> {
        let inner = self.lock();
        inner
            .order
            .iter()
            .rev()
            .filter_map(|id| inner.records.get(id))
            .map(|r| &r.snapshot)
            .filter(|s| state.is_none_or(|want| s.state == want))
            .filter(|s| capability.is_none_or(|want| s.capability == want))
            .take(limit)
            .cloned()
            .collect()
    }

    /// How many executions this store remembers.
    pub fn len(&self) -> usize {
        self.lock().records.len()
    }

    /// Does it remember none?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many executions are in a state right now.
    pub fn count(&self, state: ExecutionState) -> usize {
        self.lock()
            .records
            .values()
            .filter(|r| r.snapshot.state == state)
            .count()
    }

    /// How many are not finished: running, starting, cancelling or queued.
    pub fn active(&self) -> usize {
        self.lock()
            .records
            .values()
            .filter(|r| r.snapshot.state.is_active())
            .count()
    }

    /// A page of one execution's retained history, after a sequence.
    pub fn events(&self, id: &ExecutionId, after: u64, limit: usize) -> Option<EventPage> {
        let inner = self.lock();
        let record = inner.records.get(id)?;
        let all: Vec<Arc<ExecutionEvent>> = record
            .events
            .iter()
            .filter(|e| e.sequence > after)
            .cloned()
            .collect();
        let more = all.len() > limit;
        let events: Vec<Arc<ExecutionEvent>> = all.into_iter().take(limit).collect();
        Some(EventPage {
            last_sequence: events
                .last()
                .map(|e| e.sequence)
                .unwrap_or_else(|| after.max(record.snapshot.last_sequence)),
            more,
            // the client asked from a point the store no longer holds
            truncated: after + 1 < record.first_retained,
            events,
        })
    }

    /// The cancellation token of an execution, for the worker that runs it.
    pub fn token(&self, id: &ExecutionId) -> Option<Arc<AtomicBool>> {
        self.lock().records.get(id).map(|r| Arc::clone(&r.cancel))
    }

    /// Ask an execution to stop. Publishes `execution.cancelling` and sets the token; the
    /// handler decides when it stops, and a handler that never looks at its token will
    /// finish normally, which the final state will say.
    ///
    /// Both happen under one hold of the lock. A worker reads the token without the lock,
    /// but a handler that stops publishes `execution.cancelled`, which needs the lock that
    /// `cancelling` is recorded under, so a handler that sees the token cannot publish
    /// until `cancelling` is recorded, whichever of the two is done first inside. Recording
    /// the state before the token is extra ordering on top of that: on its own it would
    /// hold even were the token set after the lock is let go. What neither may be is the
    /// token set in one hold and `cancelling` published in the next: a handler looking in
    /// between publishes `cancelled` from `running` — a move the state machine refuses —
    /// and the `cancelling` that follows is the last thing the execution ever says.
    pub fn request_cancel(&self, id: &ExecutionId, by: &str) -> CancelOutcome {
        let published = {
            let mut inner = self.lock();
            let Some(record) = inner.records.get_mut(id) else {
                return CancelOutcome::Unknown;
            };
            let state = record.snapshot.state;
            if state.is_final() {
                return CancelOutcome::AlreadyFinished(state);
            }
            if record.cancel_requested {
                return CancelOutcome::AlreadyRequested;
            }
            record.cancel_requested = true;
            // a reported execution is stopped by the process that runs it, which the next
            // report tells; a queued one there is still that process's to finish, so nothing
            // is said here that its own report would then contradict
            if record.reported.is_some() && state == ExecutionState::Queued {
                return CancelOutcome::Requested;
            }
            let token = Arc::clone(&record.cancel);
            // a queued execution has no worker to notice the token, so it is finished here
            let payload = if state == ExecutionState::Queued {
                EventPayload::Cancelled
            } else {
                EventPayload::Cancelling { by: by.to_string() }
            };
            let published = self.publish_locked(&mut inner, id, payload);
            token.store(true, Ordering::SeqCst);
            published
        };
        if let Some((event, deliveries)) = published {
            traced(&event, deliveries);
        }
        CancelOutcome::Requested
    }

    /// Listen. The receiver is dropped to unsubscribe; the store forgets a subscriber the
    /// first time a send finds it gone, so nothing has to be cleaned up by hand.
    pub fn subscribe(&self, filter: Filter) -> (u64, std::sync::mpsc::Receiver<Delivery>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(self.limits.max_subscriber_queue);
        let id = self.next_subscriber.fetch_add(1, Ordering::Relaxed);
        self.lock().subscribers.push(Subscriber {
            id,
            filter,
            tx,
            dropped: 0,
        });
        (id, rx)
    }

    /// Change what a subscriber wants, without reconnecting.
    pub fn resubscribe(&self, subscriber: u64, filter: Filter) -> bool {
        let mut inner = self.lock();
        match inner.subscribers.iter_mut().find(|s| s.id == subscriber) {
            Some(s) => {
                s.filter = filter;
                true
            }
            None => false,
        }
    }

    /// Stop listening.
    pub fn unsubscribe(&self, subscriber: u64) {
        self.lock().subscribers.retain(|s| s.id != subscriber);
    }

    /// How many subscribers this store is feeding.
    pub fn subscribers(&self) -> usize {
        self.lock().subscribers.len()
    }
}

impl Inner {
    /// Hand one event to every subscriber that wants it, and forget the ones that are
    /// gone. Returns how many received it.
    fn fan_out(&mut self, id: &ExecutionId, event: &Arc<ExecutionEvent>) -> usize {
        let mut delivered = 0usize;
        let mut gone = Vec::new();
        for subscriber in self.subscribers.iter_mut() {
            if !subscriber.filter.wants(id) {
                continue;
            }
            if subscriber.dropped > 0 {
                // tell it once that it missed something, before anything else
                match subscriber.tx.try_send(Delivery::Lagged {
                    dropped: subscriber.dropped,
                }) {
                    Ok(()) => subscriber.dropped = 0,
                    Err(std::sync::mpsc::TrySendError::Full(_)) => {
                        subscriber.dropped += 1;
                        continue;
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        gone.push(subscriber.id);
                        continue;
                    }
                }
            }
            match subscriber.tx.try_send(Delivery::Event(Arc::clone(event))) {
                Ok(()) => delivered += 1,
                // a slow client never stalls an execution: the event is dropped for that
                // client alone and it is told to resynchronise
                Err(std::sync::mpsc::TrySendError::Full(_)) => subscriber.dropped += 1,
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => gone.push(subscriber.id),
            }
        }
        if !gone.is_empty() {
            self.subscribers.retain(|s| !gone.contains(&s.id));
        }
        delivered
    }
}

/// Say in this process's log that an event was published, once the lock is let go.
fn traced(event: &ExecutionEvent, deliveries: usize) {
    tracing::debug!(
        execution_id = %event.execution_id,
        sequence = event.sequence,
        event = event.type_name(),
        subscribers = deliveries,
        "execution event"
    );
}

/// Fold one event into a snapshot. The only place a snapshot changes.
fn apply(record: &mut Record, payload: &EventPayload, sequence: u64, limits: Limits) {
    let now = crate::peers::rfc3339(std::time::SystemTime::now());
    let s = &mut record.snapshot;
    s.last_sequence = sequence;
    s.event_count += 1;
    if let Some(next) = payload.implies_state() {
        s.state = next;
    }
    match payload {
        EventPayload::Started => {
            s.started_at = Some(now.clone());
            record.started = Some(Instant::now());
        }
        EventPayload::Progress(p) => {
            s.progress = Some(ProgressView {
                current: p.current,
                total: p.total,
                message: p.message.clone(),
            });
        }
        EventPayload::StepStarted { name, title } => {
            s.steps.push(StepView {
                name: name.clone(),
                title: title.clone(),
                state: StepState::Running,
                started_at: now.clone(),
                finished_at: None,
                detail: None,
            });
        }
        EventPayload::StepCompleted { name, ok, detail } => {
            let state = if *ok {
                StepState::Completed
            } else {
                StepState::Failed
            };
            match s.steps.iter_mut().rev().find(|s| &s.name == name) {
                Some(step) => {
                    step.state = state;
                    step.finished_at = Some(now.clone());
                    step.detail.clone_from(detail);
                }
                // a handler that reports a phase as decided without having announced it
                // first is reporting the truth; inventing a start it never had would not be
                None => s.steps.push(StepView {
                    name: name.clone(),
                    title: name.clone(),
                    state,
                    started_at: now.clone(),
                    finished_at: Some(now.clone()),
                    detail: detail.clone(),
                }),
            }
        }
        EventPayload::Diagnostic(d) => {
            // bounded like everything else: a handler that reports a finding per file of a
            // large repository must not grow the snapshot every list request answers
            if s.diagnostics.len() < limits.max_events.min(200) {
                s.diagnostics.push(d.clone());
            }
        }
        EventPayload::Completed { output } => {
            s.output = Some(output.clone());
            s.error = None;
        }
        EventPayload::Failed { error } => {
            s.error = Some(error.clone());
            s.output = None;
        }
        // A cancellation after a failure keeps the failure: the guard, not a nested `if`,
        // because the arm below it is the no-op the other case already falls to.
        EventPayload::Cancelled if s.error.is_none() => {
            s.error = Some(ExecutionError {
                code: "cancelled".into(),
                message: "the execution was cancelled".into(),
                suggestion: None,
                correlation_id: s.correlation_id.clone(),
            });
        }
        _ => {}
    }
    if s.state.is_final() && s.finished_at.is_none() {
        s.finished_at = Some(now);
        s.duration_ms = record.started.map(|t| t.elapsed().as_millis() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::model::{ActorKind, LogStream};

    fn store() -> ExecutionStore {
        ExecutionStore::new(Limits::default())
    }

    fn repository() -> RepositoryRef {
        RepositoryRef {
            name: "r".into(),
            id: "id".into(),
            branch: None,
        }
    }

    fn start(store: &ExecutionStore) -> ExecutionId {
        let id = ExecutionId::fresh();
        store.create(
            id.clone(),
            "demo.x",
            "Demo",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            repository(),
        );
        id
    }

    #[test]
    fn a_snapshot_and_the_stream_agree_at_every_step() {
        let store = store();
        let (_, rx) = store.subscribe(Filter::All);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        store.publish(
            &id,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: Some(2),
                message: Some("half".into()),
            }),
        );
        store.publish(
            &id,
            EventPayload::Completed {
                output: serde_json::json!({ "ok": true }),
            },
        );
        let snapshot = store.get(&id).expect("a snapshot");
        assert_eq!(snapshot.state, ExecutionState::Succeeded);
        assert_eq!(snapshot.output, Some(serde_json::json!({ "ok": true })));
        assert_eq!(snapshot.percent(), Some(50));
        assert!(snapshot.finished_at.is_some());
        assert_eq!(snapshot.last_sequence, 4);
        assert_eq!(snapshot.event_count, 4);

        let seen: Vec<Arc<ExecutionEvent>> = rx
            .try_iter()
            .map(|d| match d {
                Delivery::Event(e) => e,
                Delivery::Lagged { .. } => panic!("a reader that kept up was told it lagged"),
            })
            .collect();
        assert_eq!(seen.len(), 4);
        assert_eq!(
            seen.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [1, 2, 3, 4],
            "sequences are dense and ordered"
        );
        assert_eq!(seen.last().unwrap().type_name(), "execution.completed");
    }

    #[test]
    fn a_refused_transition_changes_nothing() {
        let store = store();
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        store.publish(
            &id,
            EventPayload::Completed {
                output: Value::Null,
            },
        );
        let before = store.get(&id).unwrap();
        assert_eq!(
            store.publish(&id, EventPayload::Started),
            None,
            "a finished execution does not start again"
        );
        assert_eq!(
            store.publish(
                &id,
                EventPayload::Failed {
                    error: ExecutionError {
                        code: "internal".into(),
                        message: "no".into(),
                        suggestion: None,
                        correlation_id: id.to_string()
                    }
                }
            ),
            None
        );
        assert_eq!(store.get(&id).unwrap(), before);
        assert_eq!(
            store.publish(&ExecutionId::fresh(), EventPayload::Started),
            None
        );
    }

    #[test]
    fn history_replays_from_a_cursor_and_says_when_it_lost_the_start() {
        let limits = Limits {
            max_events: 4,
            ..Limits::default()
        };
        let store = ExecutionStore::new(limits);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        for n in 0..6 {
            store.publish(
                &id,
                EventPayload::Log {
                    stream: LogStream::Handler,
                    message: format!("line {n}"),
                },
            );
        }
        let page = store.events(&id, 0, 100).expect("a page");
        assert_eq!(page.events.len(), 4, "the bound holds");
        assert!(page.truncated, "and says the start is gone");
        let snapshot = store.get(&id).unwrap();
        assert!(snapshot.events_truncated);
        assert_eq!(snapshot.event_count, 8, "counted, not retained");

        let last = page.last_sequence;
        let after = store.events(&id, last, 100).unwrap();
        assert!(after.events.is_empty());
        assert!(!after.more);
        assert_eq!(after.last_sequence, last);

        let paged = store.events(&id, 4, 2).unwrap();
        assert_eq!(paged.events.len(), 2);
        assert!(paged.more);
    }

    #[test]
    fn a_slow_subscriber_is_told_to_resynchronise_and_never_stalls_the_execution() {
        let store = ExecutionStore::new(Limits {
            max_subscriber_queue: 2,
            ..Limits::default()
        });
        let (_, rx) = store.subscribe(Filter::All);
        let id = start(&store); // fills the queue: created is 1 of 2
        for n in 0..20 {
            store.publish(
                &id,
                EventPayload::Log {
                    stream: LogStream::Handler,
                    message: n.to_string(),
                },
            );
        }
        // the execution kept going regardless of the reader
        assert_eq!(store.get(&id).unwrap().event_count, 21);
        let deliveries: Vec<Delivery> = rx.try_iter().collect();
        assert!(deliveries.len() <= 3, "the queue stayed bounded");
        // drain and then publish once more: the reader is told what it missed
        store.publish(&id, EventPayload::Started);
        let mut told = false;
        for d in rx.try_iter() {
            if let Delivery::Lagged { dropped } = d {
                assert!(dropped > 0);
                told = true;
            }
        }
        assert!(told, "a lagging subscriber is told, not silently starved");
    }

    #[test]
    fn a_dropped_receiver_is_forgotten_on_the_next_event() {
        let store = store();
        let (_, rx) = store.subscribe(Filter::All);
        assert_eq!(store.subscribers(), 1);
        drop(rx);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        assert_eq!(store.subscribers(), 0);
    }

    #[test]
    fn a_filter_narrows_and_can_be_changed_without_reconnecting() {
        let store = store();
        let a = start(&store);
        let (sub, rx) = store.subscribe(Filter::Only(vec![a.clone()]));
        let b = start(&store);
        store.publish(&b, EventPayload::Started);
        assert!(rx.try_iter().next().is_none(), "b was not wanted");
        assert!(store.resubscribe(sub, Filter::Only(vec![b.clone()])));
        store.publish(
            &b,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: None,
                message: None,
            }),
        );
        assert!(matches!(rx.try_recv(), Ok(Delivery::Event(_))));
        store.unsubscribe(sub);
        assert_eq!(store.subscribers(), 0);
        assert!(!store.resubscribe(sub, Filter::All));
    }

    #[test]
    fn cancelling_a_queued_execution_finishes_it_and_cancelling_twice_says_so() {
        let store = store();
        let id = start(&store);
        assert_eq!(store.request_cancel(&id, "test"), CancelOutcome::Requested);
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelled);
        assert_eq!(
            store.request_cancel(&id, "test"),
            CancelOutcome::AlreadyFinished(ExecutionState::Cancelled)
        );
        assert_eq!(
            store.request_cancel(&ExecutionId::fresh(), "test"),
            CancelOutcome::Unknown
        );

        let running = start(&store);
        store.publish(&running, EventPayload::Started);
        assert_eq!(
            store.request_cancel(&running, "test"),
            CancelOutcome::Requested
        );
        assert!(store.token(&running).unwrap().load(Ordering::SeqCst));
        assert_eq!(
            store.get(&running).unwrap().state,
            ExecutionState::Cancelling
        );
        assert_eq!(
            store.request_cancel(&running, "test"),
            CancelOutcome::AlreadyRequested
        );
        store.publish(&running, EventPayload::Cancelled);
        let snapshot = store.get(&running).unwrap();
        assert_eq!(snapshot.state, ExecutionState::Cancelled);
        assert_eq!(
            snapshot.error.map(|e| e.code),
            Some("cancelled".to_string())
        );
    }

    #[test]
    fn a_handler_that_stops_the_instant_it_sees_the_token_still_ends_cancelled() {
        // A worker reads the token without the lock and publishes `cancelled` the moment it
        // sees it. Were the token visible, with the lock let go, before `cancelling` is
        // recorded, that `cancelled` would arrive from `running`, which the state machine
        // refuses, and the `cancelling` that landed after it would be the execution's last
        // word — the engine's test once waited its whole ten seconds on exactly that. A
        // worker spinning on the token makes the loaded runner's rare interleaving the
        // common one.
        for round in 0..300 {
            let store = Arc::new(store());
            let id = start(&store);
            store.publish(&id, EventPayload::Started);
            let token = store.token(&id).unwrap();
            let ready = Arc::new(AtomicBool::new(false));
            let worker = {
                let (store, id, ready) = (Arc::clone(&store), id.clone(), Arc::clone(&ready));
                std::thread::spawn(move || {
                    ready.store(true, Ordering::SeqCst);
                    // bounded, so that a cancel that never sets the token fails rather than hangs
                    let deadline = Instant::now() + std::time::Duration::from_secs(10);
                    while !token.load(Ordering::SeqCst) {
                        if Instant::now() >= deadline {
                            return None;
                        }
                        std::hint::spin_loop();
                    }
                    store.publish(&id, EventPayload::Cancelled)
                })
            };
            while !ready.load(Ordering::SeqCst) {
                std::hint::spin_loop();
            }
            assert_eq!(store.request_cancel(&id, "test"), CancelOutcome::Requested);
            let published = worker.join().unwrap();
            let state = store.get(&id).unwrap().state;
            assert!(
                published.is_some() && state == ExecutionState::Cancelled,
                "round {round}: the handler stopped and the execution says {state:?}"
            );
        }

        // the other side of the same lock: once a queued execution is cancelled, a worker
        // that picked it up in that instant is refused its start, and so runs nothing
        let store = store();
        let id = start(&store);
        assert_eq!(store.request_cancel(&id, "test"), CancelOutcome::Requested);
        assert_eq!(store.publish(&id, EventPayload::Started), None);
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelled);
    }

    #[test]
    fn the_oldest_finished_execution_is_forgotten_and_a_running_one_never_is() {
        let store = ExecutionStore::new(Limits {
            max_executions: 2,
            ..Limits::default()
        });
        let running = start(&store);
        store.publish(&running, EventPayload::Started);
        let mut finished = Vec::new();
        for _ in 0..4 {
            let id = start(&store);
            store.publish(&id, EventPayload::Started);
            store.publish(
                &id,
                EventPayload::Completed {
                    output: Value::Null,
                },
            );
            finished.push(id);
        }
        assert!(
            store.get(&running).is_some(),
            "work in flight is not evicted"
        );
        assert!(
            store.get(finished.last().unwrap()).is_some(),
            "the newest is kept"
        );
        assert!(
            store.get(&finished[0]).is_none(),
            "the oldest finished is gone"
        );
        assert_eq!(store.active(), 1);
        assert_eq!(
            store.count(ExecutionState::Succeeded),
            store.list(Some(ExecutionState::Succeeded), None, 100).len()
        );
    }

    fn seed() -> Seed<'static> {
        Seed {
            capability: "demo.x",
            title: "Demo",
            input: serde_json::json!({}),
            cancellable: true,
            actor: Actor::reported(4242),
            repository: repository(),
        }
    }

    fn log(n: u32) -> EventPayload {
        EventPayload::Log {
            stream: LogStream::Handler,
            message: format!("line {n}"),
        }
    }

    #[test]
    fn a_reported_execution_is_applied_once_in_order_and_resumes_after_a_gap() {
        let store = store();
        let (_, rx) = store.subscribe(Filter::All);
        let id = ExecutionId::fresh();
        store.adopt(id.clone(), seed(), 4242).expect("adopted");
        assert!(store.is_reported(&id));
        // adopting again — every report carries the description — changes nothing
        store
            .adopt(id.clone(), seed(), 4242)
            .expect("adopted twice");
        assert_eq!(store.get(&id).unwrap().event_count, 1, "one created event");

        // the reporter's own sequence: 1 created, 2 queued, 3 started, 4 a log line
        let first = vec![
            (
                1,
                EventPayload::Created {
                    capability: "demo.x".into(),
                    title: "Demo".into(),
                    input: serde_json::json!({}),
                },
            ),
            (2, EventPayload::Queued { ahead: 0 }),
            (3, EventPayload::Started),
            (4, log(1)),
        ];
        let outcome = store
            .report(&id, first.clone())
            .expect("a reported execution");
        assert_eq!((outcome.applied, outcome.through), (3, 4));
        assert!(!outcome.cancel_requested);
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Running);

        // the same report again is harmless
        let again = store.report(&id, first).unwrap();
        assert_eq!((again.applied, again.through), (0, 4));

        // a report with a gap applies nothing past it and says where to resume
        let gapped = store.report(&id, vec![(6, log(3))]).unwrap();
        assert_eq!((gapped.applied, gapped.through), (0, 4));
        let resumed = store
            .report(
                &id,
                vec![
                    (5, log(2)),
                    (6, log(3)),
                    (
                        7,
                        EventPayload::Completed {
                            output: serde_json::json!({ "ok": true }),
                        },
                    ),
                ],
            )
            .unwrap();
        assert_eq!((resumed.applied, resumed.through), (3, 7));
        let snapshot = store.get(&id).unwrap();
        assert_eq!(snapshot.state, ExecutionState::Succeeded);
        assert_eq!(snapshot.output, Some(serde_json::json!({ "ok": true })));
        assert_eq!(snapshot.actor.pid, Some(4242));
        // the live channel saw it as it sees this process's own
        let types: Vec<&str> = rx
            .try_iter()
            .filter_map(|d| match d {
                Delivery::Event(e) => Some(e.type_name()),
                Delivery::Lagged { .. } => None,
            })
            .collect();
        assert_eq!(
            types,
            [
                "execution.created",
                "execution.queued",
                "execution.started",
                "execution.log",
                "execution.log",
                "execution.log",
                "execution.completed"
            ]
        );
    }

    /// A report about an id this store holds nothing under answers nothing, and a reporter
    /// that repeats its own `created` past the one the store published is not believed twice.
    #[test]
    fn a_report_about_nothing_held_answers_nothing_and_a_repeated_created_is_passed_over() {
        let store = store();
        assert_eq!(
            store.report(&ExecutionId::fresh(), vec![(2, EventPayload::Started)]),
            None
        );
        let id = ExecutionId::fresh();
        store.adopt(id.clone(), seed(), 4242).expect("adopted");
        let outcome = store
            .report(
                &id,
                vec![(
                    2,
                    EventPayload::Created {
                        capability: "demo.x".into(),
                        title: "Demo".into(),
                        input: serde_json::json!({}),
                    },
                )],
            )
            .expect("a reported execution");
        assert_eq!((outcome.applied, outcome.through), (0, 2));
        assert_eq!(store.get(&id).unwrap().event_count, 1, "one created event");
    }

    #[test]
    fn a_report_cannot_write_into_an_execution_this_process_runs() {
        let store = store();
        let own = start(&store);
        assert_eq!(
            store.adopt(own.clone(), seed(), 1),
            Err(AdoptError::Own(own.clone()))
        );
        assert!(!store.is_reported(&own));
        assert_eq!(
            store.report(&own, vec![(2, EventPayload::Started)]),
            None,
            "a report names a reported execution or nothing"
        );
        assert_eq!(store.get(&own).unwrap().state, ExecutionState::Queued);
    }

    #[test]
    fn a_stop_asked_here_is_handed_to_the_process_that_runs_it() {
        let store = store();
        let queued = ExecutionId::fresh();
        store.adopt(queued.clone(), seed(), 7).unwrap();
        store
            .report(&queued, vec![(2, EventPayload::Queued { ahead: 1 })])
            .unwrap();
        // queued there: nothing is said here that the reporter would then contradict
        assert_eq!(
            store.request_cancel(&queued, "client"),
            CancelOutcome::Requested
        );
        assert_eq!(store.get(&queued).unwrap().state, ExecutionState::Queued);
        let told = store.report(&queued, Vec::new()).unwrap();
        assert!(told.cancel_requested, "the reporter is told to stop it");
        store
            .report(&queued, vec![(3, EventPayload::Cancelled)])
            .unwrap();
        assert_eq!(store.get(&queued).unwrap().state, ExecutionState::Cancelled);

        let running = ExecutionId::fresh();
        store.adopt(running.clone(), seed(), 7).unwrap();
        store
            .report(
                &running,
                vec![
                    (2, EventPayload::Queued { ahead: 0 }),
                    (3, EventPayload::Started),
                ],
            )
            .unwrap();
        assert_eq!(
            store.request_cancel(&running, "client"),
            CancelOutcome::Requested
        );
        assert_eq!(
            store.get(&running).unwrap().state,
            ExecutionState::Cancelling
        );
        let before = store.get(&running).unwrap().event_count;
        // the reporter cancels its own and says so; the repeat is not a second event
        let outcome = store
            .report(
                &running,
                vec![
                    (
                        4,
                        EventPayload::Cancelling {
                            by: "server".into(),
                        },
                    ),
                    (5, EventPayload::Cancelled),
                ],
            )
            .unwrap();
        assert_eq!((outcome.applied, outcome.through), (1, 5));
        let snapshot = store.get(&running).unwrap();
        assert_eq!(snapshot.state, ExecutionState::Cancelled);
        assert_eq!(snapshot.event_count, before + 1);
    }

    #[test]
    fn a_reporter_that_goes_silent_is_failed_rather_than_shown_running_for_ever() {
        let store = store();
        let id = ExecutionId::fresh();
        store.adopt(id.clone(), seed(), 99).unwrap();
        store
            .report(
                &id,
                vec![
                    (2, EventPayload::Queued { ahead: 0 }),
                    (3, EventPayload::Started),
                ],
            )
            .unwrap();
        let own = start(&store);
        store.publish(&own, EventPayload::Started);
        assert!(
            store.abandon_silent(REPORT_SILENCE).is_empty(),
            "heard just now"
        );
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            store.abandon_silent(Duration::from_millis(5)),
            vec![id.clone()]
        );
        let snapshot = store.get(&id).unwrap();
        assert_eq!(snapshot.state, ExecutionState::Failed);
        let error = snapshot.error.expect("an error");
        assert_eq!(error.code, "unavailable");
        assert!(error.message.contains("process 99"), "{}", error.message);
        assert_eq!(
            store.get(&own).unwrap().state,
            ExecutionState::Running,
            "this process's own execution is never abandoned"
        );
        assert!(store.abandon_silent(Duration::ZERO).is_empty(), "once");
        // a reporter that comes back finds it final, and nothing moves it
        store.report(
            &id,
            vec![(
                4,
                EventPayload::Completed {
                    output: Value::Null,
                },
            )],
        );
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Failed);
    }

    #[test]
    fn listing_is_newest_first_and_narrows_by_state_and_capability() {
        let store = store();
        let a = start(&store);
        let b = start(&store);
        store.publish(&b, EventPayload::Started);
        let all = store.list(None, None, 10);
        assert_eq!(all.first().map(|e| e.id.clone()), Some(b.clone()));
        assert_eq!(all.len(), 2);
        assert_eq!(store.list(Some(ExecutionState::Queued), None, 10)[0].id, a);
        assert_eq!(store.list(None, Some("demo.x"), 10).len(), 2);
        assert!(store.list(None, Some("nothing.here"), 10).is_empty());
        assert_eq!(store.list(None, None, 1).len(), 1);
    }

    fn keyed(key: &str, digest: &str) -> Option<Idempotency> {
        Some(Idempotency {
            key: IdempotencyKey::parse(key).unwrap(),
            digest: digest.into(),
        })
    }

    fn seed_in(repository_id: &str, capability: &'static str) -> Seed<'static> {
        Seed {
            capability,
            title: "Demo",
            input: serde_json::json!({}),
            cancellable: true,
            actor: Actor::of(ActorKind::Http),
            repository: RepositoryRef {
                name: "r".into(),
                id: repository_id.into(),
                branch: None,
            },
        }
    }

    #[test]
    fn two_starts_sent_at_once_with_one_key_record_one_execution() {
        // I4000: the double click. Every thread is released at the same instant and sends
        // the same key with the same input; exactly one may be told it created anything.
        let store = Arc::new(store());
        let threads = 16;
        let gate = Arc::new(std::sync::Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let store = Arc::clone(&store);
                let gate = Arc::clone(&gate);
                std::thread::spawn(move || {
                    let id = ExecutionId::fresh();
                    gate.wait();
                    match store.create_once(id.clone(), seed_in("id", "demo.x"), keyed("k", "d")) {
                        Ok(Admission::Created(_)) => (true, id),
                        Ok(Admission::Existing(e)) => (false, e.id),
                        Err(e) => panic!("the same input was refused: {e}"),
                    }
                })
            })
            .collect();
        let answers: Vec<(bool, ExecutionId)> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        let created: Vec<_> = answers.iter().filter(|(c, _)| *c).collect();
        assert_eq!(
            created.len(),
            1,
            "one start created, the rest were answered it"
        );
        let first = &created[0].1;
        assert!(
            answers.iter().all(|(_, id)| id == first),
            "every answer names one id"
        );
        assert_eq!(store.len(), 1, "one execution is held");
        assert_eq!(
            store.get(first).unwrap().idempotency_key.as_deref(),
            Some("k"),
            "the key is carried on the record"
        );
    }

    #[test]
    fn a_key_sent_again_with_another_input_is_refused_and_names_what_it_started() {
        let store = store();
        let first = ExecutionId::fresh();
        store
            .create_once(first.clone(), seed_in("id", "demo.x"), keyed("k", "a"))
            .unwrap();
        let refused = store
            .create_once(
                ExecutionId::fresh(),
                seed_in("id", "demo.x"),
                keyed("k", "b"),
            )
            .unwrap_err();
        assert_eq!(refused.existing, first);
        assert_eq!(
            (refused.key.as_str(), refused.capability.as_str()),
            ("k", "demo.x")
        );
        assert!(refused.to_string().contains(first.as_str()), "{refused}");
        assert_eq!(store.len(), 1, "a refusal records nothing");
    }

    #[test]
    fn a_key_is_scoped_to_its_repository_and_capability_and_absent_keys_never_match() {
        let store = store();
        for (repository_id, capability) in [("id", "demo.x"), ("id", "demo.y"), ("other", "demo.x")]
        {
            assert!(matches!(
                store.create_once(
                    ExecutionId::fresh(),
                    seed_in(repository_id, capability),
                    keyed("k", "a")
                ),
                Ok(Admission::Created(_))
            ));
        }
        for _ in 0..2 {
            assert!(matches!(
                store.create_once(ExecutionId::fresh(), seed_in("id", "demo.x"), None),
                Ok(Admission::Created(_))
            ));
        }
        assert_eq!(
            store.len(),
            5,
            "three scopes of one key, and two starts without one"
        );
    }

    #[test]
    fn a_key_is_held_as_long_as_its_execution_and_freed_when_it_is_forgotten() {
        let store = ExecutionStore::new(Limits {
            max_executions: 1,
            ..Limits::default()
        });
        let first = ExecutionId::fresh();
        store
            .create_once(first.clone(), seed_in("id", "demo.x"), keyed("k", "a"))
            .unwrap();
        store.publish(&first, EventPayload::Started);
        store.publish(
            &first,
            EventPayload::Completed {
                output: Value::Null,
            },
        );
        // finished and still held: a repeated start is answered it, not run again
        match store
            .create_once(
                ExecutionId::fresh(),
                seed_in("id", "demo.x"),
                keyed("k", "a"),
            )
            .unwrap()
        {
            Admission::Existing(e) => {
                assert_eq!((e.id, e.state), (first.clone(), ExecutionState::Succeeded))
            }
            Admission::Created(_) => panic!("a finished execution's key started another"),
        }
        // another execution pushes the finished one out of the bound, and its key with it
        start(&store);
        assert!(store.get(&first).is_none());
        assert!(
            matches!(
                store.create_once(
                    ExecutionId::fresh(),
                    seed_in("id", "demo.x"),
                    keyed("k", "b")
                ),
                Ok(Admission::Created(_))
            ),
            "a forgotten execution's key is free, whatever input it is sent with now"
        );
    }
}
