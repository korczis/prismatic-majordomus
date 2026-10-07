//! The `mcp` module: the MCP projection of this registry, described as one typed value.
//!
//! MCP is a projection and owns nothing: every tool is a capability, every resource a
//! capability or an object of the layer. What a client can find out *about* the projection
//! was nevertheless scattered — the protocol versions in `initialize`, the tools one page
//! at a time in `tools/list`, the effect of each in an annotation, the client
//! configurations in `share/providers.yaml`, whether each one starts this repository's
//! server in a shell function. `mcp.projection` is the same facts as one answer, read by
//! the Cockpit's MCP page, the HTTP route, the tool and `majordomus mcp --inspect` alike,
//! so that none of them keeps an inventory of its own.
//!
//! Nothing here is a second list. The tools are the registry's, filtered by the exposure
//! they declare; the effect and the hints are the capability's classification
//! ([`ExecutionPolicy::hints`](crate::capability::ExecutionPolicy::hints)); the protocol
//! facts are the constants the server answers `initialize` with; the clients are the
//! providers the distribution declares.
//!
//! ```
//! use majordomus_cli::capability::builtin::mcp;
//! use majordomus_cli::capability::model::CapabilityKind;
//!
//! let m = mcp::module();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "mcp.projection");
//! // describing the projection reads the registry and changes nothing
//! assert_eq!(c.kind, CapabilityKind::Query);
//! assert_eq!(c.exposure.http.as_ref().unwrap().path, "/api/v1/mcp");
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    Effect, Exposure, Hints, McpExposure, McpResource, Provenance, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::{CachePolicy, CapabilityRegistry};
use crate::{capability, module};

use super::get;

/// The URI under which the description is read as a resource.
pub const MCP_URI: &str = "majordomus://mcp";

/// The launcher a client configuration names: the one command that finds or builds the
/// executable and speaks MCP. Relative to a repository that carries it, and the name of the
/// installed command where a repository does not.
pub const LAUNCHER: &str = "majordomus-mcp";

/// The input of `mcp.projection`: which tools to list. Everything else in the answer is
/// about the projection as a whole and is always given.
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpProjectionInput;
/// use majordomus_cli::capability::Effect;
/// let all: McpProjectionInput = serde_json::from_str("{}").unwrap();
/// assert!(all.effect.is_none());
/// let writers: McpProjectionInput =
///     serde_json::from_str(r#"{"effect":"repository_mutation"}"#).unwrap();
/// assert_eq!(writers.effect, Some(Effect::RepositoryMutation));
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpProjectionInput {
    /// List only the tools with this effect: `read`, `process_state` or
    /// `repository_mutation`. Absent lists every tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
}

impl BenchmarkCases for McpProjectionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("every-tool", McpProjectionInput { effect: None }),
            NamedCase::new(
                "writers",
                McpProjectionInput {
                    effect: Some(Effect::RepositoryMutation),
                },
            ),
        ]
    }
}

/// Who answers `initialize`, as it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpServerIdentity {
    /// The server name.
    pub name: String,
    /// The name a person reads.
    pub title: String,
    /// The version: this executable's, never one of the protocol adapter's own.
    pub version: String,
}

/// One way a client reaches the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpTransportEntry {
    /// `stdio` or `http`.
    pub id: String,
    /// How it is reached: the command a client starts, or the path on the shared server.
    pub reached_by: String,
    /// Sessions attached to this process over it right now.
    pub attached: usize,
}

/// What the server serves of the protocol, and what it does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpServing {
    /// The methods a request may name. Anything else is answered `-32601`.
    pub methods: Vec<String>,
    /// Tools: every capability that declares one.
    pub tools: bool,
    /// Resources: every capability that declares one, and every object of the layer.
    pub resources: bool,
    /// Prompts. Not served: the repository's prompt assets are resources, because a prompt
    /// served unrendered would read as finished.
    pub prompts: bool,
    /// Subscriptions and list-change notifications. Not served: this server sends nothing
    /// unasked.
    pub notifications: bool,
}

