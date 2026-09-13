//! The benchmark projection: every externally callable operation of the registry, as a
//! target, derived and never listed by hand. A capability with a required benchmark
//! policy is a target directly and through every transport its exposure declares; the
//! inputs come from its input type's `BenchmarkCases`; the transports' own operations
//! (`initialize`, `tools/list`, `/openapi.json`, ...) are system targets declared once in
//! `system`. Coverage is `covered / required` with a generated denominator; the runners
//! time targets through the executor, a real loopback socket and a real child process;
//! the results are a versioned document; the accepted baseline is compared against a
//! policy that is data.
//!
//! "Derived and never listed by hand" is the load-bearing claim, and it is checkable
//! without running a benchmark: what owes one is read off the descriptors, the transports
//! a capability is timed over are the ones its own exposure declares, and the transports'
//! own operations are a separate namespace that no capability can occupy.
//!
//! ```
//! use majordomus_cli::bench::{SystemTarget, Transport};
//! use majordomus_cli::capability::{builtin, BenchmarkPolicy, CapabilityRegistry};
//!
//! let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
//!
//! // The requirement is a property of each descriptor. Nothing enumerates the subjects.
//! let required: Vec<&str> = registry
//!     .iter()
//!     .filter(|c| c.benchmark == BenchmarkPolicy::Required)
//!     .map(|c| c.id.as_str())
//!     .collect();
//! assert!(!required.is_empty());
//!
//! // Each of them is a target on the transports it is exposed over — never on three by
//! // default, so the denominator cannot be inflated by capabilities nobody can reach.
//! let over_mcp = registry
//!     .iter()
//!     .filter(|c| c.benchmark == BenchmarkPolicy::Required)
//!     .filter(|c| c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()))
//!     .count();
//! assert!(over_mcp <= required.len());
//!
//! // And the transports' own targets are not capabilities: no id collides with a key.
//! assert!(SystemTarget::ALL.iter().all(|t| registry.get(t.key()).is_none()));
//! assert!(SystemTarget::ALL.iter().all(|t| t.transport() != Transport::Direct));
//! ```

pub mod baseline;
pub(crate) mod coverage;
pub(crate) mod projection;
pub mod results;
pub(crate) mod runner;
pub(crate) mod stats;
pub(crate) mod system;

pub use coverage::{Coverage, CoverageLine, CoverageState};
pub use projection::{BenchmarkProjection, BenchmarkTarget, TargetKind, Transport};
pub use results::{BenchmarkResult, ResultDocument, RESULT_SCHEMA};
pub use runner::{Profile, Runner};
pub use stats::Statistics;
pub use system::SystemTarget;
