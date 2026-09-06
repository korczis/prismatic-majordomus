//! The `web` module: the surfaces this repository exposes over HTTP, answered from the
//! resolution the process is already serving from.
//!
//! This is the machine-readable half of the home page, and deliberately not a second
//! answer: both read `ctx.web`, which is resolved once per process, and the HTTP router
//! hands its capability calls a context narrowed to exactly what it serves. Asking this
//! capability over a served socket therefore describes that server; asking it on the
//! command line or over a standalone MCP session describes the repository, whose surfaces
//! carry the availability and the feature that say which process would serve them.
//!
//! Nothing here knows the name of a surface. A report that a producer generates tomorrow
//! is in this answer the moment it declares itself, and a route that is removed leaves it.
//!
//! ```
//! use majordomus_cli::capability::builtin::web::module;
//! // the module declares its capability once; every projection of it is derived
//! let m = module();
//! let c = m.capabilities.iter().find(|c| c.capability.id.as_str() == "web.surfaces").unwrap();
//! assert_eq!(c.capability.exposure.http.as_ref().unwrap().path, "/api/v1/web/surfaces");
//! assert_eq!(c.capability.exposure.mcp.as_ref().unwrap().tool.as_deref(), Some("majordomus_web_surfaces"));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::web::validate::{self, Artifacts, Finding};
use crate::web::{Surface, Visibility};
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which the topology is read as an MCP resource.
pub const SURFACES_URI: &str = "majordomus://web";

/// The web surfaces of this repository, as the process that answers resolved them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SurfaceReport {
    /// Every resolved surface, in route-precedence order: the order a router consults
    /// them in, so the first whose mount owns a path is the one that answers it.
    pub surfaces: Vec<Surface>,
    /// The ids a process answers for, in the same order. A surface published and not
    /// served is absent here.
    pub served: Vec<String>,
    /// The ids a publication holds, in the same order.
    pub published: Vec<String>,
    /// The ids offered to a person, which is what the home page lists.
    pub public: Vec<String>,
    /// What validation says about the topology as it stands. Empty is the healthy answer;
    /// a finding here is the same one `majordomus web validate` reports, with its remedy.
    pub findings: Vec<Finding>,
}

fn surfaces(ctx: &Context, _: Empty) -> Result<SurfaceReport, CapabilityError> {
    Ok(report(
        &ctx.web,
        std::path::Path::new(&ctx.index.repository.root),
    ))
}

/// The report of one resolved topology.
///
/// Separate from the handler because it is the whole of the answer: given a topology and
/// the root its artifacts are relative to, everything below is a reading of that value and
/// nothing is asked of the process.
fn report(topology: &crate::web::Topology, root: &std::path::Path) -> SurfaceReport {
    let ids = |take: &dyn Fn(&Surface) -> bool| -> Vec<String> {
        topology
            .surfaces
            .iter()
            .filter(|s| take(s))
            .map(|s| s.id.clone())
            .collect()
    };
    SurfaceReport {
        served: ids(&|s| s.availability.is_served()),
        published: ids(&|s| s.publishes()),
        public: ids(&|s| s.visibility == Visibility::Public),
        findings: validate::validate(topology, root, Artifacts::Ignore),
        surfaces: topology.surfaces.clone(),
    }
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "web",
        title: "Web surfaces",
        description: "What this repository exposes over HTTP, resolved from the producers that make it rather than from a register anybody maintains: the routes the executable answers itself, the documentation build, and every generated report that declared its own mount. The same resolution serves the router, renders the home page and composes a publication.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "web.surfaces",
                title: "Every web surface, resolved",
                description: "The web topology in route-precedence order, with each surface's mount, category, visibility, kind, producer, artifact, runtime feature and the provenance of every value a reader could be surprised by; and which ids are served, published and offered to a person. Answered from the resolution this process serves from, so it cannot disagree with what the router routes or what the home page lists.",
                input: Empty,
                output: SurfaceReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_web_surfaces".into()),
                        resource: Some(McpResource { uri: SURFACES_URI.into(), name: "web".into() }),
                    }),
                    http: get("/api/v1/web/surfaces"),
                    cli: None,
                },
                tags: ["web", "introspection"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(5) },
                handler: surfaces,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::discover::{self, Runtime};
    use crate::web::Topology;

    #[test]
    fn the_report_is_three_readings_of_one_topology_and_invents_nothing() {
        let mut surfaces = discover::native_all();
        surfaces.extend(discover::application(std::path::Path::new("/nonexistent")));
        let topology = Topology::new(surfaces);
        let report = report(&topology, std::path::Path::new("/nonexistent"));

        assert_eq!(report.surfaces.len(), topology.surfaces.len());
        // every id in every list is a surface of the topology: no list is maintained
        for id in report
            .served
            .iter()
            .chain(&report.published)
            .chain(&report.public)
        {
            assert!(
                topology.get(id).is_some(),
                "{id} is in a list and not in the topology"
            );
        }
        assert!(report.served.contains(&"swagger".to_string()));
        assert!(report.public.contains(&"swagger".to_string()));
        // MCP is served and not advertised; the home page reads `public`, not `served`
        assert!(report.served.contains(&"mcp".to_string()));
        assert!(!report.public.contains(&"mcp".to_string()));
        assert!(report.findings.is_empty(), "{:?}", report.findings);
    }

    #[test]
    fn a_narrowed_topology_answers_for_the_process_that_narrowed_it() {
        let bare = Topology::new(discover::native(Runtime::default()));
        let report = report(&bare, std::path::Path::new("/nonexistent"));
        assert!(!report.served.contains(&"cockpit".to_string()));
        assert!(!report.served.contains(&"mcp".to_string()));
        assert!(report.published.is_empty(), "a process publishes nothing");
    }
}
