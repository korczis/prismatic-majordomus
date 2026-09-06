//! The transports' own operations, benchmarked beside the capabilities: the MCP protocol
//! methods a client calls before any tool, and the HTTP projection's infrastructure
//! routes. Declared once, here, as data; the projection turns them into targets and the
//! coverage into requirements.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One system target.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SystemTarget {
    /// A fresh `majordomus mcp` process: spawn, `initialize`, first `tools/list`.
    McpProcessCold,
    /// `initialize` on a running process.
    McpInitialize,
    /// `ping`.
    McpPing,
    /// `tools/list`.
    McpToolsList,
    /// `resources/list`.
    McpResourcesList,
    /// `resources/read` of the first declarative resource.
    McpResourcesRead,
    /// `GET /`.
    HttpIndex,
    /// `GET /openapi.json`.
    HttpOpenApi,
    /// `GET /docs`.
    HttpDocs,
    /// `GET /cockpit`: the Cockpit's landing page, rendered.
    HttpCockpitOverview,
    /// `GET /cockpit/capabilities`: the whole registry as a table, the widest page there is.
    HttpCockpitCapabilities,
    /// `GET /cockpit/graphs/registry`: a page whose content is a derived graph.
    HttpCockpitGraph,
}

impl SystemTarget {
    /// Every system target, in a stable order.
    pub const ALL: [SystemTarget; 12] = [
        SystemTarget::McpProcessCold,
        SystemTarget::McpInitialize,
        SystemTarget::McpPing,
        SystemTarget::McpToolsList,
        SystemTarget::McpResourcesList,
        SystemTarget::McpResourcesRead,
        SystemTarget::HttpIndex,
        SystemTarget::HttpOpenApi,
        SystemTarget::HttpDocs,
        SystemTarget::HttpCockpitOverview,
        SystemTarget::HttpCockpitCapabilities,
        SystemTarget::HttpCockpitGraph,
    ];

    /// The stable key results and baselines use.
    pub fn key(self) -> &'static str {
        match self {
            SystemTarget::McpProcessCold => "system.mcp.process_cold",
            SystemTarget::McpInitialize => "system.mcp.initialize",
            SystemTarget::McpPing => "system.mcp.ping",
            SystemTarget::McpToolsList => "system.mcp.tools_list",
            SystemTarget::McpResourcesList => "system.mcp.resources_list",
            SystemTarget::McpResourcesRead => "system.mcp.resources_read",
            SystemTarget::HttpIndex => "system.http.index",
            SystemTarget::HttpOpenApi => "system.http.openapi",
            SystemTarget::HttpDocs => "system.http.docs",
            SystemTarget::HttpCockpitOverview => "system.http.cockpit_overview",
            SystemTarget::HttpCockpitCapabilities => "system.http.cockpit_capabilities",
            SystemTarget::HttpCockpitGraph => "system.http.cockpit_graph",
        }
    }

    /// The transport the target belongs to.
    pub fn transport(self) -> super::Transport {
        match self {
            SystemTarget::McpProcessCold
            | SystemTarget::McpInitialize
            | SystemTarget::McpPing
            | SystemTarget::McpToolsList
            | SystemTarget::McpResourcesList
            | SystemTarget::McpResourcesRead => super::Transport::Mcp,
            SystemTarget::HttpIndex
            | SystemTarget::HttpOpenApi
            | SystemTarget::HttpDocs
            | SystemTarget::HttpCockpitOverview
            | SystemTarget::HttpCockpitCapabilities
            | SystemTarget::HttpCockpitGraph => super::Transport::Http,
        }
    }

    /// One line of what is measured.
    pub fn description(self) -> &'static str {
        match self {
            SystemTarget::McpProcessCold => {
                "spawn a majordomus mcp process, initialize, first tools/list"
            }
            SystemTarget::McpInitialize => "initialize on a running process",
            SystemTarget::McpPing => "ping: the protocol round trip with nothing behind it",
            SystemTarget::McpToolsList => "tools/list",
            SystemTarget::McpResourcesList => "resources/list",
            SystemTarget::McpResourcesRead => "resources/read of the first declarative resource",
            SystemTarget::HttpIndex => "GET /",
            SystemTarget::HttpOpenApi => "GET /openapi.json",
            SystemTarget::HttpDocs => "GET /docs (the Swagger UI shell)",
            SystemTarget::HttpCockpitOverview => {
                "GET /cockpit (the Cockpit's landing page, server-rendered)"
            }
            SystemTarget::HttpCockpitCapabilities => {
                "GET /cockpit/capabilities (every capability as a table: the widest page)"
            }
            SystemTarget::HttpCockpitGraph => {
                "GET /cockpit/graphs/registry (a page whose content is a derived graph)"
            }
        }
    }
}
