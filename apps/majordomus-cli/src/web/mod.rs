//! The repository's web surfaces: one model, discovered once, read by everything that
//! serves, publishes, validates or describes them.
//!
//! A *surface* is anything this repository exposes over HTTP — the application's site, the
//! capability routes, the Swagger UI, the Cockpit, a generated test or benchmark report.
//! Before this module each of those was known separately by the router, the publication
//! script, the gate model and the documentation, and a new one cost a registration in each.
//! Here it is discovered from its producer ([`discover`]), checked against the invariants a
//! topology must satisfy ([`validate`]), composed into a publishable tree ([`compose`]) and
//! written out for diagnostics ([`manifest`]) — all from the same [`model::Topology`].
//!
//! The decision and what it rejected are in `.ai/repo/adrs/0013-*.md`.
//!
//! ```
//! use majordomus_cli::web::{discover, model::Mount};
//! // the executable's own routes come from the constants that already declare them
//! let native = discover::native(discover::Runtime::full());
//! assert!(native.iter().any(|s| s.mount == Mount::parse("/docs").unwrap()));
//! ```

pub mod compose;
pub mod discover;
pub mod manifest;
pub mod model;
pub mod validate;

pub use model::{Availability, Mount, Provenance, Surface, SurfaceKind, Topology};
