//! The `server` module: the shared server of this checkout, and of every other checkout of
//! the same git repository — what each lease says, whether the server it names answers,
//! and whether what answers is the code on disk.
//!
//! The lease has always carried this — the pid, the address, when the server started, the
//! executable it runs — and nothing served it: a person read the file with `cat`, a shell
//! recipe parsed it with `sed`, and a client learned only what the election happened to
//! log. Two readers had grown their own ideas of "ready" on the way (the environment's
//! connection attempt, the health route's registry count), and neither asked the question
//! the lease can answer: is the process behind this address current? This is that answer,
//! on every surface the registry projects to, from the one typed reading of the file
//! ([`crate::lease::LeaseFile`]).
//!
//! # One repository, every server of it
//!
//! A server serves a checkout; a linked worktree is a checkout of its own with a lease of
//! its own, and until now nothing said that the two belong to one repository. The status
//! lists every checkout git registers for the repository,
//! reads the lease of each (through the worktree topology reader), and names the git repository they share
//! ([`crate::repository::git_identity`]), so that a session in one worktree can see the
//! servers — and through them the peers — of the others.
//!
//! Read on every call and never cached: the lease is written by other processes.
//!
//! ```
//! use majordomus_cli::capability::builtin::server::{module, standing_of, ServerStanding};
//! use majordomus_cli::lease::LeaseFile;
//! use std::time::Duration;
//!
//! // the module composes one capability, and its decision is a pure function
//! let m = module();
//! assert_eq!(m.id.as_str(), "server");
//! assert!(m.capabilities.iter().any(|e| e.capability.id.to_string() == "server.status"));
//! let (standing, reason) = standing_of(&LeaseFile::Absent, Duration::ZERO, |_| false, "1.0.0");
//! assert_eq!(standing, ServerStanding::Absent);
//! assert!(reason.is_none());
//! ```

use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::lease::{self, ExecutableIdentity, LeaseDocument, LeaseFile, BIND_GRACE};
use crate::repository::{self, GitIdentity, Repository};
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `server.status` is read as an MCP resource.
pub const SERVER_URI: &str = "majordomus://server";

/// Where a checkout's server stands, decided from its lease, whether the server the lease
/// names answers for that checkout, and whether what answers is this executable's code.
///
/// ```
/// use majordomus_cli::capability::builtin::server::ServerStanding;
/// let all = [ServerStanding::Absent, ServerStanding::Starting, ServerStanding::Ready,
///            ServerStanding::Outdated, ServerStanding::Stale];
/// let words: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
/// assert_eq!(words, ["absent", "starting", "ready", "outdated", "stale"]);
/// assert_eq!(serde_json::from_str::<ServerStanding>("\"stale\"").unwrap(), ServerStanding::Stale);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServerStanding {
    /// No lease: nothing serves this checkout.
    Absent,
    /// A lease without an address, young enough that its owner is still binding.
    Starting,
    /// The server the lease names answers for this checkout, from the code on disk, at
    /// this executable's version.
    Ready,
    /// The server answers, but from older code than this executable, or from a file that
    /// has been replaced since it started: everything it says is yesterday's.
    Outdated,
    /// The lease names a server that does not answer, or is not a lease at all.
    Stale,
}

impl ServerStanding {
    /// The word every surface prints: the same one the JSON carries, so that a shell
    /// comparing the two never has to know which case each side chose.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::server::ServerStanding;
    /// assert_eq!(ServerStanding::Outdated.as_str(), "outdated");
    /// assert_eq!(serde_json::to_value(ServerStanding::Outdated).unwrap(), "outdated");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ServerStanding::Absent => "absent",
            ServerStanding::Starting => "starting",
            ServerStanding::Ready => "ready",
            ServerStanding::Outdated => "outdated",
            ServerStanding::Stale => "stale",
        }
    }
}

