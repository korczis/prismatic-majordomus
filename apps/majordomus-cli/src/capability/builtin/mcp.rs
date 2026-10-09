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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpServerIdentity;
/// let s: McpServerIdentity = serde_json::from_value(serde_json::json!({
///     "name": "majordomus", "title": "Majordomus", "version": "0.14.0",
/// })).unwrap();
/// // the version is the executable's: the protocol adapter has none of its own to drift
/// assert_eq!((s.name.as_str(), s.version.as_str()), ("majordomus", "0.14.0"));
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpTransportEntry;
/// let t: McpTransportEntry = serde_json::from_value(serde_json::json!({
///     "id": "stdio", "reached_by": "majordomus mcp", "attached": 1,
/// })).unwrap();
/// // a count of sessions at the moment of asking, not a claim that a server is running
/// assert_eq!((t.id.as_str(), t.attached), ("stdio", 1));
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpServing;
/// let s: McpServing = serde_json::from_value(serde_json::json!({
///     "methods": ["initialize", "tools/call"], "tools": true, "resources": true,
///     "prompts": false, "notifications": false,
/// })).unwrap();
/// // what is not served is stated, so a client does not learn it from a refusal
/// assert!(s.tools && !s.prompts && !s.notifications);
/// assert!(!s.methods.iter().any(|m| m.starts_with("prompts/")));
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpToolEntry;
/// use majordomus_cli::capability::Effect;
/// let t: McpToolEntry = serde_json::from_value(serde_json::json!({
///     "name": "majordomus_plan_transition", "capability": "plan.transition",
///     "module": "plan", "title": "Move an issue", "effect": "repository_mutation",
///     "hints": { "read_only": false, "destructive": true, "idempotent": false, "open_world": false },
/// })).unwrap();
/// assert_eq!(t.effect, Effect::RepositoryMutation);
/// // the hints are the effect's classification, carried beside it rather than restated
/// assert!(t.hints.destructive && !t.hints.read_only && !t.hints.idempotent);
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpEffectCount;
/// use majordomus_cli::capability::Effect;
/// let c: McpEffectCount =
///     serde_json::from_value(serde_json::json!({ "effect": "read", "tools": 149 })).unwrap();
/// assert_eq!((c.effect, c.tools), (Effect::Read, 149));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpEffectCount {
    /// The effect.
    pub effect: Effect,
    /// How many tools have it.
    pub tools: usize,
}

/// The resources a client can read.
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpResourceSummary;
/// let r: McpResourceSummary = serde_json::from_value(serde_json::json!({
///     "builtin": 54, "objects": 1742, "template": "majordomus://<kind>/<identity>",
/// })).unwrap();
/// // the two counts together are the length of `resources/list`
/// assert_eq!(r.builtin + r.objects, 1796);
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpClientStanding;
/// // the words a caller reads, as they are written on every surface
/// assert_eq!(serde_json::to_value(McpClientStanding::Wired).unwrap(), "wired");
/// assert_eq!(serde_json::to_value(McpClientStanding::Foreign).unwrap(), "foreign");
/// assert_eq!(serde_json::to_value(McpClientStanding::Absent).unwrap(), "absent");
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::{McpClient, McpClientStanding};
/// let mut clients: Vec<McpClient> = serde_json::from_value(serde_json::json!([
///     { "id": "gemini", "title": "Gemini CLI", "config": ".gemini/settings.json", "standing": "absent" },
///     { "id": "claude-code", "title": "Claude Code", "config": ".mcp.json", "standing": "wired" },
/// ])).unwrap();
/// // ordered by the provider's id, whatever order the distribution declared them in
/// majordomus_cli::order::canonical(&mut clients);
/// assert_eq!(clients[0].id, "claude-code");
/// assert_eq!(clients[0].standing, McpClientStanding::Wired);
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpFinding;
/// let f: McpFinding = serde_json::from_value(serde_json::json!({
///     "code": "mcp_client_config_foreign",
///     "message": ".mcp.json exists and does not name majordomus-mcp",
///     "remedy": "add a `majordomus` server to .mcp.json whose command is majordomus-mcp",
/// })).unwrap();
/// // a finding always says what to do, not only what is wrong
/// assert!(!f.code.is_empty() && !f.remedy.is_empty());
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::McpProjection;
/// let p: McpProjection = serde_json::from_value(serde_json::json!({
///     "server": { "name": "majordomus", "title": "Majordomus", "version": "0.14.0" },
///     "protocol_versions": ["2025-06-18"],
///     "transports": [{ "id": "stdio", "reached_by": "majordomus mcp", "attached": 0 }],
///     "serving": { "methods": ["initialize"], "tools": true, "resources": true, "prompts": false, "notifications": false },
///     "tool_count": 2,
///     "tools": [],
///     "effects": [{ "effect": "read", "tools": 1 }, { "effect": "repository_mutation", "tools": 1 }],
///     "writers": ["majordomus_plan_transition"],
///     "resources": { "builtin": 1, "objects": 0, "template": "majordomus://<kind>/<identity>" },
///     "launcher_in_repository": false,
///     "clients": [],
///     "findings": [],
/// })).unwrap();
/// // the count is of every tool, whatever the `effect` filter narrowed `tools` to
/// assert_eq!(p.tool_count, p.effects.iter().map(|e| e.tools).sum::<usize>());
/// assert!(p.tools.is_empty() && p.writers.len() == 1);
/// ```
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

