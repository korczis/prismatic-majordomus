//! The `graph` module: the derived graphs of the repository, listed and read. The
//! derivations live in [`crate::graph`]; this file is the declaration that turns them
//! into an MCP tool, an HTTP route, an OpenAPI operation and a benchmark target, exactly
//! like every other capability.
//!
//! No graph is authored. A renderer — the Cockpit's Cytoscape view, a Mermaid block, a
//! terminal — is a consumer of the canonical node and edge model and never a second
//! definition of it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::graph::{self, Graph, GraphInfo};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which `graph.list` is read as an MCP resource.
pub const GRAPHS_URI: &str = "majordomus://graphs";

/// Every graph this executable derives.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphList {
    /// How many.
    pub count: usize,
    /// Each, described but not derived.
    pub graphs: Vec<GraphInfo>,
}

/// Which graph to derive.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphInput {
    /// The graph's id, as `graph.list` gives it (`registry`, `layer`, `rules`, `adrs`,
    /// `use-cases`).
    pub id: String,
}

impl BenchmarkCases for GraphInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // one case per graph, so every derivation is timed and every derivation appears
        // as an example in the OpenAPI document
        graph::ids()
            .into_iter()
            .map(|id| NamedCase::new(id, GraphInput { id: id.into() }))
            .collect()
    }
}

fn graph_list(ctx: &Context, _: Empty) -> Result<GraphList, CapabilityError> {
    let graphs = graph::list(&ctx.registry, &ctx.index);
    Ok(GraphList {
        count: graphs.len(),
        graphs,
    })
}

fn graph_get(ctx: &Context, input: GraphInput) -> Result<Graph, CapabilityError> {
    graph::derive(&input.id, &ctx.registry, &ctx.index).ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no graph '{}'; this executable derives {}",
            input.id,
            graph::ids().join(", ")
        ))
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "graph",
        title: "Graphs",
        description: "The graphs derived from the registry and the index: the executable's own capability registry, the shape of the layer, the rule dependencies, the decisions and what they put in force, and the use cases and what they exercise. Canonical nodes and edges; a rendering library is a consumer, never the shape.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "graph.list",
                title: "List graphs",
                description: "Every graph this executable derives: its id, what it shows, and what it is derived from.",
                input: Empty,
                output: GraphList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_graphs".into()),
                        resource: Some(McpResource { uri: GRAPHS_URI.into(), name: "graphs".into() }),
                    }),
                    http: get("/api/v1/graphs"),
                    cli: None,
                },
                tags: ["graph", "introspection"],
                handler: graph_list,
            },
            capability! {
                id: "graph.get",
                title: "Derive one graph",
                description: "One graph by id: its nodes and edges with the vocabularies that say what each kind means, the file every node was derived from, and whether the result is acyclic. Deterministic for a given tree and executable.",
                input: GraphInput,
                output: Graph,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_graph"),
                    http: get("/api/v1/graph"),
                    cli: None,
                },
                tags: ["graph"],
                cache: CachePolicy::Process { max_entries: 16, ttl_seconds: None },
                handler: graph_get,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the benchmark target — is derived from
    /// it. A refactor that dropped an exposure or renamed a route would still compile, and
    /// the suites that exercise the behaviour behind it would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "graph");
        let expected: &[(&str, &str, &str)] = &[
            ("graph.list", "majordomus_graphs", "/api/v1/graphs"),
            ("graph.get", "majordomus_graph", "/api/v1/graph"),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
    }
}
