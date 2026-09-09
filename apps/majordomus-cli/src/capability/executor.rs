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

/// The executor of one registry.
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
    pub fn cached_entries(&self) -> usize {
        lock(&self.cache).entries.len()
    }

    /// Drop every cached entry.
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
