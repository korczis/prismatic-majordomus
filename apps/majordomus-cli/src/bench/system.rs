//! The transports' own operations, benchmarked beside the capabilities: the MCP protocol
//! methods a client calls before any tool, and the HTTP projection's infrastructure
//! routes. Declared once, here, as data; the projection turns them into targets and the
//! coverage into requirements.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One system target: an operation a transport performs for itself, which no capability
/// declares and which every client pays for anyway.
///
/// A client that calls one tool has already paid for `initialize` and `tools/list`; a
/// browser that opens one page has already paid for `GET /`. Timing only the capabilities
/// would report a protocol as fast while the first useful answer is a second away, so the
/// transports' own operations are declared here and timed beside them.
///
/// Each one's [`key`](SystemTarget::key) is namespaced `system.<transport>.<name>` and its
/// [`transport`](SystemTarget::transport) agrees with that namespace. That is what keeps
/// these out of the per-capability denominators: coverage tallies a line under the
/// `system` bucket precisely when its module is
/// [`SYSTEM_MODULE`](crate::bench::coverage::SYSTEM_MODULE).
///
/// ```
/// use majordomus_cli::bench::SystemTarget;
///
/// for target in SystemTarget::ALL {
///     let (prefix, _) = target.key().split_once('.').unwrap();
///     assert_eq!(prefix, "system", "{} is not namespaced", target.key());
///     assert!(
///         target.key().starts_with(&format!("system.{}.", target.transport().name())),
///         "{} claims a transport its key does not name",
///         target.key()
///     );
/// }
///
/// // Six of the thirteen are protocol methods an MCP client calls before any tool.
/// let mcp = SystemTarget::ALL
///     .iter()
///     .filter(|t| t.transport() == majordomus_cli::bench::Transport::Mcp)
///     .count();
/// assert_eq!(mcp, 6);
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
    /// It is stable in the sense that matters to a ratchet: renaming a variant is free,
    /// changing its key retires a baseline line. A key the run no longer produces is
    /// reported by a check as `STALE` rather than compared against something else, so the
    /// cost of changing one is visible instead of silent.
    ///
    /// ```
    /// use majordomus_cli::bench::SystemTarget;
    /// assert_eq!(SystemTarget::McpProcessCold.key(), "system.mcp.process_cold");
    /// assert_eq!(SystemTarget::HttpOpenApi.key(), "system.http.openapi");
    ///
    /// // Every key is distinct: two targets sharing one would have their samples
    /// // compared against a single baseline line.
    /// let mut keys: Vec<&str> = SystemTarget::ALL.iter().map(|t| t.key()).collect();
    /// keys.sort_unstable();
    /// let total = keys.len();
    /// keys.dedup();
    /// assert_eq!(keys.len(), total);
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
    /// There is no `Direct` system target and there cannot be one: the executor has no
    /// operation of its own to perform before a call, which is exactly why the direct
    /// transport is the floor the other two are read against.
    ///
    /// ```
    /// use majordomus_cli::bench::{SystemTarget, Transport};
    /// assert_eq!(SystemTarget::McpPing.transport(), Transport::Mcp);
    /// assert_eq!(SystemTarget::HttpSwagger.transport(), Transport::Http);
    /// assert!(
    ///     !SystemTarget::ALL.iter().any(|t| t.transport() == Transport::Direct),
    ///     "the executor performs nothing before a call"
    /// );
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

    /// One line of what is measured, for the coverage report and the results table.
    ///
    /// It says what the sample contains, because two targets on the same route are not the
    /// same measurement: `GET /` with no `Accept` is the topology as JSON, and `GET /` with
    /// `Accept: text/html` is the home page rendered from it. A reader comparing the two
    /// numbers needs the description to know they are different work on one path.
    ///
    /// ```
    /// use majordomus_cli::bench::SystemTarget;
    /// assert!(SystemTarget::HttpIndex.description().contains("topology"));
    /// assert!(SystemTarget::HttpHome.description().contains("text/html"));
    /// assert!(SystemTarget::ALL.iter().all(|t| !t.description().is_empty()));
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
