//! The benchmark projection: every externally callable operation of the registry, as a
//! target, derived and never listed by hand. A capability with a required benchmark
//! policy is a target directly and through every transport its exposure declares; the
//! inputs come from its input type's `BenchmarkCases`; the transports' own operations
//! (`initialize`, `tools/list`, `/openapi.json`, ...) are system targets declared once in
//! [`system`]. Coverage is `covered / required` with a generated denominator; the runners
//! time targets through the executor, a real loopback socket and a real child process;
//! the results are a versioned document; the accepted baseline is compared against a
//! policy that is data.

pub mod baseline;
pub mod coverage;
pub mod projection;
pub mod results;
pub mod runner;
pub mod stats;
pub mod system;

pub use coverage::{Coverage, CoverageLine, CoverageState};
pub use projection::{BenchmarkProjection, BenchmarkTarget, TargetKind, Transport};
pub use results::{BenchmarkResult, ResultDocument, RESULT_SCHEMA};
pub use runner::{Profile, Runner};
pub use stats::Statistics;
pub use system::SystemTarget;
