//! The `perf` module: this process's performance counters, readable over every
//! transport so that a test or a person can prove, after any number of requests, that no
//! request rebuilt canonical state.

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::perf::{CounterSnapshot, COUNTERS};
use crate::{capability, module};

use super::{get, mcp, Empty};

fn perf_counters(_: &Context, _: Empty) -> Result<CounterSnapshot, CapabilityError> {
    Ok(COUNTERS.snapshot())
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "perf",
        title: "Performance",
        description: "This process's work counters and phase timings: what happened once at startup and what happens per call, for the structural tests and the benchmark evidence.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "perf.counters",
                title: "Performance counters",
                description: "The counters of this process: repository scans, index and registry builds, schema generations, projection builds, executions, handler invocations, cache hits, misses and evictions, and the phase timings, as they stand now.",
                input: Empty,
                output: CounterSnapshot,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_perf"), http: get("/api/v1/perf"), cli: None },
                tags: ["performance", "introspection"],
                handler: perf_counters,
            },
        ],
    }
}