/// What this executable would want a server of this checkout to be: the address it would
/// bind, the version and the executable it would serve from. What `standing` is measured
/// against.
///
/// ```
/// use majordomus_cli::capability::builtin::server::Desired;
/// let d = Desired { host: "127.0.0.1".into(), port: 8741, version: "1.0.0".into(), executable: None };
/// let v = serde_json::to_value(&d).unwrap();
/// assert_eq!(v["port"], 8741);
/// assert!(v.get("executable").is_none(), "an executable that cannot be located is not written as null");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Desired {
    /// The interface the shared server binds by default.
    pub host: String,
    /// The port it asks for first.
    pub port: u16,
    /// This executable's version.
    pub version: String,
    /// This executable, when it can be located.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<ExecutableIdentity>,
}

/// A lease as a reader sees it: what the server wrote about itself, without the token that
/// makes the file the server's own.
///
/// ```
/// use majordomus_cli::capability::builtin::server::LeaseView;
/// let view = LeaseView { pid: 7, url: Some("http://127.0.0.1:8741".into()),
///     started_at: "2026-09-10T00:00:00Z".into(), executable: None, version: Some("1.0.0".into()) };
/// let v = serde_json::to_value(&view).unwrap();
/// assert_eq!(v["pid"], 7);
/// assert!(v.get("token").is_none() && v.get("root").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaseView {
    /// The server's process id. Informational: nothing decides liveness from it.
    pub pid: u32,
    /// The address, once bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// When the server took the lease, RFC 3339.
    pub started_at: String,
    /// The executable the server was started from, when it could be located.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<ExecutableIdentity>,
    /// The executable's version; absent for a server too old to have written one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl LeaseView {
    fn of(d: &LeaseDocument) -> Self {
        LeaseView {
            pid: d.pid,
            url: d.url.clone(),
            started_at: d.started_at.clone(),
            executable: d.executable.clone(),
            version: d.version.clone(),
        }
    }
}

/// One checkout of the repository, and the server its lease names.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{ServerStanding, ServerView};
/// let view = ServerView { worktree: "/r".into(), branch: Some("master".into()), checkout_id: "c".into(),
///     primary: true, this_checkout: true, standing: ServerStanding::Absent, reason: None, lease: None, peers: None };
/// let v = serde_json::to_value(&view).unwrap();
/// assert_eq!(v["standing"], "absent");
/// assert!(v.get("lease").is_none() && v.get("peers").is_none(), "what there is not is not written");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServerView {
    /// The checkout, absolute and canonical.
    pub worktree: PathBuf,
    /// The branch checked out there, when git names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The checkout's identity: what its server answers as `repository_id`.
    pub checkout_id: String,
    /// Whether this is the primary checkout rather than a linked worktree.
    pub primary: bool,
    /// Whether this is the checkout the answering process serves.
    pub this_checkout: bool,
    /// Where its server stands.
    pub standing: ServerStanding,
    /// Why, when the standing is not `ready`: what the lease held, or what answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The lease, when the file holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease: Option<LeaseView>,
    /// How many peers the server reports attached, when it answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<usize>,
}

/// The shared server of this checkout and of every other checkout of the repository.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{Desired, ServerStanding, ServerStatus};
/// let status = ServerStatus { checkout_id: "c".into(), git: None,
///     desired: Desired { host: "127.0.0.1".into(), port: 8741, version: "1.0.0".into(), executable: None },
///     this_process: None, standing: ServerStanding::Absent, servers: Vec::new() };
/// let text = serde_json::to_string(&status).unwrap();
/// assert_eq!(serde_json::from_str::<ServerStatus>(&text).unwrap(), status);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServerStatus {
    /// This checkout's identity: what this process answers as `repository_id`.
    pub checkout_id: String,
    /// The git repository this checkout belongs to; absent where git cannot be asked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitIdentity>,
    /// What this executable would serve, and what `standing` is measured against.
    pub desired: Desired,
    /// The lease this process holds, when it is the server; absent when the question was
    /// answered by a process that serves nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub this_process: Option<LeaseView>,
    /// Where this checkout's server stands.
    pub standing: ServerStanding,
    /// Every checkout of the repository, the primary first, each with its server. One entry
    /// — this checkout — where git cannot be asked.
    pub servers: Vec<ServerView>,
}

