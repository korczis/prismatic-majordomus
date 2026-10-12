//! The one execution path.
//!
//! Every call of an executable capability — from the stdio MCP session, from `/mcp`, from
//! an HTTP route, from the command line, from the Cockpit or from a benchmark runner —
//! goes through [`CapabilityExecutor::execute`]: registry lookup, counters, the cache the
//! capability's own policy asks for, then the handler. Transport adapters convert protocol
//! to JSON and back and own nothing else, so instrumentation and caching apply to every
//! transport at once and cannot drift apart. A second execution path is the defect this
//! module exists to make impossible.
//!
//! # The cache key
//!
//! Process memory, bounded per capability by its [`CachePolicy`], keyed
//! by three things: the canonical id, the input **normalised** (object keys sorted at every
//! level, so `{"a":1,"b":2}` and `{"b":2,"a":1}` are one entry), and the registry
//! fingerprint — which hashes every descriptor and every declarative object's content. Two
//! processes over different repository states therefore never share an entry, and a changed
//! layer never answers from an old one.
//!
//! Errors are never cached. Commands are never cached, and the registry refuses a
//! descriptor that asks for it, so that is a build-time refusal rather than a runtime one.
//!
//! Two policies narrow the key further, for answers the registry fingerprint cannot see
//! move. [`CachePolicy::Generation`] adds the context's [`Context::reading`]: an answer
//! read off the index, including the git state it was read at, is kept for as long as that
//! reading is the one asked. [`CachePolicy::Checkout`] adds that and the fingerprint of the
//! checkout, taken again on every call (`capability::checkout`): HEAD, the staging index,
//! the refs, and every changed or untracked path of the work tree with its size and
//! modification time. A checkout git cannot fingerprint is not cached.
//!
//! # One handler per key at a time
//!
//! Concurrent calls that miss on the same key do not each run the handler: the first runs
//! it, the others wait for it and are answered from what it stored. A screen that asks the
//! same source from two panels at once, or two browsers that open one page together, pay
//! for one computation. A handler that fails stores nothing, so a caller that waited on it
//! runs the handler itself.
//!
//! # Switching it off
//!
//! `MAJORDOMUS_CAPABILITY_CACHE=off` in a process's environment makes every call run its
//! handler and store nothing, whatever the capability's policy: the way to settle a doubt
//! about a cached answer by asking without the cache (`project.cache-is-invisible`).
//!
//! # Invisibility
//!
//! A cached answer is byte-identical to a computed one; nothing in a response says which it
//! was. `project.cache-is-invisible` requires it, and [`canonical_json`] is what makes the
//! key well-defined enough for it to be true.
//!
//! ```
//! use majordomus_cli::capability::executor::canonical_json;
//!
//! // the same input written two ways is one cache entry, at every level of nesting
//! let one = serde_json::json!({ "b": 2, "a": { "y": 1, "x": 0 } });
//! let other = serde_json::json!({ "a": { "x": 0, "y": 1 }, "b": 2 });
//! assert_eq!(canonical_json(&one), canonical_json(&other));
//!
//! // and a different input is a different entry
//! assert_ne!(canonical_json(&one), canonical_json(&serde_json::json!({ "b": 3 })));
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::perf::{self, Counters, Phase, COUNTERS};

use super::handler::{CapabilityError, Context};
use super::model::CachePolicy;

/// The executor of one registry.
#[derive(Debug, Default)]
pub struct CapabilityExecutor {
    cache: Mutex<Cache>,
    /// The handlers this executor ran: cache misses and uncached calls. A measurement of
    /// this executor reads it; the process-wide counters also count every other executor
    /// in the process, which a test running beside another reads as its own.
    invocations: AtomicU64,
    /// How many calls of a capability that writes the repository have succeeded here.
    writes: AtomicU64,
    /// The latest reading of every source a page has asked for, kept current off the path
    /// of the request (`capability::readings`). Beside the cache and not in it: the cache
    /// holds answers, and a reading says how old it is.
    pub(crate) readings: super::readings::Readings,
}

#[derive(Debug, Default)]
struct Cache {
    entries: HashMap<CacheKey, CacheEntry>,
    /// Insertion order per capability, for eviction.
    order: HashMap<String, VecDeque<CacheKey>>,
    /// The keys whose handler is running now, each with the lock its waiters queue on.
    flights: HashMap<CacheKey, Arc<Mutex<()>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    id: String,
    input: String,
    fingerprint: String,
    /// What the policy adds to the registry fingerprint: empty for
    /// [`CachePolicy::Process`], the context's reading for [`CachePolicy::Generation`], and
    /// the reading with the checkout's fingerprint for [`CachePolicy::Checkout`].
    state: String,
}

/// The environment switch that turns the capability cache off for a process.
pub const SWITCH: &str = "MAJORDOMUS_CAPABILITY_CACHE";

/// Whether this process was told to run every capability without its cache: `SWITCH`
/// set to `off`.
///
/// ```
/// use majordomus_cli::capability::executor::{bypassed, SWITCH};
/// assert_eq!(bypassed(), std::env::var_os(SWITCH).is_some_and(|v| v == "off"));
/// ```
pub fn bypassed() -> bool {
    std::env::var_os(SWITCH).is_some_and(|v| v == "off")
}

