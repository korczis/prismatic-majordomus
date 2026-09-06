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
pub mod report;
pub mod serve;
pub mod validate;

pub use model::{Availability, Mount, Provenance, Surface, SurfaceKind, Topology};

/// One route the projection answers for itself, as the OpenAPI document, the published
/// site and the Cockpit all show it: where it answers, what it is, and whether a
/// publication can carry it.
///
/// This is not a fourth list. Every entry is a surface [`discover::native`] resolved from
/// the constants that already declare it, so a prefix that moves moves here; the root
/// index is the one route no surface owns, because the application is mounted over it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectionRoute {
    /// The surface's identity in the topology.
    pub id: String,
    /// Where it answers.
    pub path: String,
    /// One line: what it is.
    pub what: String,
    /// Whether the running executable, a publication, or both answer for it.
    pub availability: Availability,
}

impl ProjectionRoute {
    /// May a published page offer this as a link a reader can follow?
    ///
    /// A page that a publication carries can link it. A page that only a running process
    /// answers cannot: on a published site that link is a promise nothing keeps, and the
    /// usual repair — a test on the page's own address — hides the rule in a template.
    ///
    /// ```
    /// use majordomus_cli::web::projection_routes;
    /// let swagger = projection_routes().into_iter().find(|r| r.id == "swagger").unwrap();
    /// assert!(!swagger.linkable(), "no server, no console");
    /// ```
    pub fn linkable(&self) -> bool {
        self.availability.is_published()
    }
}

/// The projection's own routes, in the order a reader meets them: the index first, then
/// every native surface by mount.
///
/// The index is `ServedOnly` and says so: a publication mounts its application at `/` and
/// carries no JSON index, so a published page that linked `/` expecting the index would
/// be pointing at the landing page instead.
pub fn projection_routes() -> Vec<ProjectionRoute> {
    let mut out = vec![ProjectionRoute {
        id: "index".into(),
        path: "/".into(),
        what: "The index: what this server is, and where its document, its capabilities, its peers and MCP are".into(),
        availability: Availability::ServedOnly,
    }];
    let mut native: Vec<ProjectionRoute> = discover::native(discover::Runtime::full())
        .into_iter()
        .map(|s| ProjectionRoute {
            id: s.id,
            path: s.mount.to_string(),
            what: s.title,
            availability: s.availability,
        })
        .collect();
    native.sort_by(|a, b| a.path.cmp(&b.path));
    out.extend(native);
    out
}

#[cfg(test)]
mod projection_route_tests {
    use super::*;

    fn route(id: &str) -> ProjectionRoute {
        projection_routes()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("{id} is a surface of this executable"))
    }

    #[test]
    fn the_routes_are_the_surfaces_and_not_a_list_written_beside_them() {
        let resolved: Vec<String> = discover::native(discover::Runtime::full())
            .into_iter()
            .map(|s| s.mount.to_string())
            .collect();
        let shown: Vec<String> = projection_routes()
            .into_iter()
            .filter(|r| r.id != "index")
            .map(|r| r.path)
            .collect();
        for mount in &resolved {
            assert!(
                shown.contains(mount),
                "{mount} is a surface and is not shown"
            );
        }
        assert_eq!(shown.len(), resolved.len(), "shown: {shown:?}");
    }

    #[test]
    fn a_surface_only_a_process_answers_is_not_offered_by_a_publication() {
        for id in ["swagger", "cockpit", "mcp", "api", "index"] {
            let r = route(id);
            assert_eq!(r.availability, Availability::ServedOnly, "{id}");
            assert!(
                !r.linkable(),
                "{id} would be linked from a page with no server"
            );
        }
    }

    #[test]
    fn every_route_says_what_it_is_in_the_words_of_whatever_produced_it() {
        for r in projection_routes() {
            assert!(!r.what.trim().is_empty(), "{} has no description", r.path);
            assert!(r.path.starts_with('/'), "{} is not a mount", r.path);
        }
    }

    #[test]
    fn the_order_is_the_same_on_every_run() {
        assert_eq!(projection_routes(), projection_routes());
        let paths: Vec<String> = projection_routes().into_iter().map(|r| r.path).collect();
        assert_eq!(paths.first().map(String::as_str), Some("/"));
    }
    #[test]
    fn the_one_native_route_a_publication_carries_may_be_linked() {
        // scripts/site-build copies the committed document to site/static/openapi.json and
        // site-check refuses a publication without it, so this mount is answered by both
        let openapi = route("openapi");
        assert_eq!(openapi.availability, Availability::Both);
        assert!(openapi.linkable());
    }

}