/// Decide where a server stands from what its lease file holds, how old the file is,
/// whether the server it names answers for the checkout, and this executable's version.
///
/// Pure, so that every branch of the decision is a test rather than a deployment:
///
/// ```
/// use majordomus_cli::capability::builtin::server::{standing_of, ServerStanding};
/// use majordomus_cli::lease::{LeaseDocument, LeaseFile, SCHEMA};
/// use std::time::Duration;
///
/// let young = Duration::ZERO;
/// let never = |_: &str| false;
/// let always = |_: &str| true;
/// assert_eq!(standing_of(&LeaseFile::Absent, young, never, "1.0.0").0, ServerStanding::Absent);
/// assert_eq!(standing_of(&LeaseFile::Empty, young, never, "1.0.0").0, ServerStanding::Starting);
/// assert_eq!(standing_of(&LeaseFile::Corrupt("not JSON".into()), young, never, "1.0.0").0, ServerStanding::Stale);
///
/// let lease = |url: Option<&str>, version: Option<&str>| LeaseDocument {
///     schema: SCHEMA.into(), pid: 1, token: "t".into(), root: "/r".into(),
///     url: url.map(str::to_string), started_at: "2026-09-10T00:00:00Z".into(),
///     executable: None, version: version.map(str::to_string),
/// };
/// // still binding, then abandoned
/// assert_eq!(standing_of(&LeaseFile::Document(lease(None, None)), young, never, "1.0.0").0, ServerStanding::Starting);
/// assert_eq!(standing_of(&LeaseFile::Document(lease(None, None)), Duration::from_secs(3600), never, "1.0.0").0, ServerStanding::Stale);
/// // names a server that does not answer
/// assert_eq!(standing_of(&LeaseFile::Document(lease(Some("http://127.0.0.1:1"), Some("1.0.0"))), young, never, "1.0.0").0, ServerStanding::Stale);
/// // answers from this version
/// assert_eq!(standing_of(&LeaseFile::Document(lease(Some("http://127.0.0.1:1"), Some("1.0.0"))), young, always, "1.0.0").0, ServerStanding::Ready);
/// // answers from another version, or from before versions were written down
/// assert_eq!(standing_of(&LeaseFile::Document(lease(Some("http://127.0.0.1:1"), Some("0.9.0"))), young, always, "1.0.0").0, ServerStanding::Outdated);
/// assert_eq!(standing_of(&LeaseFile::Document(lease(Some("http://127.0.0.1:1"), None)), young, always, "1.0.0").0, ServerStanding::Outdated);
/// ```
pub fn standing_of(
    file: &LeaseFile,
    age: Duration,
    answers: impl Fn(&str) -> bool,
    version: &str,
) -> (ServerStanding, Option<String>) {
    let doc = match file {
        LeaseFile::Absent => return (ServerStanding::Absent, None),
        LeaseFile::Empty if age > BIND_GRACE => {
            return (
                ServerStanding::Stale,
                Some("empty lease: its owner never wrote it".into()),
            )
        }
        LeaseFile::Empty => return (ServerStanding::Starting, None),
        LeaseFile::Corrupt(reason) => {
            return (
                ServerStanding::Stale,
                Some(format!("corrupt lease: {reason}")),
            )
        }
        LeaseFile::Document(doc) => doc,
    };
    let Some(url) = doc.url.as_deref() else {
        return if age > BIND_GRACE {
            (
                ServerStanding::Stale,
                Some("abandoned lease: its owner never published a URL".into()),
            )
        } else {
            (ServerStanding::Starting, None)
        };
    };
    if !answers(url) {
        return (
            ServerStanding::Stale,
            Some(format!(
                "the server the lease names at {url} does not answer for this checkout"
            )),
        );
    }
    if let Some(reason) = doc
        .executable
        .as_ref()
        .and_then(ExecutableIdentity::replaced)
    {
        return (ServerStanding::Outdated, Some(reason));
    }
    match doc.version.as_deref() {
        None => (
            ServerStanding::Outdated,
            Some(format!(
                "the server published no version, so it is older than this executable ({version})"
            )),
        ),
        Some(theirs) if theirs != version => (
            ServerStanding::Outdated,
            Some(format!(
                "the server is serving version {theirs}; this executable is {version}"
            )),
        ),
        Some(_) => (ServerStanding::Ready, None),
    }
}

