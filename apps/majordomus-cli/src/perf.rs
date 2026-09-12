//! Performance truth: process-wide counters of the work that must happen once (a
//! repository scan, an index build, a registry build, a schema generation, a projection
//! build) and of the work that happens per call (executions, handler invocations, cache
//! hits and misses), plus phase timings on a monotonic clock. They are what the
//! structural tests read after hundreds of requests to prove that no request rebuilt
//! canonical state, what `perf.counters` answers over every transport, and what the
//! benchmark evidence carries beside its latencies. Atomics only: reading them costs
//! nothing worth measuring, and nothing here allocates on a hot path.
//!
//! # What the counters are for
//!
//! They exist to make one class of regression visible that no functional test can see: a
//! change that makes every request correct and rebuilds the index to get there. The
//! startup counters must stay at their post-startup values however many requests arrive,
//! and `tests/hot_path.rs` asserts exactly that. `project.hot-path-reads-once` is the rule.
//!
//! ```
//! use majordomus_cli::perf::{Counters, Phase, COUNTERS};
//!
//! // a phase is timed by a guard, so the timing ends when the work does and not later
//! let before = COUNTERS.snapshot();
//! {
//!     let _timing = majordomus_cli::perf::phase(Phase::IndexBuild);
//!     Counters::bump(&COUNTERS.index_builds);
//! }
//! let after = COUNTERS.snapshot();
//! assert_eq!(after.index_builds, before.index_builds + 1);
//!
//! // and what happened once at startup is reported apart from what happens per call
//! assert!(after.startup_work().iter().any(|(name, _)| *name == "index_builds"));
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The counters of this process.
pub static COUNTERS: Counters = Counters::new();

/// One counter per unit of work. Each is incremented at exactly one place in the code,
/// named in its doc comment.
#[derive(Debug)]
pub struct Counters {
    /// `discovery::discover`: one enumeration of the repository's declared sources.
    pub repository_scans: AtomicU64,
    /// `Index::build`.
    pub index_builds: AtomicU64,
    /// `CapabilityRegistry::builder().build()`.
    pub registry_builds: AtomicU64,
    /// `CanonicalSchema::of`: one JSON Schema derived from a Rust type.
    pub schema_generations: AtomicU64,
    /// `Surface`'s tool and resource listings computed (once per shared listing).
    pub mcp_projection_builds: AtomicU64,
    /// `openapi::document`.
    pub openapi_builds: AtomicU64,
    /// `Router::new`.
    pub http_projection_builds: AtomicU64,
    /// `graph::derive`: one graph derived from the registry and the index.
    pub graph_builds: AtomicU64,
    /// `CapabilityExecutor::execute`: every call through every transport.
    pub executions: AtomicU64,
    /// Handlers actually run (an execution the cache did not answer), at every depth: a
    /// handler that composes another capability through the executor counts both, which
    /// is how composition is seen to use the one execution path rather than reach into
    /// another module behind it. So `handler_invocations` equals `executions` minus
    /// `cache_hits`, and an execution that ran a handler twice is exactly what breaks it.
    pub handler_invocations: AtomicU64,
    /// Executions answered from the cache.
    pub cache_hits: AtomicU64,
    /// Executions of a cached capability that ran the handler.
    pub cache_misses: AtomicU64,
    /// Entries dropped to keep a capability under its `max_entries`, or past their TTL.
    pub cache_evictions: AtomicU64,
    phases: [PhaseCell; Phase::ALL.len()],
}

#[derive(Debug)]
struct PhaseCell {
    count: AtomicU64,
    nanos: AtomicU64,
}

impl PhaseCell {
    const fn new() -> Self {
        PhaseCell {
            count: AtomicU64::new(0),
            nanos: AtomicU64::new(0),
        }
    }
}

/// The phases timed. Each has one owner in the code, named in its doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// `Repository::discover`: finding the root and reading the manifest.
    RepositoryDiscovery,
    /// `Index::build`: enumeration, reading, validation of every declared file.
    IndexBuild,
    /// `CapabilityRegistry` build and validation.
    RegistryBuild,
    /// `openapi::document`.
    OpenApiBuild,
    /// The MCP tool and resource listings.
    McpProjectionBuild,
    /// `graph::derive`: deriving one graph.
    GraphBuild,
    /// A handler running inside the executor.
    HandlerExecution,
    /// A cache lookup inside the executor, hit or miss.
    CacheLookup,
}

impl Phase {
    /// Every phase, in declaration order.
    pub const ALL: [Phase; 8] = [
        Phase::RepositoryDiscovery,
        Phase::IndexBuild,
        Phase::RegistryBuild,
        Phase::OpenApiBuild,
        Phase::McpProjectionBuild,
        Phase::GraphBuild,
        Phase::HandlerExecution,
        Phase::CacheLookup,
    ];