#[derive(Debug)]
struct CacheEntry {
    value: Value,
    stored: Instant,
}

impl CapabilityExecutor {
    /// An executor with an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a capability by id with a JSON input. The one place a handler runs.
    pub fn execute(&self, ctx: &Context, id: &str, input: Value) -> Result<Value, CapabilityError> {
        let policy = ctx.registry.get(id).map(|c| c.cache);
        self.run(ctx, id, input, policy)
    }

    /// The same call with the cache stepped over: the handler runs, whatever is stored.
    ///
    /// One caller: the execution engine. A cached answer is indistinguishable from a fresh
    /// one — that is the point of the cache and the rule `project.cache-is-invisible` says
    /// so — but an *execution* is not an answer. It is a record of work happening, watched
    /// while it happens, and an execution answered from a cache would report no steps, no
    /// progress and no output of its own while claiming to have succeeded: a story of work
    /// that did not occur. So an execution always runs the handler, and nothing it produces
    /// is put into the cache either, because the cache holds what calls returned and this
    /// call is a different kind of thing.
    pub fn execute_uncached(
        &self,
        ctx: &Context,
        id: &str,
        input: Value,
    ) -> Result<Value, CapabilityError> {
        self.run(ctx, id, input, Some(CachePolicy::Disabled))
    }

    fn run(
        &self,
        ctx: &Context,
        id: &str,
        input: Value,
        policy: Option<CachePolicy>,
    ) -> Result<Value, CapabilityError> {
        Counters::bump(&COUNTERS.executions);
        let policy = policy.ok_or_else(|| CapabilityError::NotFound(format!("capability {id}")))?;
        let policy = if bypassed() {
            CachePolicy::Disabled
        } else {
            policy
        };
        let Some(key) = key_of(ctx, id, &input, policy) else {
            return self.invoke(ctx, id, input);
        };
        let ttl = policy.ttl_seconds();
        if let Some(value) = self.lookup(&key, ttl) {
            return Ok(value);
        }
        // a miss: take the key's flight, so that one handler runs for it at a time, and look
        // again once it is ours — a caller that waited is answered by the one it waited for
        let flight = {
            let mut cache = lock(&self.cache);
            Arc::clone(cache.flights.entry(key.clone()).or_default())
        };
        let _flight = flight.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(value) = self.peek(&key, ttl) {
            return Ok(value);
        }
        let value = self.invoke(ctx, id, input);
        let mut cache = lock(&self.cache);
        if cache
            .flights
            .get(&key)
            .is_some_and(|f| Arc::ptr_eq(f, &flight))
        {
            cache.flights.remove(&key);
        }
        let value = value?;
        let max_entries = policy.max_entries().unwrap_or(1);
        let Cache { entries, order, .. } = &mut *cache;
        let order = order.entry(key.id.clone()).or_default();
        if !entries.contains_key(&key) {
            order.push_back(key.clone());
            while order.len() > max_entries {
                if let Some(old) = order.pop_front() {
                    entries.remove(&old);
                    Counters::bump(&COUNTERS.cache_evictions);
                }
            }
        }
        entries.insert(
            key,
            CacheEntry {
                value: value.clone(),
                stored: Instant::now(),
            },
        );
        Ok(value)
    }

    /// The stored answer for `key` when it is still fresh, counted as a hit or a miss.
    fn lookup(&self, key: &CacheKey, ttl: Option<u64>) -> Option<Value> {
        let _guard = perf::phase(Phase::CacheLookup);
        let mut cache = lock(&self.cache);
        let fresh = cache
            .entries
            .get(key)
            .map(|e| ttl.is_none_or(|ttl| e.stored.elapsed() < Duration::from_secs(ttl)));
        match fresh {
            Some(true) => {
                Counters::bump(&COUNTERS.cache_hits);
                return cache.entries.get(key).map(|e| e.value.clone());
            }
            Some(false) => {
                cache.entries.remove(key);
                Counters::bump(&COUNTERS.cache_evictions);
                Counters::bump(&COUNTERS.cache_misses);
            }
            None => Counters::bump(&COUNTERS.cache_misses),
        }
        None
    }

    /// The stored answer for `key` once its flight is ours: what the handler a caller waited
    /// for stored. Counted as a hit, since no handler runs for it.
    fn peek(&self, key: &CacheKey, ttl: Option<u64>) -> Option<Value> {
        let cache = lock(&self.cache);
        let value = cache
            .entries
            .get(key)
            .filter(|e| ttl.is_none_or(|ttl| e.stored.elapsed() < Duration::from_secs(ttl)))
            .map(|e| e.value.clone());
        if value.is_some() {
            Counters::bump(&COUNTERS.cache_hits);
        }
        value
    }