/// One tool, as the registry projects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpToolEntry {
    /// The name a client calls.
    pub name: String,
    /// The capability it projects; `capabilities.describe` answers its schemas.
    pub capability: String,
    /// The module that composes the capability.
    pub module: String,
    /// The title.
    pub title: String,
    /// What a call changes.
    pub effect: Effect,
    /// What a caller may assume before calling; the tool's annotations are this value.
    pub hints: Hints,
}

/// How many tools have one effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpEffectCount {
    /// The effect.
    pub effect: Effect,
    /// How many tools have it.
    pub tools: usize,
}

/// The resources a client can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpResourceSummary {
    /// Resources a builtin capability answers.
    pub builtin: usize,
    /// Objects of this repository's layer, each one more resource.
    pub objects: usize,
    /// The URI shape of an object's resource.
    pub template: String,
}

/// Where a client's project-scoped configuration stands in this repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpClientStanding {
    /// The file exists and names the launcher: this client starts this repository's server.
    Wired,
    /// The file exists and does not name the launcher: the client reads it and starts
    /// something else, or nothing.
    Foreign,
    /// The file does not exist: this client starts no server here until it is written.
    Absent,
}

/// One client the distribution declares a project-scoped configuration for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpClient {
    /// The provider id.
    pub id: String,
    /// The name a person knows it by.
    pub title: String,
    /// The file it reads, repository-relative.
    pub config: String,
    /// Where that file stands here.
    pub standing: McpClientStanding,
}

/// A client is the provider it belongs to, and is ordered by that id: the order every other
/// listing of providers reads in, whatever order the distribution declared them in.
impl crate::order::Ordered for McpClient {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id)
    }
}

/// Something about the projection a person should act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpFinding {
    /// The stable code.
    pub code: String,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub remedy: String,
}

/// The MCP projection of this registry in this repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpProjection {
    /// Who answers `initialize`.
    pub server: McpServerIdentity,
    /// The protocol versions accepted, newest first.
    pub protocol_versions: Vec<String>,
    /// How a client reaches the server.
    pub transports: Vec<McpTransportEntry>,
    /// What of the protocol is served.
    pub serving: McpServing,
    /// How many tools the registry projects, whatever the filter asked for.
    pub tool_count: usize,
    /// The tools asked for, in registry order.
    pub tools: Vec<McpToolEntry>,
    /// The tools counted by effect, every effect present.
    pub effects: Vec<McpEffectCount>,
    /// The tools that write the repository, by name. The `initialize` instructions name the
    /// same ones.
    pub writers: Vec<String>,
    /// The resources.
    pub resources: McpResourceSummary,
    /// Whether this repository carries the launcher at `bin/majordomus-mcp`. Where it does
    /// not, the installed command of the same name is what a configuration names.
    pub launcher_in_repository: bool,
    /// The clients the distribution declares a configuration for, and where each stands.
    pub clients: Vec<McpClient>,
    /// What a person should act on. Empty is nothing to do.
    pub findings: Vec<McpFinding>,
}

/// Every tool the registry projects, in registry order.
///
/// ```
/// use majordomus_cli::capability::builtin::{self, mcp};
/// use majordomus_cli::capability::{CapabilityRegistry, Effect};
/// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
/// let tools = mcp::tools(&registry);
/// let transition = tools.iter().find(|t| t.name == "majordomus_plan_transition").unwrap();
/// assert_eq!(transition.effect, Effect::RepositoryMutation);
/// assert!(transition.hints.destructive && !transition.hints.read_only);
/// ```
pub fn tools(registry: &CapabilityRegistry) -> Vec<McpToolEntry> {
    registry
        .iter()
        .filter_map(|c| {
            let name = c.exposure.mcp.as_ref()?.tool.clone()?;
            Some(McpToolEntry {
                name,
                capability: c.id.to_string(),
                module: c.module.to_string(),
                title: c.title.clone(),
                effect: c.execution.effect,
                hints: c.execution.hints(),
            })
        })
        .collect()
}