    /// The name as serialised.
    pub fn name(self) -> &'static str {
        match self {
            Phase::RepositoryDiscovery => "repository_discovery",
            Phase::IndexBuild => "index_build",
            Phase::RegistryBuild => "registry_build",
            Phase::OpenApiBuild => "open_api_build",
            Phase::McpProjectionBuild => "mcp_projection_build",
            Phase::GraphBuild => "graph_build",
            Phase::HandlerExecution => "handler_execution",
            Phase::CacheLookup => "cache_lookup",
        }
    }
}

/// Times one phase from creation to drop.
pub struct PhaseGuard {
    phase: Phase,
    start: Instant,
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        let cell = &COUNTERS.phases[self.phase as usize];
        cell.count.fetch_add(1, Ordering::Relaxed);
        cell.nanos
            .fetch_add(self.start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

/// Time a phase until the guard drops.
///
/// ```
/// use majordomus_cli::perf::{self, Phase};
/// let before = perf::COUNTERS.snapshot().phases["cache_lookup"].count;
/// {
///     let _guard = perf::phase(Phase::CacheLookup);
/// }
/// assert_eq!(perf::COUNTERS.snapshot().phases["cache_lookup"].count, before + 1);
/// ```
pub fn phase(phase: Phase) -> PhaseGuard {
    PhaseGuard {
        phase,
        start: Instant::now(),
    }
}

impl Counters {
    const fn new() -> Self {
        Counters {
            repository_scans: AtomicU64::new(0),
            index_builds: AtomicU64::new(0),
            registry_builds: AtomicU64::new(0),
            schema_generations: AtomicU64::new(0),
            mcp_projection_builds: AtomicU64::new(0),
            openapi_builds: AtomicU64::new(0),
            http_projection_builds: AtomicU64::new(0),
            graph_builds: AtomicU64::new(0),
            executions: AtomicU64::new(0),
            handler_invocations: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            phases: [
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
                PhaseCell::new(),
            ],
        }
    }

    /// Add one to a counter.
    #[inline]
    pub fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// The counters and phases as they stand.
    pub fn snapshot(&self) -> CounterSnapshot {
        let read = |c: &AtomicU64| c.load(Ordering::Relaxed);
        CounterSnapshot {
            repository_scans: read(&self.repository_scans),
            index_builds: read(&self.index_builds),
            registry_builds: read(&self.registry_builds),
            schema_generations: read(&self.schema_generations),
            mcp_projection_builds: read(&self.mcp_projection_builds),
            openapi_builds: read(&self.openapi_builds),
            http_projection_builds: read(&self.http_projection_builds),
            graph_builds: read(&self.graph_builds),
            executions: read(&self.executions),
            handler_invocations: read(&self.handler_invocations),
            cache_hits: read(&self.cache_hits),
            cache_misses: read(&self.cache_misses),
            cache_evictions: read(&self.cache_evictions),
            phases: Phase::ALL
                .iter()
                .map(|p| {
                    let cell = &self.phases[*p as usize];
                    (
                        p.name().to_string(),
                        PhaseTotals {
                            count: read(&cell.count),
                            total_nanos: read(&cell.nanos),
                        },
                    )
                })
                .collect(),
        }
    }
}

/// A phase's totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PhaseTotals {
    /// How many times the phase ran.
    pub count: u64,
    /// Nanoseconds spent in it, summed.
    pub total_nanos: u64,
}

/// The counters and phase totals of this process at one moment. Every value is a count
/// or a duration this process measured; none is written anywhere by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CounterSnapshot {
    /// Enumerations of the repository's declared sources.
    pub repository_scans: u64,
    /// Index builds.
    pub index_builds: u64,
    /// Registry builds.
    pub registry_builds: u64,
    /// JSON Schemas derived from Rust types.
    pub schema_generations: u64,
    /// MCP tool and resource listings computed.
    pub mcp_projection_builds: u64,
    /// OpenAPI documents built.
    pub openapi_builds: u64,
    /// HTTP routers built.
    pub http_projection_builds: u64,
    /// Graphs derived from the registry and the index.
    pub graph_builds: u64,
    /// Calls through the executor, every transport.
    pub executions: u64,
    /// Handlers actually run.
    pub handler_invocations: u64,
    /// Executions answered from the cache.
    pub cache_hits: u64,
    /// Executions of a cached capability that ran the handler.
    pub cache_misses: u64,
    /// Cache entries dropped.
    pub cache_evictions: u64,
    /// Phase totals by phase name.
    pub phases: std::collections::BTreeMap<String, PhaseTotals>,
}

impl CounterSnapshot {
    /// The counters that must not move once a process serves: a request that moves one of
    /// them rebuilt canonical state.
    pub fn startup_work(&self) -> [(&'static str, u64); 7] {
        [
            ("repository_scans", self.repository_scans),
            ("index_builds", self.index_builds),
            ("registry_builds", self.registry_builds),
            ("schema_generations", self.schema_generations),
            ("mcp_projection_builds", self.mcp_projection_builds),
            ("openapi_builds", self.openapi_builds),
            ("http_projection_builds", self.http_projection_builds),
        ]
    }
}
