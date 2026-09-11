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
//! The vocabulary is derived, so it can be read without measuring anything: a target is a
//! capability crossed with a transport and a case, a system target is one of the
//! transports' own operations, and a key is what a result and a baseline both refer to.
//!
//! ```
//! use majordomus_cli::bench::{SystemTarget, Transport};
//! // the transports are a closed set, and each has one spelling
//! assert_eq!(Transport::ALL.len(), 3);
//! for transport in Transport::ALL {
//!     assert_eq!(Transport::parse(transport.name()), Some(transport));
//! }
//!
//! // every system target belongs to the transport its key names, so a key cannot be
//! // filed under the wrong denominator
//! for target in SystemTarget::ALL {
//!     let prefix = format!("system.{}.", target.transport().name());
//!     assert!(target.key().starts_with(&prefix), "{} vs {prefix}", target.key());
//!     assert!(!target.description().is_empty());
//! }
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
