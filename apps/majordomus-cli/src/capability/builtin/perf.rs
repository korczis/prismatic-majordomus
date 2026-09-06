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

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place the id and the two projection names exist. A
    /// refactor that dropped an exposure would still compile, and every suite that tests
    /// the counters themselves would still pass; this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "perf");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["perf.counters"]);
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_perf")
        );
        assert_eq!(
            exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/perf")
        );
    }
}
