//! The `connect` module: how a client attaches to this repository's one shared MCP server.
//!
//! Every other surface answers questions *through* the server; this one is the question a
//! person has before there is a session at all — what do I put in front of my client so
//! that it reaches this repository. The answer was folklore: three files at the root that
//! a reader had to find and copy, and, for a client whose configuration lives inside the
//! application (the ChatGPT app's Settings → MCP servers), nothing at all.
//!
//! Nothing here is a table of vendors. The set of clients is the providers the
//! distribution ships an adapter for (`share/providers.yaml`, ADR 0024); what each one
//! reads is that declaration's `client_config` — and the file's own bytes, read from this
//! checkout, are the snippet, so a configuration that changes at the root changes this
//! answer with it. A provider that keeps its configuration inside the application declares
//! `client_setup` instead, and the tokens in it are filled with this checkout's values:
//! the launcher's absolute path, the running server's endpoint, the repository root.
//!
//! The server half is read, never taken: the lease is looked at and probed, exactly as
//! `lease::serving` does for a command that only wants to ask. Asking how to connect must
//! not make the asking process the server.
//!
//! ```
//! use majordomus_cli::capability::builtin::connect;
//!
//! // the module's one composition: a capability with the projections every surface reads
//! let module = connect::module();
//! assert_eq!(module.id.as_str(), "connect");
//! assert_eq!(module.capabilities.len(), 1);
//!
//! // and what it answers with is the declaration filled in for a checkout, never a
//! // vendor's procedure copied into a surface
//! let setup = connect::fill(
//!     "add {name} as {command}",
//!     connect::SERVER_NAME,
//!     "/r/bin/majordomus-mcp",
//!     Some("http://127.0.0.1:8741/mcp"),
//!     "/r",
//! );
//! assert_eq!(setup, "add majordomus as /r/bin/majordomus-mcp");
//! ```

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::get;

/// The URI under which `connect.list` is read as an MCP resource.
pub const CONNECT_URI: &str = "majordomus://connect";

/// The name every client configuration gives the server, so that one repository is one
/// server under one name on every board.
pub const SERVER_NAME: &str = "majordomus";

/// The launcher a client configuration names, repository-relative.
pub const LAUNCHER: &str = "bin/majordomus-mcp";

// ---------------------------------------------------------------- output types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
/// Where a client keeps the configuration that names this server.
///
/// ```
/// use majordomus_cli::capability::builtin::ConfiguredIn;
/// // a file in this tree is shared by everyone who clones it; the application's own
/// // settings are added once per machine, and the wire word says which
/// assert_eq!(
///     serde_json::to_value(ConfiguredIn::Repository).unwrap(),
///     serde_json::json!("repository")
/// );
/// assert_eq!(ConfiguredIn::Application.as_str(), "application");
/// ```
pub enum ConfiguredIn {
    /// In this tree, at a path the vendor decided: the file is committed and shared.
    Repository,
    /// Inside the application: added once per machine, through its own settings.
    Application,
    /// The provider declares neither a file nor a procedure: it attaches through the agent
    /// it runs, or it is a name for a convention rather than a client.
    Nothing,
}