/// How many peers the server at `url` reports, through the route the registry declares
/// for `peers.list`. `None` when it does not answer or the route is not declared.
fn peers_of(ctx: &Context, url: &str) -> Option<usize> {
    let path = ctx
        .registry
        .get("peers.list")?
        .exposure
        .http
        .as_ref()?
        .path
        .clone();
    let reply =
        crate::mcp::bridge::request(url, "GET", &path, &[], None, lease::PROBE_TIMEOUT).ok()?;
    if reply.status != 200 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&reply.body).ok()?;
    value["count"].as_u64().map(|n| n as usize)
}

fn server_status(ctx: &Context, _: Empty) -> Result<ServerStatus, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let local_half = Repository::open(&root)
        .map(|r| r.local_path())
        .unwrap_or_else(|_| ".ai/local".to_string());
    let git = repository::git_identity(&root);
    // every checkout git registers for the repository, the primary first; this one alone
    // where git cannot be asked
    let checkouts: Vec<(PathBuf, Option<String>)> = match &git {
        Some(_) => match crate::worktree::topology::read(&root) {
            Ok(records) => records
                .into_iter()
                .filter(|r| !r.bare)
                .map(|r| (r.path, r.branch))
                .collect(),
            Err(_) => vec![(root.clone(), None)],
        },
        None => vec![(root.clone(), None)],
    };
    let mut servers = Vec::with_capacity(checkouts.len());
    for (i, (path, branch)) in checkouts.into_iter().enumerate() {
        let worktree = path.canonicalize().unwrap_or(path);
        let file = lease::lease_file(&worktree, &local_half);
        let read = LeaseFile::read(&file);
        let (standing, reason) = standing_of(
            &read,
            lease::file_age(&file),
            |url| lease::probe(url, &worktree),
            crate::VERSION,
        );
        let peers = match standing {
            ServerStanding::Ready | ServerStanding::Outdated => read
                .document()
                .and_then(|d| d.url.as_deref())
                .and_then(|url| peers_of(ctx, url)),
            _ => None,
        };
        servers.push(ServerView {
            checkout_id: repository::identity(&worktree),
            primary: i == 0,
            this_checkout: worktree == root,
            branch,
            standing,
            reason,
            lease: read.document().map(LeaseView::of),
            peers,
            worktree,
        });
    }
    let standing = servers
        .iter()
        .find(|s| s.this_checkout)
        .map(|s| s.standing)
        .unwrap_or(ServerStanding::Absent);
    Ok(ServerStatus {
        checkout_id: repository::identity(&root),
        git,
        desired: Desired {
            host: "127.0.0.1".into(),
            port: crate::cli::DEFAULT_PORT,
            version: crate::VERSION.into(),
            executable: lease::executable_identity(),
        },
        this_process: lease::held().as_ref().map(LeaseView::of),
        standing,
        servers,
    })
}

/// The module: one capability, `server.status`, composed into the application by
/// `builtin::modules`; adding a second capability to it touches this file alone.
///
/// ```
/// use majordomus_cli::capability::builtin::server::module;
/// assert_eq!(module().capabilities.len(), 1);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "server",
        title: "Server",
        description: "The shared server of this checkout and of every other checkout of the same git repository: what each lease says, whether the server it names answers, whether what answers is the code on disk at this executable's version, and how many peers each one holds.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "server.status",
                title: "The shared server, and every server of the repository",
                description: "Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; the lease this process holds when it is the server; and every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, and the peers its server reports. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes.",
                input: Empty,
                output: ServerStatus,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_server".into()),
                        resource: Some(McpResource { uri: SERVER_URI.into(), name: "server".into() }),
                    }),
                    http: get("/api/v1/server"),
                    cli: None,
                },
                tags: ["server", "lease", "coordination", "introspection"],
                cache: CachePolicy::Disabled,
                handler: server_status,
            },
        ]
    }
}
