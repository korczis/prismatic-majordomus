//! The transports' own operations, benchmarked beside the capabilities: the MCP protocol
//! methods a client calls before any tool, and the HTTP projection's infrastructure
//! routes. Declared once, here, as data; the projection turns them into targets and the
//! coverage into requirements.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One system target: an operation of a transport itself, not of any capability.
///
/// A client pays for these before it reaches a single capability — `initialize` and
/// `tools/list` on MCP, the OpenAPI document and the Cockpit's pages over HTTP — so
/// leaving them out would make the benchmark a measurement of handlers and not of the
/// tool. They are declared here as data, once: the projection turns each into a target and
/// the coverage into a requirement, and neither of those names any of them.
///
/// ```
/// use majordomus_cli::bench::{SystemTarget, Transport};
/// // the set is closed and every member is declared once
/// let mut keys: Vec<&str> = SystemTarget::ALL.iter().map(|t| t.key()).collect();
/// let declared = keys.len();
/// keys.sort_unstable();
/// keys.dedup();
/// assert_eq!(keys.len(), declared, "a key is claimed twice");
/// // the transports' own operations, and no capability among them
/// assert!(SystemTarget::ALL.iter().all(|t| t.key().starts_with("system.")));
/// assert_eq!(SystemTarget::McpPing.transport(), Transport::Mcp);
/// assert_eq!(SystemTarget::HttpOpenApi.transport(), Transport::Http);
/// ```
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
    /// `GET /`: the topology as JSON, what a client that asks for no page receives.
    HttpIndex,
    /// `GET /` with `Accept: text/html`: the home page, rendered from the topology.
    HttpHome,
    /// `GET /openapi.json`.
    HttpOpenApi,
    /// `GET /swagger`.
    HttpSwagger,
    /// `GET /cockpit`: the Cockpit's landing page, rendered.
    HttpCockpitOverview,
    /// `GET /cockpit/capabilities`: the whole registry as a table, the widest page there is.
    HttpCockpitCapabilities,
    /// `GET /cockpit/graphs/registry`: a page whose content is a derived graph.
    HttpCockpitGraph,
}

impl SystemTarget {
    /// Every system target, in a stable order.
    pub const ALL: [SystemTarget; 13] = [
        SystemTarget::McpProcessCold,
        SystemTarget::McpInitialize,
        SystemTarget::McpPing,
        SystemTarget::McpToolsList,
        SystemTarget::McpResourcesList,
        SystemTarget::McpResourcesRead,
        SystemTarget::HttpIndex,
        SystemTarget::HttpHome,
        SystemTarget::HttpOpenApi,
        SystemTarget::HttpSwagger,
        SystemTarget::HttpCockpitOverview,
        SystemTarget::HttpCockpitCapabilities,
        SystemTarget::HttpCockpitGraph,
    ];

    /// The stable key results and baselines use.
    ///
    /// Stable is the whole requirement: a result document, an accepted baseline and a
    /// policy allowance all refer to a target by this string, so renaming one turns its
    /// baseline into a stale line rather than into a regression. The shape is
    /// `system.<transport>.<operation>`, which is what keeps it from colliding with a
    /// capability target's `<id>|<transport>|<case>`.
    ///
    /// ```
    /// use majordomus_cli::bench::SystemTarget;
    /// assert_eq!(SystemTarget::McpToolsList.key(), "system.mcp.tools_list");
    /// // a system key never looks like a capability target's key
    /// assert!(SystemTarget::ALL.iter().all(|t| !t.key().contains('|')));
    /// ```
    pub fn key(self) -> &'static str {
        match self {
            SystemTarget::McpProcessCold => "system.mcp.process_cold",
            SystemTarget::McpInitialize => "system.mcp.initialize",
            SystemTarget::McpPing => "system.mcp.ping",
            SystemTarget::McpToolsList => "system.mcp.tools_list",
            SystemTarget::McpResourcesList => "system.mcp.resources_list",
            SystemTarget::McpResourcesRead => "system.mcp.resources_read",
            SystemTarget::HttpIndex => "system.http.index",
            SystemTarget::HttpHome => "system.http.home",
            SystemTarget::HttpOpenApi => "system.http.openapi",
            SystemTarget::HttpSwagger => "system.http.swagger",
            SystemTarget::HttpCockpitOverview => "system.http.cockpit_overview",
            SystemTarget::HttpCockpitCapabilities => "system.http.cockpit_capabilities",
            SystemTarget::HttpCockpitGraph => "system.http.cockpit_graph",
        }
    }

    /// The transport the target belongs to.
    ///
    /// Derived from the variant rather than read off the key, so the two cannot disagree —
    /// and coverage tallies a system target under `system` rather than under this, which is
    /// why no capability module may be called `system`.
    ///
    /// ```
    /// use majordomus_cli::bench::{SystemTarget, Transport};
    /// assert_eq!(SystemTarget::McpProcessCold.transport(), Transport::Mcp);
    /// assert_eq!(SystemTarget::HttpCockpitGraph.transport(), Transport::Http);
    /// // nothing in this set is measured through the in-process executor
    /// assert!(SystemTarget::ALL.iter().all(|t| t.transport() != Transport::Direct));
    /// ```
    pub fn transport(self) -> super::Transport {
        match self {
            SystemTarget::McpProcessCold
            | SystemTarget::McpInitialize
            | SystemTarget::McpPing
            | SystemTarget::McpToolsList
            | SystemTarget::McpResourcesList
            | SystemTarget::McpResourcesRead => super::Transport::Mcp,
            SystemTarget::HttpIndex
            | SystemTarget::HttpHome
            | SystemTarget::HttpOpenApi
            | SystemTarget::HttpSwagger
            | SystemTarget::HttpCockpitOverview
            | SystemTarget::HttpCockpitCapabilities
            | SystemTarget::HttpCockpitGraph => super::Transport::Http,
        }
    }

    /// One line of what is measured, for a reader of a report.
    ///
    /// It says what the operation *is*, never how fast it was: a description is written
    /// once here and a number belongs to a run. This is the text a generated benchmark
    /// document renders, so a target's meaning is not restated beside every table.
    ///
    /// ```
    /// use majordomus_cli::bench::SystemTarget;
    /// assert!(SystemTarget::McpPing.description().contains("ping"));
    /// // every target says what it is, and each says something of its own
    /// let mut lines: Vec<&str> = SystemTarget::ALL.iter().map(|t| t.description()).collect();
    /// let declared = lines.len();
    /// lines.sort_unstable();
    /// lines.dedup();
    /// assert_eq!(lines.len(), declared);
    /// ```
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
            SystemTarget::HttpIndex => "GET / (the topology as JSON)",
            SystemTarget::HttpHome => {
                "GET / with Accept: text/html (the home page, rendered from the topology)"
            }
            SystemTarget::HttpOpenApi => "GET /openapi.json",
            SystemTarget::HttpSwagger => "GET /swagger (the Swagger UI shell)",
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