/// Where a client configuration stands: read from the file, never from a list of who is
/// expected to be wired.
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::{standing, McpClientStanding};
/// let dir = std::env::temp_dir().join(format!("mj-mcp-standing-{}", std::process::id()));
/// std::fs::create_dir_all(&dir).unwrap();
/// assert_eq!(standing(&dir, ".mcp.json"), McpClientStanding::Absent);
/// std::fs::write(dir.join(".mcp.json"), r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
/// assert_eq!(standing(&dir, ".mcp.json"), McpClientStanding::Foreign);
/// std::fs::write(dir.join(".mcp.json"), r#"{"mcpServers":{"majordomus":{"command":"bin/majordomus-mcp"}}}"#).unwrap();
/// assert_eq!(standing(&dir, ".mcp.json"), McpClientStanding::Wired);
/// std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn standing(root: &Path, config: &str) -> McpClientStanding {
    match std::fs::read_to_string(root.join(config)) {
        Err(_) => McpClientStanding::Absent,
        Ok(text) if text.contains(LAUNCHER) => McpClientStanding::Wired,
        Ok(_) => McpClientStanding::Foreign,
    }
}

fn projection(ctx: &Context, input: McpProjectionInput) -> Result<McpProjection, CapabilityError> {
    use crate::mcp::protocol;
    use crate::peers::Transport;

    let all = tools(&ctx.registry);
    let mut effects = Vec::new();
    for effect in [
        Effect::Read,
        Effect::ProcessState,
        Effect::RepositoryMutation,
    ] {
        let n = all.iter().filter(|t| t.effect == effect).count();
        if n > 0 {
            effects.push(McpEffectCount { effect, tools: n });
        }
    }
    let writers = all
        .iter()
        .filter(|t| t.effect == Effect::RepositoryMutation)
        .map(|t| t.name.clone())
        .collect();

    let peers = ctx.peers.list();
    let attached = |t: Transport| {
        peers
            .iter()
            .filter(|p| p.attached && p.transport == t)
            .count()
    };

    let root = Path::new(&ctx.index.repository.root);
    let launcher_in_repository = root.join("bin").join(LAUNCHER).is_file();
    let mut clients: Vec<McpClient> = ctx
        .index
        .providers
        .providers
        .iter()
        .filter_map(|d| {
            let config = d.client_config.clone()?;
            Some(McpClient {
                id: d.id.clone(),
                title: d.title.clone(),
                standing: standing(root, &config),
                config,
            })
        })
        .collect();
    crate::order::canonical(&mut clients);

    let mut findings = Vec::new();
    for c in &clients {
        if c.standing == McpClientStanding::Foreign {
            findings.push(McpFinding {
                code: "mcp_client_config_foreign".into(),
                message: format!(
                    "{} exists and does not name {LAUNCHER}, so {} does not start this repository's server from it",
                    c.config, c.title
                ),
                remedy: format!(
                    "add a `majordomus` server to {} whose command is {}",
                    c.config,
                    if launcher_in_repository {
                        "bin/majordomus-mcp"
                    } else {
                        LAUNCHER
                    }
                ),
            });
        }
    }
    if !clients.is_empty() && clients.iter().all(|c| c.standing == McpClientStanding::Absent) {
        findings.push(McpFinding {
            code: "mcp_no_client_configured".into(),
            message: "no declared client has a configuration in this repository, so none of them starts its server".into(),
            remedy: format!(
                "write one of {} naming the command {LAUNCHER}; docs/MCP.md shows each form",
                clients
                    .iter()
                    .map(|c| c.config.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    let builtin = ctx
        .registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .filter(|c| c.exposure.mcp.as_ref().is_some_and(|m| m.resource.is_some()))
        .count();

    Ok(McpProjection {
        server: McpServerIdentity {
            name: protocol::SERVER_NAME.into(),
            title: protocol::SERVER_TITLE.into(),
            version: crate::VERSION.into(),
        },
        protocol_versions: protocol::PROTOCOL_VERSIONS
            .iter()
            .map(|v| v.to_string())
            .collect(),
        transports: vec![
            McpTransportEntry {
                id: "stdio".into(),
                reached_by: "majordomus mcp".into(),
                attached: attached(Transport::Stdio),
            },
            McpTransportEntry {
                id: "http".into(),
                reached_by: format!("POST {} on the shared server", crate::http::mcp::PATH),
                attached: attached(Transport::Http),
            },
        ],
        serving: McpServing {
            methods: protocol::METHODS.iter().map(|m| m.to_string()).collect(),
            tools: true,
            resources: true,
            prompts: false,
            notifications: false,
        },
        tool_count: all.len(),
        tools: match input.effect {
            Some(effect) => all.iter().filter(|t| t.effect == effect).cloned().collect(),
            None => all.clone(),
        },
        effects,
        writers,
        resources: McpResourceSummary {
            builtin,
            objects: ctx.index.objects.len(),
            template: crate::site::MCP_RESOURCE_TEMPLATE.into(),
        },
        launcher_in_repository,
        clients,
        findings,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "mcp",
        title: "MCP projection",
        description: "The MCP projection of this registry, described as one typed value: who answers `initialize` and with which protocol versions, the transports and how many sessions each holds, what of the protocol is served and what is not, every tool with the effect and the hints its annotations are derived from, the tools that write the repository, the resources, and where each declared client's configuration stands in this repository. Nothing in it is a list of its own: the tools are the registry's, the effect is the capability's classification, the clients are the distribution's providers.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "mcp.projection",
                title: "The MCP projection of this registry",
                description: "Who answers `initialize` and with which protocol versions; the transports and the sessions attached over each; the methods served, and that prompts and notifications are not; every tool (or only those with one effect) with its capability, effect and the hints its annotations are derived from; the tools that write the repository; the resources; and whether each client the distribution declares has a configuration here that starts this repository's server. Reads the registry and one file per declared client; writes nothing.",
                input: McpProjectionInput,
                output: McpProjection,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_mcp".into()),
                        resource: Some(McpResource { uri: MCP_URI.into(), name: "mcp".into() }),
                    }),
                    http: get("/api/v1/mcp"),
                    cli: None,
                },
                tags: ["mcp", "introspection", "projections"],
                // the sessions attached are read from the board at the moment of asking
                cache: CachePolicy::Disabled,
                handler: projection,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin;

    fn registry() -> CapabilityRegistry {
        CapabilityRegistry::builder()
            .with_builtin(builtin::all())
            .build()
            .unwrap()
    }

    /// The listing is the registry's and nothing else's: a capability with a tool exposure
    /// is in it, one without is not, and the count is the registry's own count.
    #[test]
    fn the_tools_are_exactly_the_capabilities_that_declare_one() {
        let r = registry();
        let listed = tools(&r);
        let declared: Vec<String> = r
            .iter()
            .filter_map(|c| c.exposure.mcp.as_ref()?.tool.clone())
            .collect();
        assert_eq!(
            listed.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
            declared
        );
        assert_eq!(listed.len(), r.summary().mcp_tools);
        for t in &listed {
            let c = r.by_mcp_tool(&t.name).unwrap();
            assert_eq!(c.id.as_str(), t.capability);
            assert_eq!(c.execution.effect, t.effect);
            assert_eq!(c.execution.hints(), t.hints);
        }
    }

    /// The description describes itself: it is a tool of the projection it reports, and a
    /// read.
    #[test]
    fn the_description_is_one_of_the_tools_it_lists_and_is_a_read() {
        let listed = tools(&registry());
        let me = listed.iter().find(|t| t.name == "majordomus_mcp").unwrap();
        assert_eq!(me.capability, "mcp.projection");
        assert_eq!(me.effect, Effect::Read);
        assert!(me.hints.read_only);
    }
}