impl ConfiguredIn {
    /// The word this is reported under.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::ConfiguredIn;
    /// assert_eq!(ConfiguredIn::Application.as_str(), "application");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ConfiguredIn::Repository => "repository",
            ConfiguredIn::Application => "application",
            ConfiguredIn::Nothing => "nothing",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The shared server as a client would reach it, right now.
///
/// ```
/// use majordomus_cli::capability::builtin::ServerView;
/// // nothing is running: the endpoint is absent and the reason is what a person needs
/// let v: ServerView = serde_json::from_value(serde_json::json!({
///     "running": false,
///     "reason": "no server holds the lease",
///     "lease": ".ai/local/state/mcp/server.json",
/// })).unwrap();
/// assert!(!v.running && v.mcp_url.is_none());
/// assert_eq!(v.reason.as_deref(), Some("no server holds the lease"));
/// ```
pub struct ServerView {
    /// A server holds the lease and answered a probe for this root.
    pub running: bool,
    /// Its base URL, when one is running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Its Streamable HTTP endpoint, when one is running: the URL a client that speaks the
    /// transport itself is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_url: Option<String>,
    /// Why there is no endpoint, when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The lease file, repository-relative: what was read to answer this.
    pub lease: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One client, and what it takes for it to reach this repository.
///
/// ```
/// use majordomus_cli::capability::builtin::{ClientConnection, ConfiguredIn};
/// // a client that reads a file the checkout does not carry: named, and not pretended
/// let c: ClientConnection = serde_json::from_value(serde_json::json!({
///     "id": "claude-code",
///     "title": "Claude Code",
///     "configured_in": "repository",
///     "config_path": ".mcp.json",
///     "configured": false,
/// })).unwrap();
/// assert!(matches!(c.configured_in, ConfiguredIn::Repository));
/// assert_eq!(c.config_path.as_deref(), Some(".mcp.json"));
/// assert!(!c.configured && c.setup.is_none());
/// ```
pub struct ClientConnection {
    /// The provider id: the template's file stem, the key everywhere else.
    pub id: String,
    /// The name a person knows it by.
    pub title: String,
    /// Where this client keeps the configuration.
    pub configured_in: ConfiguredIn,
    /// The file it reads, repository-relative, when it reads one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// That file is present in this checkout.
    pub configured: bool,
    /// The configuration as this checkout holds it, for a client that reads a file; the
    /// vendor's procedure with this checkout's values filled in, for one that does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// Why there is nothing to show, when there is nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The stdio transport, as a client configuration names it.
///
/// ```
/// use majordomus_cli::capability::builtin::StdioView;
/// // both forms of the one launcher: the file at the root resolves the relative path,
/// // an application's settings have nothing to resolve it against
/// let v: StdioView = serde_json::from_value(serde_json::json!({
///     "command": "/r/bin/majordomus-mcp",
///     "relative": "bin/majordomus-mcp",
///     "present": true,
///     "cwd": "/r",
/// })).unwrap();
/// assert!(v.command.ends_with(&v.relative));
/// ```
pub struct StdioView {
    /// The launcher, absolute: what a client whose configuration lives outside the tree
    /// must be given, because it has no repository-relative path to resolve against.
    pub command: String,
    /// The same, repository-relative: what a file at the root names.
    pub relative: String,
    /// It is present and executable in this checkout.
    pub present: bool,
    /// The directory the client starts it in.
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// How every client reaches this repository, and what each one needs.
///
/// ```
/// use majordomus_cli::capability::builtin::ConnectReport;
/// let r: ConnectReport = serde_json::from_value(serde_json::json!({
///     "repository": "/r",
///     "name": "majordomus",
///     "stdio": { "command": "/r/bin/majordomus-mcp", "relative": "bin/majordomus-mcp", "present": true, "cwd": "/r" },
///     "server": { "running": false, "lease": ".ai/local/state/mcp/server.json" },
///     "clients": [],
/// })).unwrap();
/// // one server per repository, under one name on every client's board
/// assert_eq!(r.name, "majordomus");
/// assert!(r.clients.is_empty());
/// ```
pub struct ConnectReport {
    /// The repository root, absolute.
    pub repository: String,
    /// The name every configuration gives the server.
    pub name: String,
    /// The stdio transport: one launcher, shared by every client that spawns a process.
    pub stdio: StdioView,
    /// The Streamable HTTP transport: the server that is running, if one is.
    pub server: ServerView,
    /// One entry per client, sorted by id.
    pub clients: Vec<ClientConnection>,
}

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which client to answer for: one provider id, or every client the distribution ships an
/// adapter for. The id is the provider's, so what a person types here is the same word the
/// providers table, the product model and the site use for that client.
///
/// ```
/// use majordomus_cli::capability::builtin::ConnectInput;
/// let one: ConnectInput = serde_json::from_value(serde_json::json!({ "client": "chatgpt" })).unwrap();
/// assert_eq!(one.client.as_deref(), Some("chatgpt"));
/// // nothing named means every client
/// let all: ConnectInput = serde_json::from_value(serde_json::json!({})).unwrap();
/// assert!(all.client.is_none());
/// // and a field nobody declared is refused rather than ignored
/// assert!(serde_json::from_value::<ConnectInput>(serde_json::json!({ "clinet": "chatgpt" })).is_err());
/// ```
pub struct ConnectInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One provider id (`chatgpt`, `claude-code`, `codex`, ...). Absent means every client.
    pub client: Option<String>,
}

impl BenchmarkCases for ConnectInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("every-client", ConnectInput::default()),
            NamedCase::new(
                "one-client",
                ConnectInput {
                    client: Some("chatgpt".into()),
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- the answer

/// The running server's base URL, or why there is none.
///
/// Read-only in the strict sense: the lease file is read and the URL in it is probed, and
/// nothing is created, taken or waited for. A process asking how to connect must not
/// become the server by asking.
fn serving(root: &Path, lease: &Path) -> (Option<String>, String) {
    let Ok(text) = std::fs::read_to_string(lease) else {
        return (
            None,
            "no server holds the lease for this repository; start one with `majordomus serve`, or let the first client start it".into(),
        );
    };
    let url = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|d| {
            d.get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    match url {
        None => (
            None,
            "the lease names no URL yet: its owner is still binding, or it was left behind by a process that died".into(),
        ),
        Some(url) if crate::lease::probe(&url, root) => (Some(url), String::new()),
        Some(url) => (
            None,
            format!("the lease names {url}, and nothing answers there for this root: the next client to start takes it over"),
        ),
    }
}

/// Fill the tokens of a declared setup with this checkout's values.
///
/// ```
/// use majordomus_cli::capability::builtin::fill;
/// let out = fill("run {command} in {repository}", "majordomus", "/r/bin/x", None, "/r");
/// assert_eq!(out, "run /r/bin/x in /r");
/// // an endpoint that is not there says so where the URL would have been
/// assert!(fill("{url}", "majordomus", "/r/bin/x", None, "/r").contains("no server"));
/// ```
pub fn fill(
    template: &str,
    name: &str,
    command: &str,
    mcp_url: Option<&str>,
    repository: &str,
) -> String {
    let url = mcp_url.map(str::to_string).unwrap_or_else(|| {
        "(no server is running: start one with `majordomus serve` and read the URL it logs)"
            .to_string()
    });
    template
        .replace("{name}", name)
        .replace("{command}", command)
        .replace("{url}", &url)
        .replace("{repository}", repository)
}

fn connect_list(ctx: &Context, input: ConnectInput) -> Result<ConnectReport, CapabilityError> {
    let index = ctx.index.as_ref();
    let root = PathBuf::from(&index.repository.root);
    let local = index
        .repository
        .sections
        .get("local")
        .cloned()
        .unwrap_or_else(|| ".ai/local".to_string());
    let lease_rel = format!("{local}/{}", crate::lease::LEASE_PATH);
    let (url, reason) = serving(&root, &root.join(&lease_rel));
    let mcp_url = url
        .as_deref()
        .map(|u| format!("{}{}", u.trim_end_matches('/'), crate::http::mcp::PATH));

    let command = root.join(LAUNCHER);
    let stdio = StdioView {
        command: command.display().to_string(),
        relative: LAUNCHER.to_string(),
        present: command.is_file(),
        cwd: index.repository.root.clone(),
    };

    let wanted = input.client.as_deref();
    if let Some(id) = wanted {
        if !index.providers.providers.iter().any(|p| p.id == id) {
            return Err(CapabilityError::NotFound(format!(
                "no client '{id}'; the clients are the providers this distribution ships an adapter for: {}",
                index
                    .providers
                    .providers
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }

    let mut clients = Vec::new();
    for provider in &index.providers.providers {
        if wanted.is_some_and(|id| id != provider.id) {
            continue;
        }
        let (configured_in, config_path, configured, setup, note) = match (
            provider.client_config.as_deref(),
            provider.client_setup.as_deref(),
        ) {
            (Some(path), _) => {
                let file = root.join(path);
                let content = std::fs::read_to_string(&file).ok();
                (
                    ConfiguredIn::Repository,
                    Some(path.to_string()),
                    content.is_some(),
                    content,
                    (!file.is_file()).then(|| {
                        format!("{path} is not in this checkout: the client reads it, and nothing here writes it")
                    }),
                )
            }
            (None, Some(procedure)) => (
                ConfiguredIn::Application,
                None,
                false,
                Some(fill(
                    procedure,
                    SERVER_NAME,
                    &stdio.command,
                    mcp_url.as_deref(),
                    &index.repository.root,
                )),
                None,
            ),
            (None, None) => (
                ConfiguredIn::Nothing,
                None,
                false,
                None,
                Some(format!(
                    "{} declares no client configuration and no setup: it reaches this server through the client it runs, or it names a convention rather than a client",
                    provider.id
                )),
            ),
        };
        clients.push(ClientConnection {
            id: provider.id.clone(),
            title: provider.title.clone(),
            configured_in,
            config_path,
            configured,
            setup,
            note,
        });
    }

    Ok(ConnectReport {
        repository: index.repository.root.clone(),
        name: SERVER_NAME.to_string(),
        stdio,
        server: ServerView {
            running: url.is_some(),
            mcp_url,
            url,
            reason: (!reason.is_empty()).then_some(reason),
            lease: lease_rel,
        },
        clients,
    })
}

// ---------------------------------------------------------------- the module

/// The `connect` module: attaching a client to the shared server.
///
/// ```
/// use majordomus_cli::capability::builtin::connect::module;
/// let m = module();
/// let c = &m.capabilities[0].capability;
/// assert_eq!(c.id.to_string(), "connect.list");
/// // the same answer through every projection: a tool, a resource, a route and a command
/// assert!(c.exposure.mcp.is_some() && c.exposure.http.is_some() && c.exposure.cli.is_some());
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "connect",
        title: "Attaching a client",
        description: "How each client reaches this repository's one shared MCP server: the launcher every configuration names, the Streamable HTTP endpoint of the server that is running, the configuration this checkout holds for a client that reads a file, and the vendor's own procedure — with this checkout's values filled in — for one that keeps it inside the application.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "connect.list",
                title: "How a client attaches to this repository",
                description: "One entry per client the distribution ships an adapter for, with where its configuration lives, whether this checkout carries it, and what to put in front of it. The clients are the providers; nothing here is a list of vendors.",
                input: ConnectInput,
                output: ConnectReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_connect".into()),
                        resource: Some(McpResource { uri: CONNECT_URI.into(), name: "connect".into() }),
                    }),
                    http: get("/api/v1/connect"),
                    cli: Some(crate::capability::CliExposure { path: vec!["connect".into()] }),
                },
                tags: ["connect", "mcp", "providers"],
                cache: CachePolicy::Disabled,
                handler: connect_list,
            },
        ],
    }
}