/// How many tools carry each effect, in the order the effects are declared; an effect no tool
/// has is left out rather than counted as zero.
fn effect_counts(all: &[McpToolEntry]) -> Vec<McpEffectCount> {
    [
        Effect::Read,
        Effect::ProcessState,
        Effect::RepositoryMutation,
    ]
    .into_iter()
    .map(|effect| McpEffectCount {
        effect,
        tools: all.iter().filter(|t| t.effect == effect).count(),
    })
    .filter(|count| count.tools > 0)
    .collect()
}

/// What the client configurations of this repository leave to be done: one finding for
/// each configuration that exists and does not start this repository's server, and one
/// when no declared client has a configuration at all.
fn client_findings(clients: &[McpClient], launcher_in_repository: bool) -> Vec<McpFinding> {
    let command = if launcher_in_repository {
        "bin/majordomus-mcp"
    } else {
        LAUNCHER
    };
    let mut findings: Vec<McpFinding> = clients
        .iter()
        .filter(|c| c.standing == McpClientStanding::Foreign)
        .map(|c| McpFinding {
            code: "mcp_client_config_foreign".into(),
            message: format!(
                "{} exists and does not name {LAUNCHER}, so {} does not start this repository's server from it",
                c.config, c.title
            ),
            remedy: format!(
                "add a `majordomus` server to {} whose command is {command}",
                c.config
            ),
        })
        .collect();
    if !clients.is_empty()
        && clients
            .iter()
            .all(|c| c.standing == McpClientStanding::Absent)
    {
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
    findings
}

fn projection(ctx: &Context, input: McpProjectionInput) -> Result<McpProjection, CapabilityError> {
    Ok(describe(ctx, input))
}

/// The projection as the capability answers it, for a caller inside this program that
/// already holds the context: `mcp --inspect` prints the same value the tool, the resource
/// and the route are asked for, and reading it cannot fail, because nothing in it is
/// fetched — the registry, the peer board and the layer's index are in the context, and a
/// client configuration that cannot be read is one that is absent.
///
/// ```
/// use majordomus_cli::capability::builtin::mcp::{describe, McpProjectionInput};
/// use majordomus_cli::capability::{builtin, CapabilityRegistry, Context, Effect};
/// use majordomus_cli::git::GitState;
/// use majordomus_cli::index::{Index, RepositoryInfo, State};
/// use std::sync::Arc;
/// // an index with no objects and no declared provider: the registry alone answers
/// let index = Index {
///     repository: RepositoryInfo {
///         root: "/tmp/doc".into(), layer_schema: "ai-repository/v1".into(),
///         sections: Default::default(), git: GitState::Unavailable { reason: "doc".into() },
///         discovery: "filesystem".into(), source_classes: vec![], kind_sources: vec![],
///         scope_origin: majordomus_cli::scope::Origin::Distribution, scope_path: String::new(),
///         observed: Default::default(),
///     },
///     objects: vec![], diagnostics: vec![], state: State::Ok, fingerprint: String::new(),
///     scoped: Default::default(), distribution: None, providers: Default::default(),
///     share: None,
/// };
/// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).with_index(&index).build().unwrap();
/// let ctx = Context::new(Arc::new(index), Arc::new(registry));
///
/// let all = describe(&ctx, McpProjectionInput::default());
/// assert_eq!(all.tools.len(), all.tool_count);
/// assert_eq!(all.effects.iter().map(|e| e.tools).sum::<usize>(), all.tool_count);
/// // no provider is declared, so there is no client and nothing to be done about one
/// assert!(all.clients.is_empty() && all.findings.is_empty());
///
/// // one effect narrows the tools listed and nothing else
/// let writers = describe(&ctx, McpProjectionInput { effect: Some(Effect::RepositoryMutation) });
/// assert_eq!(writers.tool_count, all.tool_count);
/// assert_eq!(writers.tools.len(), all.writers.len());
/// assert!(writers.tools.iter().all(|t| all.writers.contains(&t.name)));
/// ```
pub fn describe(ctx: &Context, input: McpProjectionInput) -> McpProjection {
    use crate::mcp::protocol;
    use crate::peers::Transport;

    let all = tools(&ctx.registry);
    let effects = effect_counts(&all);
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

    let findings = client_findings(&clients, launcher_in_repository);

    let builtin = ctx
        .registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .filter(|c| {
            c.exposure
                .mcp
                .as_ref()
                .is_some_and(|m| m.resource.is_some())
        })
        .count();

    McpProjection {
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
    }
}

/// The `mcp` module: the one capability that describes the MCP projection, composed into
/// the application like every other module, so that describing the projection is itself a
/// tool, a resource and a route of it.
///
/// ```
/// use majordomus_cli::capability::builtin::mcp;
/// let m = mcp::module();
/// assert_eq!(m.id.as_str(), "mcp");
/// let c = &m.capabilities[0].capability;
/// assert_eq!(c.exposure.mcp.as_ref().unwrap().tool.as_deref(), Some("majordomus_mcp"));
/// assert_eq!(c.exposure.mcp.as_ref().unwrap().resource.as_ref().unwrap().uri, mcp::MCP_URI);
/// ```
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

    /// An effect is counted when a tool has it and left out when none does: a row saying
    /// "0 tools" would be a row about nothing.
    #[test]
    fn an_effect_no_tool_has_is_not_counted() {
        let all = tools(&registry());
        let counted = effect_counts(&all);
        assert_eq!(
            counted.iter().map(|c| c.tools).sum::<usize>(),
            all.len(),
            "every tool has exactly one of the declared effects"
        );
        let reads: Vec<McpToolEntry> = all
            .iter()
            .filter(|t| t.effect == Effect::Read)
            .cloned()
            .collect();
        let only = effect_counts(&reads);
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].effect, Effect::Read);
        assert_eq!(only[0].tools, reads.len());
        assert!(effect_counts(&[]).is_empty());
    }

    /// The standing is what the file says: absent, there and naming something else, or
    /// naming the launcher.
    #[test]
    fn a_configuration_stands_where_its_file_says() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert_eq!(standing(dir.path(), ".mcp.json"), McpClientStanding::Absent);
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"other":{"command":"x"}}}"#,
        )
        .unwrap();
        assert_eq!(
            standing(dir.path(), ".mcp.json"),
            McpClientStanding::Foreign
        );
        std::fs::write(
            dir.path().join(".mcp.json"),
            format!(r#"{{"mcpServers":{{"majordomus":{{"command":"bin/{LAUNCHER}"}}}}}}"#),
        )
        .unwrap();
        assert_eq!(standing(dir.path(), ".mcp.json"), McpClientStanding::Wired);
    }

    fn client(config: &str, standing: McpClientStanding) -> McpClient {
        McpClient {
            id: config.trim_start_matches('.').into(),
            title: format!("the client of {config}"),
            config: config.into(),
            standing,
        }
    }

    /// A configuration that exists and starts something else is named, with the command
    /// that would start this repository's server: the checkout's own launcher when the
    /// repository carries one, the installed one otherwise.
    #[test]
    fn a_foreign_configuration_is_a_finding_that_names_the_command() {
        let clients = [
            client(".mcp.json", McpClientStanding::Foreign),
            client(".codex/config.toml", McpClientStanding::Wired),
        ];
        let here = client_findings(&clients, true);
        assert_eq!(here.len(), 1);
        assert_eq!(here[0].code, "mcp_client_config_foreign");
        assert!(here[0].message.contains(".mcp.json"));
        assert!(here[0].message.contains("the client of .mcp.json"));
        assert!(here[0]
            .remedy
            .ends_with("whose command is bin/majordomus-mcp"));

        let installed = client_findings(&clients, false);
        assert_eq!(installed.len(), 1);
        assert!(installed[0]
            .remedy
            .ends_with(&format!("whose command is {LAUNCHER}")));
    }

    /// No configuration anywhere is one finding that lists where one could be written; a
    /// wired client, or no declared client at all, is none.
    #[test]
    fn no_configuration_at_all_is_one_finding_and_a_wired_one_is_none() {
        let absent = [
            client(".mcp.json", McpClientStanding::Absent),
            client(".gemini/settings.json", McpClientStanding::Absent),
        ];
        let findings = client_findings(&absent, false);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "mcp_no_client_configured");
        assert!(findings[0]
            .remedy
            .contains(".mcp.json, .gemini/settings.json"));

        let wired = [
            client(".mcp.json", McpClientStanding::Wired),
            client(".gemini/settings.json", McpClientStanding::Absent),
        ];
        assert!(client_findings(&wired, true).is_empty());
        assert!(client_findings(&[], true).is_empty());
    }
}
