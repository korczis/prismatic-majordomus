//! The repository's web surfaces: one model, discovered once, read by everything that
//! serves, publishes, validates or describes them.
//!
//! A *surface* is anything this repository exposes over HTTP — the application's site, the
//! capability routes, the Swagger UI, the Cockpit, a generated test or benchmark report.
//! Before this module each of those was known separately by the router, the publication
//! script, the gate model and the documentation, and a new one cost a registration in each.
//! Here it is discovered from its producer ([`discover`]), checked against the invariants a
//! topology must satisfy ([`validate`]), composed into a publishable tree (`compose`) and
//! written out for diagnostics (`manifest`) — all from the same [`model::Topology`].
//!
//! The decision and what it rejected are in `.ai/repo/adrs/0013-*.md`.
//!
//! ```
//! use majordomus_cli::web::{discover, model::Mount};
//! // the executable's own routes come from the constants that already declare them
//! let native = discover::native(discover::Runtime::full());
//! assert!(native.iter().any(|s| s.mount == Mount::parse("/swagger").unwrap()));
//! ```

pub(crate) mod compose;
pub mod discover;
pub mod files;
pub(crate) mod home;
pub mod html;
pub(crate) mod manifest;
pub mod model;
pub mod report;
pub mod serve;
pub mod validate;

pub use model::{
    Availability, Category, Feature, Mount, Provenance, Surface, SurfaceKind, Topology, Visibility,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_discovery_answers_the_router_the_home_page_and_a_publication() {
        // the claim this module exists to make: serving, publishing and describing are three
        // readings of one topology, so none of them can know a surface the others do not
        let topology = Topology::new(discover::native(discover::Runtime::full()));

        let served = topology.served(discover::Runtime::full());
        let published = topology.published();
        for id in served.ids().into_iter().chain(published.ids()) {
            assert!(
                topology.get(id).is_some(),
                "{id} is in a reading and not in the topology it was read from"
            );
        }
        // and the reserved namespaces stay apart: /swagger is the API viewer, /docs is the
        // repository's documentation, and one owner answers each path
        let swagger = topology
            .owner("/swagger")
            .expect("something owns the Swagger UI path");
        assert_eq!(swagger.id, "swagger");
        assert!(
            topology.owner("/docs").is_none_or(|s| s.id != "swagger"),
            "the API viewer does not own the documentation namespace"
        );
    }

    #[test]
    fn narrowing_a_topology_can_only_take_surfaces_away() {
        // a projection serving a subset filters the value the process already resolved; a
        // narrowing that could add one would be a second discovery
        let full = Topology::new(discover::native(discover::Runtime::full()));
        let bare = Topology::new(discover::native(discover::Runtime::default()));
        for id in bare.ids() {
            assert!(full.get(id).is_some(), "{id} appeared out of a narrowing");
        }
        assert!(bare.ids().len() <= full.ids().len());
    }

    #[test]
    fn a_resolved_topology_has_one_owner_for_every_path_it_claims() {
        let topology = Topology::new(discover::native(discover::Runtime::full()));
        let findings = validate::validate(
            &topology,
            std::path::Path::new("/nonexistent"),
            validate::Artifacts::Ignore,
        );
        assert!(findings.is_empty(), "{findings:?}");
    }
}