    /// Run the handler: counted, timed, and its writes noted.
    fn invoke(&self, ctx: &Context, id: &str, input: Value) -> Result<Value, CapabilityError> {
        Counters::bump(&COUNTERS.handler_invocations);
        self.invocations.fetch_add(1, Ordering::Relaxed);
        let value = {
            let _guard = perf::phase(Phase::HandlerExecution);
            ctx.registry.dispatch(ctx, id, input)?
        };
        // counted on the one path every transport and every execution shares, so a write
        // made from a page, a tool, a route or an execution is seen by `crate::live` alike
        if ctx
            .registry
            .get(id)
            .is_some_and(|c| c.execution.effect == crate::capability::Effect::RepositoryMutation)
        {
            self.writes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(value)
    }

    /// How many calls of a capability whose effect is a repository mutation have succeeded
    /// through this executor.
    ///
    /// A write to a tracked file moves no git control file, so a long-lived process that
    /// follows the repository by those files would go on answering from the picture it had
    /// before its own write. The executor outlives a reload, so this number is what
    /// `crate::live` compares: when it has moved, the process wrote, and the next reading
    /// is a new generation.
    ///
    /// ```
    /// use majordomus_cli::capability::executor::CapabilityExecutor;
    /// let executor = CapabilityExecutor::new();
    /// assert_eq!(executor.writes(), 0, "an executor that has run nothing has written nothing");
    /// ```
    pub fn writes(&self) -> u64 {
        self.writes.load(Ordering::SeqCst)
    }

    /// How many entries the cache holds, all capabilities together.
    pub fn cached_entries(&self) -> usize {
        lock(&self.cache).entries.len()
    }

    /// How many handlers this executor has run — its cache misses and uncached calls, never
    /// another executor's. Dropping the cache does not reset it.
    ///
    /// ```
    /// use majordomus_cli::capability::executor::CapabilityExecutor;
    /// let e = CapabilityExecutor::default();
    /// assert_eq!(e.handler_invocations(), 0);
    /// ```
    pub fn handler_invocations(&self) -> u64 {
        self.invocations.load(Ordering::Relaxed)
    }

    /// The answer this executor holds for `id` asked with `input` in the repository `ctx`
    /// reads, when its cache has one that is still fresh, and `None` otherwise. No handler
    /// runs, nothing is stored, and a miss is not counted: it is how the latest reading of a
    /// source learns that the answer for a moved repository is already at hand
    /// (`capability::readings`).
    pub fn cached(&self, ctx: &Context, id: &str, input: &Value) -> Option<Value> {
        if bypassed() {
            return None;
        }
        let policy = ctx.registry.get(id).map(|c| c.cache)?;
        let key = key_of(ctx, id, input, policy)?;
        self.peek(&key, policy.ttl_seconds())
    }

    /// How many sources this executor remembers the latest reading of
    /// (`capability::readings`).
    ///
    /// ```
    /// use majordomus_cli::capability::executor::CapabilityExecutor;
    /// assert_eq!(CapabilityExecutor::new().readings_remembered(), 0);
    /// ```
    pub fn readings_remembered(&self) -> usize {
        self.readings.remembered()
    }

    /// Drop every cached entry.
    pub fn clear(&self) {
        let mut cache = lock(&self.cache);
        cache.entries.clear();
        cache.order.clear();
    }
}

/// The key a call is kept under, or `None` when its policy keeps nothing — or keeps it per
/// checkout and the checkout has no fingerprint.
fn key_of(ctx: &Context, id: &str, input: &Value, policy: CachePolicy) -> Option<CacheKey> {
    let state = match policy {
        CachePolicy::Disabled => return None,
        CachePolicy::Process { .. } => String::new(),
        CachePolicy::Generation { .. } => format!("reading {}", ctx.reading),
        CachePolicy::Checkout { .. } => {
            let root = std::path::Path::new(&ctx.index.repository.root);
            let checkout = super::checkout::fingerprint(root)?;
            format!("reading {} checkout {checkout}", ctx.reading)
        }
    };
    Some(CacheKey {
        id: id.to_string(),
        input: canonical_json(input),
        fingerprint: ctx.registry.fingerprint().to_string(),
        state,
    })
}

fn lock(m: &Mutex<Cache>) -> std::sync::MutexGuard<'_, Cache> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The input as one canonical string: object keys sorted at every level, no whitespace,
/// so that two inputs equal as data are equal as keys whatever order a client wrote them.
///
/// ```
/// use majordomus_cli::capability::executor::canonical_json;
/// use serde_json::json;
/// assert_eq!(canonical_json(&json!({"b": 2, "a": {"d": 1, "c": [3, {"z": 0, "y": 1}]}})),
///            canonical_json(&json!({"a": {"c": [3, {"y": 1, "z": 0}], "d": 1}, "b": 2})));
/// assert_ne!(canonical_json(&json!([1, 2])), canonical_json(&json!([2, 1])), "arrays are ordered");
/// ```
pub fn canonical_json(v: &Value) -> String {
    fn write(v: &Value, out: &mut String) {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(k).unwrap_or_default());
                    out.push(':');
                    write(&m[*k], out);
                }
                out.push('}');
            }
            Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(x, out);
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut out = String::new();
    write(v, &mut out);
    out
}
