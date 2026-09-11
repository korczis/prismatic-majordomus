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
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::perf::{self, Counters, Phase, COUNTERS};

use super::handler::{CapabilityError, Context};
use super::model::CachePolicy;

/// The one execution path of a process: counters, then the cache the capability asked
/// for, then the handler.
///
/// Shared by `Arc` between every transport and every session, so what a benchmark
/// measures and what a Cockpit button runs are the same code with the same cache behind
/// it. It holds no registry and no index of its own — those arrive on the [`Context`] of
/// each call — because the canonical state of a process is immutable and shared, and an
/// executor that owned a copy would be a second opinion about the repository.
///
/// ```
/// use majordomus_cli::capability::CapabilityExecutor;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let executor = CapabilityExecutor::new();
/// assert_eq!(executor.cached_entries(), 0);
///
/// // `capabilities.list` declares a process cache, so the second call is remembered —
/// // and answers identically, which is the entire requirement placed on the cache
/// let first = executor.execute(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
/// let second = executor.execute(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
/// assert_eq!(first, second);
/// assert_eq!(executor.cached_entries(), 1);
/// ```
#[derive(Debug, Default)]
pub struct CapabilityExecutor {
    cache: Mutex<Cache>,
}

#[derive(Debug, Default)]
struct Cache {
    entries: HashMap<CacheKey, CacheEntry>,
    /// Insertion order per capability, for eviction.
    order: HashMap<String, VecDeque<CacheKey>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    id: String,
    input: String,
    fingerprint: String,
}

#[derive(Debug)]
struct CacheEntry {
    value: Value,
    stored: Instant,
}

impl CapabilityExecutor {
    /// An executor with an empty cache.
    ///
    /// One per process, composed where the context is; there is no global to reach for.
    /// Two executors over one registry would be two caches over one collection, which is
    /// the second execution path this module exists to make impossible.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityExecutor;
    /// assert_eq!(CapabilityExecutor::new().cached_entries(), 0);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a capability by id with a JSON input. The one place a handler runs.
    ///
    /// Every caller arrives here — a transport adapter, a subcommand, the execution
    /// engine, a benchmark runner — so the counters and the capability's own cache policy
    /// apply once and cannot be forgotten by whoever adds the next transport. An id
    /// nothing declares is `NotFound` before any cache is consulted, because a cache
    /// keyed by an id that does not exist would be a cache of a mistake.
    ///
    /// ```
    /// use majordomus_cli::capability::{CapabilityError, CapabilityExecutor};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let executor = CapabilityExecutor::new();
    ///
    /// let out = executor.execute(&ctx, "repository.info", serde_json::json!({})).unwrap();
    /// assert_eq!(out["objects"], ctx.index.objects.len());
    ///
    /// let missing = executor.execute(&ctx, "no.such-capability", serde_json::json!({}));
    /// assert!(matches!(missing, Err(CapabilityError::NotFound(_))));
    /// ```
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
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityExecutor;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let executor = CapabilityExecutor::new();
    ///
    /// // a capability that declares a process cache, run the way an execution runs it:
    /// // twice, and neither call read the cache or left anything in it
    /// executor.execute_uncached(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
    /// executor.execute_uncached(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(executor.cached_entries(), 0);
    ///
    /// // the same capability through `execute` does store one
    /// executor.execute(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(executor.cached_entries(), 1);
    /// ```
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
        let key = match policy {
            CachePolicy::Disabled => None,
            CachePolicy::Process { .. } => Some(CacheKey {
                id: id.to_string(),
                input: canonical_json(&input),
                fingerprint: ctx.registry.fingerprint().to_string(),
            }),
        };
        if let (Some(key), CachePolicy::Process { ttl_seconds, .. }) = (&key, policy) {
            let _guard = perf::phase(Phase::CacheLookup);
            let mut cache = lock(&self.cache);
            let fresh = cache.entries.get(key).map(|e| {
                ttl_seconds.is_none_or(|ttl| e.stored.elapsed() < Duration::from_secs(ttl))
            });
            match fresh {
                Some(true) => {
                    Counters::bump(&COUNTERS.cache_hits);
                    return Ok(cache.entries[key].value.clone());
                }
                Some(false) => {
                    cache.entries.remove(key);
                    Counters::bump(&COUNTERS.cache_evictions);
                    Counters::bump(&COUNTERS.cache_misses);
                }
                None => Counters::bump(&COUNTERS.cache_misses),
            }
        }
        Counters::bump(&COUNTERS.handler_invocations);
        let value = {
            let _guard = perf::phase(Phase::HandlerExecution);
            ctx.registry.dispatch(ctx, id, input)?
        };
        if let (Some(key), CachePolicy::Process { max_entries, .. }) = (key, policy) {
            let mut cache = lock(&self.cache);
            let Cache { entries, order } = &mut *cache;
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
        }
        Ok(value)
    }

    /// How many entries the cache holds, all capabilities together.
    ///
    /// For a diagnostic and for a test. Nothing in an answer says whether it was computed
    /// or remembered — `project.cache-is-invisible` requires exactly that — so this is the
    /// only way to observe that the cache did anything, and it is a count of what is
    /// stored rather than of hits, which the counters in [`crate::perf`] hold.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityExecutor;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let executor = CapabilityExecutor::new();
    ///
    /// // `repository.info` declares no cache, so calling it leaves nothing behind
    /// executor.execute(&ctx, "repository.info", serde_json::json!({})).unwrap();
    /// assert_eq!(executor.cached_entries(), 0);
    /// executor.execute(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(executor.cached_entries(), 1);
    /// ```
    pub fn cached_entries(&self) -> usize {
        lock(&self.cache).entries.len()
    }

    /// Drop every cached entry of every capability.
    ///
    /// Not a way of keeping the cache honest: an entry cannot go stale, because the
    /// registry fingerprint is part of its key and a repository that changed produces a
    /// different key rather than a wrong answer. It exists for the benchmark runner, which
    /// clears between samples so that timing a cached capability measures its handler
    /// rather than a `HashMap` lookup.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityExecutor;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let executor = CapabilityExecutor::new();
    /// executor.execute(&ctx, "capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(executor.cached_entries(), 1);
    /// executor.clear();
    /// assert_eq!(executor.cached_entries(), 0);
    /// ```
    pub fn clear(&self) {
        let mut cache = lock(&self.cache);
        cache.entries.clear();
        cache.order.clear();
    }
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
