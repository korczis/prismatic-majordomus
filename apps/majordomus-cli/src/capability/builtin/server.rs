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
//! # Two questions, two costs
//!
//! Listing every checkout costs a lease read and a probe per checkout, and a machine with
//! forty worktrees registered pays forty of each. A caller that only wants to know about
//! the checkout it is in says so — `checkouts: this` — and that answer reads one lease and
//! probes one server; no other checkout is enumerated, read or probed. The default is
//! `repository`, the whole list, because that is what every caller of this capability got
//! before the field existed.
//!
//! The wide answer asks sixteen checkouts at a time (`ASKED_AT_ONCE`, `each_checkout`), so its
//! wait is a few probe timeouts however many checkouts there are, not one per checkout: on
//! a machine of sixty servers, a few of them wedged, one at a time took more than thirty
//! seconds (I2987).
//!
//! Read on every call and never cached: the lease is written by other processes.
//!
//! ```
//! use majordomus_cli::capability::builtin::server::{
//!     module, standing_of, Checkouts, ServerStanding, ServerStatusInput,
//! };
//! use majordomus_cli::lease::LeaseFile;
//! use std::time::Duration;
//!
//! // asking nothing asks the wide question, as it always did
//! assert_eq!(
//!     serde_json::from_str::<ServerStatusInput>("{}").unwrap().checkouts,
//!     Checkouts::Repository
//! );
//!
//! // the module composes one capability, and its decision is a pure function
//! let m = module();
//! assert_eq!(m.id.as_str(), "server");
//! assert!(m.capabilities.iter().any(|e| e.capability.id.to_string() == "server.status"));
//! let (standing, reason) = standing_of(&LeaseFile::Absent, Duration::ZERO, |_| false, "1.0.0");
//! assert_eq!(standing, ServerStanding::Absent);
//! assert!(reason.is_none());
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::lease::{self, ExecutableIdentity, LeaseDocument, LeaseFile, VersionVerdict};
use crate::repository::{self, GitIdentity, Repository};
use crate::{capability, module};

use super::get;

/// The URI under which `server.status` is read as an MCP resource.
pub const SERVER_URI: &str = "majordomus://server";

/// Which checkouts a reading of the server status covers.
///
/// The two answers cost differently and that is the whole reason the choice exists: the
/// repository's checkouts are read from git's registry of them and each one costs a lease
/// read and a probe, while this checkout alone costs one of each and enumerates nothing.
/// The word each variant serialises to is the word the query parameter, the tool input and
/// the command-line flag all take.
///
/// ```
/// use majordomus_cli::capability::builtin::server::Checkouts;
/// assert_eq!(serde_json::to_value(Checkouts::This).unwrap(), "this");
/// assert_eq!(serde_json::to_value(Checkouts::Repository).unwrap(), "repository");
/// assert_eq!(Checkouts::default(), Checkouts::Repository, "the wide answer is what a caller who says nothing has always got");
/// ```
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Checkouts {
    /// Every checkout git registers for the repository, the primary first — a lease read
    /// and a probe each. The default, because it is the answer this capability gave before
    /// there was anything to ask.
    #[default]
    Repository,
    /// This checkout alone. No other checkout is enumerated, no other lease is read and no
    /// other server is probed; `servers` holds the one entry, and it is this one.
    This,
}

/// What to ask of `server.status`: how wide the answer is.
///
/// One field, because the standing of a server is not something a caller filters — it is
/// something a caller wants for one checkout or for all of them, and the difference is
/// forty round trips on a machine with forty worktrees.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{Checkouts, ServerStatusInput};
/// // the field is optional on every surface, and its absence is the wide answer
/// let asked_nothing: ServerStatusInput = serde_json::from_str("{}").unwrap();
/// assert_eq!(asked_nothing.checkouts, Checkouts::Repository);
/// let narrow: ServerStatusInput = serde_json::from_str(r#"{"checkouts":"this"}"#).unwrap();
/// assert_eq!(narrow.checkouts, Checkouts::This);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerStatusInput {
    /// Which checkouts the answer covers. Absent means `repository`.
    #[serde(default)]
    pub checkouts: Checkouts,
}

impl BenchmarkCases for ServerStatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // both questions are timed, because the difference between them is the point of
        // the field; both appear as examples in the OpenAPI document
        vec![
            NamedCase::new(
                "repository",
                ServerStatusInput {
                    checkouts: Checkouts::Repository,
                },
            ),
            NamedCase::new(
                "this-checkout",
                ServerStatusInput {
                    checkouts: Checkouts::This,
                },
            ),
        ]
    }
}

/// Where a checkout's server stands, decided from its lease, whether the server the lease
/// names answers for that checkout, and whether what answers is this executable's code —
/// and, refined by the readiness report, the mesh runtime and the last recorded transition,
/// where its runtime stands (I3179).
///
/// The first five words are the lease's standing, decided by [`standing_of`], and they are
/// what `standing` carries on every surface: shells compare them, so they never change. The
/// other seven are the runtime's refinements, decided by [`runtime_of`] and carried by
/// `runtime.state` alone; [`ServerStanding::refines`] names the lease word each refines.
///
/// ```
/// use majordomus_cli::capability::builtin::server::ServerStanding;
/// let all = [ServerStanding::Absent, ServerStanding::Starting, ServerStanding::Ready,
///            ServerStanding::Outdated, ServerStanding::Stale];
/// let words: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
/// assert_eq!(words, ["absent", "starting", "ready", "outdated", "stale"]);
/// assert_eq!(serde_json::from_str::<ServerStanding>("\"stale\"").unwrap(), ServerStanding::Stale);
/// // the runtime's words, in the same snake case the JSON carries
/// assert_eq!(serde_json::to_value(ServerStanding::MeshAttaching).unwrap(), "mesh_attaching");
/// assert_eq!(ServerStanding::MeshAttaching.as_str(), "mesh_attaching");
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
    /// No lease yet, and the executable a server would run is being built: recorded by the
    /// builder (I3180), and believed only while the process that recorded it is alive.
    Building,
    /// The server is ready, the repository declares its mesh enabled, and the server has not
    /// yet decided whether the mesh runs.
    MeshAttaching,
    /// The server is ready, says so itself, and its mesh runs or is not declared.
    Operational,
    /// The server answers, current, but its own readiness report or its mesh says it is not
    /// whole: it lost the lease, serves replaced code, read a degraded layer, or its declared
    /// mesh is not running.
    Degraded,
    /// The process that holds the lease recorded that it is stopping, and it is still alive.
    Stopping,
    /// The server is gone after it recorded that it was stopping.
    Stopped,
    /// The server is gone without having recorded that it was stopping: its lease names a
    /// process that no longer exists.
    Failed,
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
            ServerStanding::Building => "building",
            ServerStanding::MeshAttaching => "mesh_attaching",
            ServerStanding::Operational => "operational",
            ServerStanding::Degraded => "degraded",
            ServerStanding::Stopping => "stopping",
            ServerStanding::Stopped => "stopped",
            ServerStanding::Failed => "failed",
        }
    }

    /// The lease's word this state refines: itself for the five the lease decides, and for
    /// a runtime state the word a reader holding only the lease would have said. A surface
    /// that knows only the five words judges a runtime state by this.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::server::ServerStanding as S;
    /// assert_eq!(S::Ready.refines(), S::Ready);
    /// assert_eq!(S::Operational.refines(), S::Ready);
    /// assert_eq!(S::Building.refines(), S::Starting);
    /// assert_eq!(S::Stopped.refines(), S::Absent);
    /// assert_eq!(S::Failed.refines(), S::Stale);
    /// ```
    pub fn refines(self) -> ServerStanding {
        match self {
            ServerStanding::Absent | ServerStanding::Stopped => ServerStanding::Absent,
            ServerStanding::Starting | ServerStanding::Building => ServerStanding::Starting,
            ServerStanding::Ready
            | ServerStanding::MeshAttaching
            | ServerStanding::Operational
            | ServerStanding::Degraded
            | ServerStanding::Stopping => ServerStanding::Ready,
            ServerStanding::Outdated => ServerStanding::Outdated,
            ServerStanding::Stale | ServerStanding::Failed => ServerStanding::Stale,
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
    /// The interface a server started here would bind: what `MAJORDOMUS_HTTP_HOST` names
    /// in this process's environment, and loopback when it names nothing.
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
    /// The server's process id. Not proof of a live server — only a probe that is answered
    /// is — but decisions are made from it: a live pid earns a server that does not answer
    /// the election's busy wait before its lease is taken over, and `serve ensure`'s patience
    /// before it starts another; `serve stop` sends its `SIGTERM` to this pid.
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
///     primary: true, this_checkout: true, standing: ServerStanding::Absent, reason: None, lease: None, peers: None,
///     runtime: None };
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
    /// Where its runtime stands, and the last transition recorded for it (I3179). Absent
    /// only from an answer that predates the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeState>,
}

/// The shared server of this checkout and of every other checkout of the repository.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{Desired, ServerStanding, ServerStatus};
/// let status = ServerStatus { checkout_id: "c".into(), git: None,
///     desired: Desired { host: "127.0.0.1".into(), port: 8741, version: "1.0.0".into(), executable: None },
///     this_process: None, standing: ServerStanding::Absent, servers: Vec::new(),
///     connections: None, trunk: None, runtime: None };
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
    /// What this process's HTTP server did to connections that held it without a request:
    /// the read deadline it applies, the connections that deadline closed, and the requests
    /// refused because every handler was busy. Absent when this process serves no HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connections: Option<crate::http::deadline::ConnectionCounts>,
    /// How far the tree this checkout serves is from the remote trunk, judged against the
    /// policy's `checkout.trunk_distance`: a server of a checkout 779 commits behind answers
    /// every session from a tree that no longer exists, and this is where that is said.
    /// Absent only from an answer that predates the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<crate::worktree::TrunkDistance>,
    /// Where this checkout's runtime stands — the lease's word refined by the server's
    /// readiness report and mesh verdict and by the last recorded transition — with that
    /// transition: when, why and who recorded it (I3179). `standing` keeps the lease's five
    /// words; this is where the other seven appear. Absent only from an answer that predates
    /// the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeState>,
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
        LeaseFile::Empty if age > lease::timings().bind_grace => {
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
        return if age > lease::timings().bind_grace {
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
    let theirs = doc.version.as_deref().unwrap_or_default();
    match lease::version_verdict(doc.version.as_deref(), version) {
        VersionVerdict::Same => (ServerStanding::Ready, None),
        VersionVerdict::Unpublished => (
            ServerStanding::Outdated,
            Some(format!(
                "the server published no version, so it is older than this executable ({version})"
            )),
        ),
        VersionVerdict::ServerOlder => (
            ServerStanding::Outdated,
            Some(format!(
                "the server is serving version {theirs}, older than this executable ({version}); the next client or `serve ensure` of this executable replaces it"
            )),
        ),
        VersionVerdict::ServerNewer => (
            ServerStanding::Outdated,
            Some(format!(
                "the server is serving version {theirs}, newer than this executable ({version}); clients of this executable attach to it and say they are older"
            )),
        ),
        VersionVerdict::Unordered => (
            ServerStanding::Outdated,
            Some(format!(
                "the server is serving version {theirs}; this executable is {version}"
            )),
        ),
    }
}

/// The repository-relative path of a checkout's local half — the directory its lease is
/// written into — or the layer's default when no manifest can be read at that checkout.
///
/// A directory that is not a repository of the layer still gets an answer, because "does a
/// server serve this checkout" is a question about a path and not about a manifest.
///
/// ```
/// use majordomus_cli::capability::builtin::server::local_half;
/// assert_eq!(local_half(std::path::Path::new("/nonexistent/checkout")), ".ai/local");
/// ```
pub fn local_half(root: &Path) -> String {
    Repository::open(root)
        .map(|r| r.local_path())
        .unwrap_or_else(|_| ".ai/local".to_string())
}

/// Where the server of one checkout stands, with the lease the verdict was read from.
///
/// [`standing_of`] is the decision; this is the one reading that feeds it — the lease file
/// of `worktree` under `local_half`, how old that file is, one probe of the address it
/// names, against this executable's version. Every surface that asks where a server stands
/// asks through here, so that `server.status` and the `server` check of `health.report`
/// cannot drift into two answers.
///
/// Costs one small file read, and one HTTP round trip bounded by the declared probe timeout
/// ([`crate::lease::timings`]) when the lease names an address. Nothing is cached: the
/// file is written by other processes.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{local_half, standing_at, ServerStanding};
/// let nowhere = std::path::Path::new("/nonexistent/checkout");
/// let (standing, reason, lease) = standing_at(nowhere, &local_half(nowhere));
/// assert_eq!(standing, ServerStanding::Absent);
/// assert!(reason.is_none(), "there is nothing to explain: no lease, no server");
/// assert!(lease.document().is_none());
/// ```
pub fn standing_at(
    worktree: &Path,
    local_half: &str,
) -> (ServerStanding, Option<String>, LeaseFile) {
    let file = lease::lease_file(worktree, local_half);
    let read = LeaseFile::read(&file);
    // this process's own lease is answered from memory: a probe would be a request to
    // itself, queued behind the very request asking (lease::is_own)
    let own = read.document().is_some_and(lease::is_own);
    let (standing, reason) = standing_of(
        &read,
        lease::file_age(&file),
        |url| own || lease::probe(url, worktree),
        crate::VERSION,
    );
    (standing, reason, read)
}

// ---------------------------------------------------------------- the runtime state (I3179)

/// Where a checkout's runtime record lives, under its local half: beside the lease's
/// directory and never in it, because `.envrc` watches the lease file and a record written
/// there would reload every shell of the checkout on every transition.
pub const RUNTIME_PATH: &str = "state/runtime/state.json";

/// The schema every runtime record carries.
pub const RUNTIME_SCHEMA: &str = "majordomus-runtime-state/v1";

/// The environment variable a caller of `server.status` sets to name itself as the reader
/// that recorded a transition: `reap_orphans` is the one word it takes (`scripts/reap-orphans`
/// asks `serve status` after a sweep); anything else, or nothing, is `server_status`.
pub const RUNTIME_WRITER_ENV: &str = "MAJORDOMUS_RUNTIME_WRITER";

/// Who recorded a transition. Each state has its one kind of writer: the serving process
/// records its own `starting`, `ready` and `stopping`, because only it knows them; a reader
/// that finds the server gone records `stopped` or `failed`, because a dead process records
/// nothing.
///
/// ```
/// use majordomus_cli::capability::builtin::server::RuntimeWriter;
/// assert_eq!(serde_json::to_value(RuntimeWriter::ServeEnsure).unwrap(), "serve_ensure");
/// assert_eq!(RuntimeWriter::reader_named(Some("reap_orphans")), RuntimeWriter::ReapOrphans);
/// assert_eq!(RuntimeWriter::reader_named(Some("server")), RuntimeWriter::ServerStatus);
/// assert_eq!(RuntimeWriter::reader_named(None), RuntimeWriter::ServerStatus);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWriter {
    /// The serving process, about itself.
    Server,
    /// `serve ensure`, which found the lease stale.
    ServeEnsure,
    /// `scripts/reap-orphans`, through `serve status` after its sweep.
    ReapOrphans,
    /// `server.status`, on any surface.
    ServerStatus,
}

impl RuntimeWriter {
    /// The reader `server.status` records as, from the value of [`RUNTIME_WRITER_ENV`]: only
    /// a reader may be named this way, never the serving process.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::server::RuntimeWriter;
    /// assert_eq!(RuntimeWriter::reader_named(Some("reap_orphans")), RuntimeWriter::ReapOrphans);
    /// // the serving process is never a name a reader can take
    /// assert_eq!(RuntimeWriter::reader_named(Some("server")), RuntimeWriter::ServerStatus);
    /// ```
    pub fn reader_named(word: Option<&str>) -> RuntimeWriter {
        match word {
            Some("reap_orphans") => RuntimeWriter::ReapOrphans,
            _ => RuntimeWriter::ServerStatus,
        }
    }
}

/// The last transition of a checkout's runtime: what it became, from what, when, why, who
/// recorded it and which process it is about. The record under [`RUNTIME_PATH`].
///
/// ```
/// use majordomus_cli::capability::builtin::server::{RuntimeTransition, RuntimeWriter, ServerStanding};
/// let t = RuntimeTransition::new(ServerStanding::Ready, None, "published", RuntimeWriter::Server, Some(7));
/// let v = serde_json::to_value(&t).unwrap();
/// assert_eq!(v["state"], "ready");
/// assert_eq!(v["by"], "server");
/// assert!(v.get("from").is_none(), "a first record came from nothing it can name");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeTransition {
    /// [`RUNTIME_SCHEMA`].
    pub schema: String,
    /// What the runtime became.
    pub state: ServerStanding,
    /// What it was, by the previous record; absent for the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<ServerStanding>,
    /// When, RFC 3339.
    pub at: String,
    /// Why, in the recorder's words.
    pub reason: String,
    /// Who recorded it.
    pub by: RuntimeWriter,
    /// The server process the transition is about, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl RuntimeTransition {
    /// A transition to `state`, recorded now.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::server::{RuntimeTransition, RuntimeWriter, ServerStanding};
    /// let t = RuntimeTransition::new(ServerStanding::Failed, Some(ServerStanding::Ready),
    ///     "pid 9 is gone", RuntimeWriter::ServeEnsure, Some(9));
    /// assert_eq!((t.state, t.from, t.pid), (ServerStanding::Failed, Some(ServerStanding::Ready), Some(9)));
    /// assert!(t.at.ends_with('Z'), "RFC 3339, in UTC: {}", t.at);
    /// ```
    pub fn new(
        state: ServerStanding,
        from: Option<ServerStanding>,
        reason: &str,
        by: RuntimeWriter,
        pid: Option<u32>,
    ) -> Self {
        RuntimeTransition {
            schema: RUNTIME_SCHEMA.into(),
            state,
            from,
            at: crate::peers::rfc3339(std::time::SystemTime::now()),
            reason: reason.into(),
            by,
            pid,
        }
    }
}

/// Where a checkout's runtime stands now, and the last transition recorded for it.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{RuntimeState, ServerStanding};
/// let r = RuntimeState { state: ServerStanding::Operational, reason: None, transition: None };
/// assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({ "state": "operational" }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeState {
    /// The state, derived on this reading from the lease, the server's readiness report (its
    /// mesh verdict included) and the last recorded transition.
    pub state: ServerStanding,
    /// Why, when the state is not `operational`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The last transition recorded under [`RUNTIME_PATH`], when one ever was. The derived
    /// state may be newer than it: only the writers named on [`RuntimeWriter`] record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<RuntimeTransition>,
}

/// Where the runtime record of the checkout at `root` lives, given its local half.
///
/// ```
/// use majordomus_cli::capability::builtin::server::runtime_file;
/// use std::path::Path;
/// assert_eq!(runtime_file(Path::new("/r"), ".ai/local"), Path::new("/r/.ai/local/state/runtime/state.json"));
/// ```
pub fn runtime_file(root: &Path, local_half: &str) -> PathBuf {
    root.join(local_half).join(RUNTIME_PATH)
}

/// The runtime record beside a lease file, when `lease` is where a lease lives
/// (`<local half>/state/mcp/server.json`); `None` for any other path, so that a lease kept
/// somewhere unusual never has a record written outside its local half.
///
/// ```
/// use majordomus_cli::capability::builtin::server::runtime_file_beside;
/// use std::path::Path;
/// assert_eq!(
///     runtime_file_beside(Path::new("/r/.ai/local/state/mcp/server.json")).unwrap(),
///     Path::new("/r/.ai/local/state/runtime/state.json")
/// );
/// assert!(runtime_file_beside(Path::new("/tmp/server.json")).is_none());
/// ```
pub fn runtime_file_beside(lease: &Path) -> Option<PathBuf> {
    if !lease.ends_with(lease::LEASE_PATH) {
        return None;
    }
    let local = lease
        .ancestors()
        .nth(Path::new(lease::LEASE_PATH).components().count())?;
    Some(local.join(RUNTIME_PATH))
}

/// The last transition recorded at `path`; `None` when nothing was recorded, or the file is
/// not a record of this schema (a record nobody can read is no history).
///
/// ```
/// use majordomus_cli::capability::builtin::server::read_transition;
/// assert!(read_transition(std::path::Path::new("/nonexistent/state.json")).is_none());
/// ```
pub fn read_transition(path: &Path) -> Option<RuntimeTransition> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<RuntimeTransition>(&text)
        .ok()
        .filter(|t| t.schema == RUNTIME_SCHEMA)
}

/// Record a transition to `to` at `path`, only when the state changes: an unchanged state
/// writes nothing, so that a reader asking every second leaves the file alone. Returns
/// whether it wrote. The write is a whole file renamed into place, so a reader sees the
/// previous record or this one and never half of either.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{record_transition, read_transition, RuntimeWriter, ServerStanding};
/// let dir = tempfile::tempdir().unwrap();
/// let file = dir.path().join("state/runtime/state.json");
/// assert!(record_transition(&file, ServerStanding::Starting, "took the lease", RuntimeWriter::Server, Some(7)).unwrap());
/// assert!(!record_transition(&file, ServerStanding::Starting, "again", RuntimeWriter::Server, Some(7)).unwrap());
/// assert!(record_transition(&file, ServerStanding::Ready, "published", RuntimeWriter::Server, Some(7)).unwrap());
/// let t = read_transition(&file).unwrap();
/// assert_eq!((t.state, t.from), (ServerStanding::Ready, Some(ServerStanding::Starting)));
/// ```
pub fn record_transition(
    path: &Path,
    to: ServerStanding,
    reason: &str,
    by: RuntimeWriter,
    pid: Option<u32>,
) -> std::io::Result<bool> {
    let previous = read_transition(path);
    if previous.as_ref().is_some_and(|p| p.state == to) {
        return Ok(false);
    }
    let record = RuntimeTransition::new(to, previous.map(|p| p.state), reason, by, pid);
    let text = serde_json::to_string_pretty(&record).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, text + "\n")
        .and_then(|()| std::fs::rename(&tmp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
    Ok(true)
}

/// Decide where a checkout's runtime stands, from the lease's standing ([`standing_of`]),
/// the lease itself, the last recorded transition, the server's readiness report (with its
/// mesh verdict) when it was asked, and whether a process id names a live process.
///
/// Pure, so that every state is a test:
///
/// ```
/// use majordomus_cli::capability::builtin::server::{runtime_of, ServerStanding as S};
/// use majordomus_cli::lease::LeaseFile;
/// // no lease and no history: absent, as the lease says
/// assert_eq!(runtime_of(S::Absent, &LeaseFile::Absent, None, None, |_| false).0, S::Absent);
/// // a lease that answers, with nobody asked about its readiness: ready, as the lease says
/// assert_eq!(runtime_of(S::Ready, &LeaseFile::Absent, None, None, |_| true).0, S::Ready);
/// ```
pub fn runtime_of(
    standing: ServerStanding,
    lease: &LeaseFile,
    recorded: Option<&RuntimeTransition>,
    readiness: Option<&super::health::Readiness>,
    alive: impl Fn(u32) -> bool,
) -> (ServerStanding, Option<String>) {
    let doc = lease.document();
    let recorded_as = |state: ServerStanding| recorded.filter(|r| r.state == state);
    // a lease holder that said it is stopping, and is still here
    if let (Some(doc), Some(r)) = (doc, recorded_as(ServerStanding::Stopping)) {
        if r.pid == Some(doc.pid) && alive(doc.pid) {
            return (
                ServerStanding::Stopping,
                Some(format!(
                    "pid {} recorded at {}: {}",
                    doc.pid, r.at, r.reason
                )),
            );
        }
    }
    match standing {
        ServerStanding::Absent => match recorded.map(|r| r.state) {
            Some(ServerStanding::Building) if recorded.and_then(|r| r.pid).is_none_or(&alive) => {
                let r = recorded.expect("matched");
                (
                    ServerStanding::Building,
                    Some(format!("recorded at {}: {}", r.at, r.reason)),
                )
            }
            Some(ServerStanding::Stopping | ServerStanding::Stopped) => {
                let r = recorded.expect("matched");
                (
                    ServerStanding::Stopped,
                    Some(format!(
                        "no lease; the server recorded stopping and left (last record at {}: {})",
                        r.at, r.reason
                    )),
                )
            }
            Some(ServerStanding::Failed) => {
                let r = recorded.expect("matched");
                (
                    ServerStanding::Failed,
                    Some(format!("no lease; recorded at {}: {}", r.at, r.reason)),
                )
            }
            _ => (ServerStanding::Absent, None),
        },
        ServerStanding::Stale => match doc {
            Some(doc) if doc.pid != 0 && !alive(doc.pid) => {
                let stopped = recorded_as(ServerStanding::Stopping)
                    .or(recorded_as(ServerStanding::Stopped))
                    .is_some_and(|r| r.pid.is_none_or(|p| p == doc.pid));
                if stopped {
                    (
                        ServerStanding::Stopped,
                        Some(format!(
                            "pid {} is gone after it recorded stopping; its lease was left behind",
                            doc.pid
                        )),
                    )
                } else {
                    (
                        ServerStanding::Failed,
                        Some(format!(
                            "pid {} is gone and never recorded stopping: it ended without stopping in order",
                            doc.pid
                        )),
                    )
                }
            }
            _ => (ServerStanding::Stale, None),
        },
        ServerStanding::Ready => match readiness {
            None => (ServerStanding::Ready, None),
            Some(r) if !r.ready || r.layer != super::health::HealthStatus::Ok => (
                ServerStanding::Degraded,
                Some(if r.reasons.is_empty() {
                    "the server's readiness report says it is not ready".to_string()
                } else {
                    r.reasons.join("; ")
                }),
            ),
            Some(r) => match &r.mesh {
                Some(m) if m.attaching => (ServerStanding::MeshAttaching, Some(m.detail.clone())),
                Some(m) if !m.ok => (ServerStanding::Degraded, Some(m.detail.clone())),
                _ => (ServerStanding::Operational, None),
            },
        },
        other => (other, None),
    }
}

/// The runtime state of the checkout at `worktree`, as a reader finds it, recording
/// `stopped` or `failed` as `by` when the server is found gone and the record does not say
/// so yet. The one place a reader writes, so that `server.status`, `serve ensure` and
/// `scripts/reap-orphans` record the same thing.
///
/// ```
/// use majordomus_cli::capability::builtin::server::{runtime_at, RuntimeWriter, ServerStanding};
/// use majordomus_cli::lease::LeaseFile;
/// let dir = tempfile::tempdir().unwrap();
/// let r = runtime_at(dir.path(), ".ai/local", ServerStanding::Absent, &LeaseFile::Absent, None, RuntimeWriter::ServerStatus);
/// assert_eq!(r.state, ServerStanding::Absent);
/// assert!(r.transition.is_none() && !dir.path().join(".ai").exists(), "nothing found, nothing written");
/// ```
pub fn runtime_at(
    worktree: &Path,
    local_half: &str,
    standing: ServerStanding,
    lease: &LeaseFile,
    readiness: Option<&super::health::Readiness>,
    by: RuntimeWriter,
) -> RuntimeState {
    let file = runtime_file(worktree, local_half);
    let RuntimeState {
        state,
        reason,
        transition: recorded,
    } = runtime_read(worktree, local_half, standing, lease, readiness);
    let gone = matches!(state, ServerStanding::Stopped | ServerStanding::Failed);
    let transition = if gone && recorded.as_ref().is_none_or(|r| r.state != state) {
        let pid = lease
            .document()
            .map(|d| d.pid)
            .or(recorded.as_ref().and_then(|r| r.pid));
        let written =
            record_transition(&file, state, reason.as_deref().unwrap_or_default(), by, pid);
        if let Err(e) = &written {
            tracing::warn!(path = %file.display(), "could not record the runtime's transition: {e}");
        }
        read_transition(&file).or(recorded)
    } else {
        recorded
    };
    RuntimeState {
        state,
        reason,
        transition,
    }
}

/// The runtime state of the checkout at `worktree`, read and never written: what a surface
/// that is not one of the named readers shows (the briefing, `/ui`).
///
/// ```
/// use majordomus_cli::capability::builtin::server::{runtime_read, ServerStanding};
/// use majordomus_cli::lease::LeaseFile;
/// let dir = tempfile::tempdir().unwrap();
/// let r = runtime_read(dir.path(), ".ai/local", ServerStanding::Absent, &LeaseFile::Absent, None);
/// assert_eq!((r.state, r.transition), (ServerStanding::Absent, None));
/// ```
pub fn runtime_read(
    worktree: &Path,
    local_half: &str,
    standing: ServerStanding,
    lease: &LeaseFile,
    readiness: Option<&super::health::Readiness>,
) -> RuntimeState {
    let recorded = read_transition(&runtime_file(worktree, local_half));
    let (state, reason) = runtime_of(standing, lease, recorded.as_ref(), readiness, lease::alive);
    RuntimeState {
        state,
        reason,
        transition: recorded,
    }
}

/// The readiness report a runtime is refined with, mesh verdict included: from memory for
/// this process's own server, asked once of any other server that answers current, and not
/// asked at all of one that does not. Every surface that shows the runtime asks through here,
/// so that `server.status` and the briefing cannot refine one lease two ways.
pub(crate) fn readiness_for(
    ctx: &Context,
    standing: ServerStanding,
    read: &LeaseFile,
) -> Option<super::health::Readiness> {
    let doc = read.document();
    match (standing, doc.and_then(|d| d.url.as_deref())) {
        (ServerStanding::Ready, _) if doc.is_some_and(lease::is_own) => {
            Some(super::health::readiness_report(ctx))
        }
        (ServerStanding::Ready, Some(url)) => readiness_of(ctx, url),
        _ => None,
    }
}

/// The readiness report of the server at `url`, asked over the route the registry declares
/// for `health.ready`, bounded by the declared probe timeout. `None` when it does not answer
/// with one.
fn readiness_of(ctx: &Context, url: &str) -> Option<super::health::Readiness> {
    let path = ctx
        .registry
        .get("health.ready")?
        .exposure
        .http
        .as_ref()?
        .path
        .clone();
    let reply =
        crate::mcp::bridge::request(url, "GET", &path, &[], None, lease::timings().probe_timeout)
            .ok()?;
    (reply.status == 200)
        .then(|| serde_json::from_str(&reply.body).ok())
        .flatten()
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
    // the probe timeout the policy declares, as the election reads it (I2129)
    let timeout = lease::timings().probe_timeout;
    let reply = crate::mcp::bridge::request(url, "GET", &path, &[], None, timeout).ok()?;
    if reply.status != 200 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&reply.body).ok()?;
    value["count"].as_u64().map(|n| n as usize)
}

/// The checkouts one reading covers: every checkout git registers for the repository, the
/// primary first, or this one alone.
///
/// The narrow answer is the cheap one by construction rather than by filtering — git is
/// not asked for its registry of work trees, so nothing downstream has another checkout's
/// path to read a lease at or a server to probe. It spawns no process of its own either:
/// `primary` comes from the one stat `git_identity` already did (a linked work tree has a
/// `.git` file, the primary a directory) and the branch from that checkout's own HEAD, so
/// the narrow entry says everything about this checkout that the wide list would have
/// said, at the price of two file reads.
pub(crate) fn checkouts_of(
    root: &Path,
    git: Option<&GitIdentity>,
    which: Checkouts,
) -> Vec<(PathBuf, Option<String>, bool)> {
    let here = |primary: bool| vec![(root.to_path_buf(), repository::branch_at(root), primary)];
    let Some(git) = git else {
        // not a git repository: this checkout is the only one there is, and it is the
        // primary of nothing
        return here(true);
    };
    if which == Checkouts::This {
        return here(!git.linked);
    }
    match crate::worktree::topology::read(root) {
        Ok(records) => records
            .into_iter()
            .filter(|r| !r.bare)
            .enumerate()
            .map(|(i, r)| (r.path, r.branch, i == 0))
            .collect(),
        Err(_) => here(!git.linked),
    }
}

/// How many checkouts one reading asks at once. Each is a lease read and, for a server that
/// answers, a round trip bounded by the probe timeout.
pub(crate) const ASKED_AT_ONCE: usize = 16;

/// `ask` applied to every checkout, [`ASKED_AT_ONCE`] at a time, the answers in the order of
/// the checkouts. A worker the system refuses to start is not needed: the calling thread
/// asks too, so every checkout is answered with however many threads there are.
pub(crate) fn each_checkout<C: Sync, T: Send>(
    checkouts: &[C],
    ask: impl Fn(&C) -> T + Sync,
) -> Vec<T> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, PoisonError};
    if checkouts.len() <= 1 {
        return checkouts.iter().map(ask).collect();
    }
    let next = AtomicUsize::new(0);
    let answers: Vec<Mutex<Option<T>>> = checkouts.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        let work = || loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(checkout) = checkouts.get(i) else {
                break;
            };
            let answer = ask(checkout);
            *answers[i].lock().unwrap_or_else(PoisonError::into_inner) = Some(answer);
        };
        for _ in 1..ASKED_AT_ONCE.min(checkouts.len()) {
            let _ = std::thread::Builder::new()
                .name("mj-checkouts".into())
                .spawn_scoped(scope, work);
        }
        work();
    });
    answers
        .into_iter()
        .filter_map(|a| a.into_inner().unwrap_or_else(PoisonError::into_inner))
        .collect()
}

fn server_status(ctx: &Context, input: ServerStatusInput) -> Result<ServerStatus, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let local_half = local_half(&root);
    let git = repository::git_identity(&root);
    let linked = git.as_ref().is_some_and(|g| g.linked);
    let checkouts = checkouts_of(&root, git.as_ref(), input.checkouts);
    let writer = RuntimeWriter::reader_named(std::env::var(RUNTIME_WRITER_ENV).ok().as_deref());
    let servers = each_checkout(&checkouts, |(path, branch, primary)| {
        let worktree = path.canonicalize().unwrap_or(path.clone());
        let (standing, reason, read) = standing_at(&worktree, &local_half);
        let own = read.document().is_some_and(lease::is_own);
        let url = read.document().and_then(|d| d.url.as_deref());
        let peers = match standing {
            // this process's own board is in memory; asking it over the wire would queue
            // behind this very request (lease::is_own)
            ServerStanding::Ready | ServerStanding::Outdated if own => {
                Some(ctx.peers.list().iter().filter(|p| p.attached).count())
            }
            ServerStanding::Ready | ServerStanding::Outdated => {
                url.and_then(|url| peers_of(ctx, url))
            }
            _ => None,
        };
        let readiness = readiness_for(ctx, standing, &read);
        let runtime = runtime_at(
            &worktree,
            &local_half,
            standing,
            &read,
            readiness.as_ref(),
            writer,
        );
        ServerView {
            checkout_id: repository::identity(&worktree),
            primary: *primary,
            this_checkout: worktree == root,
            branch: branch.clone(),
            standing,
            reason,
            lease: read.document().map(LeaseView::of),
            peers,
            runtime: Some(runtime),
            worktree,
        }
    });
    let this = servers.iter().find(|s| s.this_checkout);
    let standing = this.map(|s| s.standing).unwrap_or(ServerStanding::Absent);
    let runtime = this.and_then(|s| s.runtime.clone());
    Ok(ServerStatus {
        checkout_id: repository::identity(&root),
        git,
        desired: Desired {
            host: desired_host(linked),
            // the port this checkout asks for on every start (I2702)
            port: crate::cli::checkout_port(&root),
            version: crate::VERSION.into(),
            executable: lease::executable_identity(),
        },
        this_process: lease::held().as_ref().map(LeaseView::of),
        standing,
        servers,
        connections: crate::http::deadline::counts(),
        trunk: Some(crate::worktree::trunk::measure_checkout(&root)),
        runtime,
    })
}

/// The interface a server started from this environment would bind — the same resolution
/// `serve` and `mcp` make for this checkout, without a flag, so that `desired` and a bind
/// cannot disagree.
fn desired_host(linked: bool) -> String {
    let environment = std::env::var(crate::cli::HTTP_HOST_ENV).ok();
    crate::cli::resolve_checkout_http_host(None, environment.as_deref(), linked).0
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
        description: "The shared server of this checkout and of every other checkout of the same git repository: what each lease says, whether the server it names answers, whether what answers is the code on disk at this executable's version, where each runtime stands and how it got there, and how many peers each one holds.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "server.status",
                title: "The shared server, and every server of the repository",
                description: "Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; where its runtime stands (`runtime`: that word refined by the server's readiness report, its mesh verdict and the last recorded transition into building, mesh_attaching, operational, degraded, stopping, stopped or failed, with the time, reason and recorder of that transition, kept under `.ai/local/state/runtime/` and written only when the state changes — this reader records `stopped` or `failed` for a server it finds gone); how far the tree it serves is from the remote trunk (`trunk`: current, lagging, stale, unjudged or unmeasured against the policy's `checkout.trunk_distance`, as of the last fetch); the lease this process holds when it is the server; and, unless `checkouts` narrows it to this one, every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, its runtime, and the peers its server reports. `checkouts: this` reads one lease and probes one server and enumerates no other checkout, which is what a caller asking only about the checkout it is in should pay. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes.",
                input: ServerStatusInput,
                output: ServerStatus,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_server".into()),
                        resource: Some(McpResource { uri: SERVER_URI.into(), name: "server".into() }),
                    }),
                    http: get("/api/v1/server"),
                    cli: Some(CliExposure { path: vec!["serve".into(), "status".into()] }),
                },
                tags: ["server", "lease", "coordination", "introspection"],
                cache: CachePolicy::Disabled,
                handler: server_status,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every checkout is answered, in the order given, and several are asked at once but
    /// never more than [`ASKED_AT_ONCE`]: one at a time is what made a repository of sixty
    /// servers wait half a minute.
    #[test]
    fn every_checkout_is_asked_at_once_and_answered_in_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let asking = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);
        let checkouts: Vec<usize> = (0..40).collect();
        let answers = each_checkout(&checkouts, |i| {
            let now = asking.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(20));
            asking.fetch_sub(1, Ordering::SeqCst);
            i * 2
        });
        assert_eq!(answers, (0..40).map(|i| i * 2).collect::<Vec<_>>());
        let most = most.load(Ordering::SeqCst);
        assert!(most > 1, "the checkouts were asked one at a time");
        assert!(most <= ASKED_AT_ONCE, "{most} asked at once");
        assert_eq!(each_checkout(&[7usize], |i| *i), vec![7]);
        assert!(each_checkout(&Vec::<usize>::new(), |i| *i).is_empty());
    }

    /// A lease document of a healthy server that published `version`.
    fn serving(version: &str) -> LeaseFile {
        LeaseFile::Document(LeaseDocument {
            schema: lease::SCHEMA.into(),
            pid: 1,
            token: "t".into(),
            root: "/r".into(),
            url: Some("http://127.0.0.1:1".into()),
            started_at: "2026-09-10T00:00:00Z".into(),
            executable: None,
            version: Some(version.into()),
        })
    }

    /// A server whose version cannot be ordered against this executable's — one that is not
    /// a semantic version, a local build's tag or a malformed pre-release — is outdated, and
    /// the reason names both versions without claiming which is older.
    #[test]
    fn a_version_that_cannot_be_ordered_is_outdated_naming_both() {
        for theirs in ["local-build", "1.0.0-rc.01"] {
            let (standing, reason) =
                standing_of(&serving(theirs), Duration::ZERO, |_| true, "1.0.0");
            assert_eq!(standing, ServerStanding::Outdated, "{theirs}");
            assert_eq!(
                reason.expect("a reason"),
                format!("the server is serving version {theirs}; this executable is 1.0.0")
            );
        }
    }

    /// A pre-release is a version, ordered by semantic-version precedence: a server at
    /// 1.0.0-rc.1 is older than an executable at 1.0.0, and that executable replaces it.
    #[test]
    fn a_prerelease_server_is_older_than_its_final_and_replaced() {
        let (standing, reason) =
            standing_of(&serving("1.0.0-rc.1"), Duration::ZERO, |_| true, "1.0.0");
        assert_eq!(standing, ServerStanding::Outdated);
        assert_eq!(
            reason.expect("a reason"),
            "the server is serving version 1.0.0-rc.1, older than this executable (1.0.0); \
             the next client or `serve ensure` of this executable replaces it"
        );
    }

    // ------------------------------------------------------------ the runtime state (I3179)

    use super::super::health::{HealthStatus, MeshReadiness, Readiness};
    use std::io::{Read as _, Write as _};

    /// A pid no process has: far above any pid this machine hands out.
    const DEAD: u32 = 9_999_999;

    /// A lease document of process `pid`, published at `url`.
    fn lease_of(pid: u32, url: Option<&str>) -> LeaseFile {
        LeaseFile::Document(LeaseDocument {
            schema: lease::SCHEMA.into(),
            pid,
            token: "t".into(),
            root: "/r".into(),
            url: url.map(str::to_string),
            started_at: "2026-10-23T00:00:00Z".into(),
            executable: None,
            version: Some(crate::VERSION.into()),
        })
    }

    /// A recorded transition to `state`, about `pid`.
    fn recorded(state: ServerStanding, pid: Option<u32>) -> RuntimeTransition {
        RuntimeTransition::new(state, None, "recorded", RuntimeWriter::Server, pid)
    }

    /// A readiness report: ready or not, the layer's word, the mesh.
    fn readiness(ready: bool, layer: HealthStatus, mesh: Option<MeshReadiness>) -> Readiness {
        Readiness {
            ready,
            version: crate::VERSION.into(),
            commit: "c".into(),
            dirty: None,
            capabilities: 1,
            objects: 1,
            layer,
            stale: None,
            reasons: Vec::new(),
            mesh,
        }
    }

    fn mesh(attaching: bool, ok: bool) -> MeshReadiness {
        MeshReadiness {
            enabled: true,
            attaching,
            ok,
            detail: "the mesh".into(),
        }
    }

    /// Every pid is alive, or none is.
    fn alive(_: u32) -> bool {
        true
    }
    fn dead(_: u32) -> bool {
        false
    }

    /// The five words the lease decides are the words they always were, in the JSON and on
    /// every surface, and the seven the runtime adds are new words: no old word was renamed
    /// or reused (I3179's acceptance: the old words are pinned).
    #[test]
    fn the_lease_words_are_pinned_and_the_runtime_words_are_new() {
        let old = [
            (ServerStanding::Absent, "absent"),
            (ServerStanding::Starting, "starting"),
            (ServerStanding::Ready, "ready"),
            (ServerStanding::Outdated, "outdated"),
            (ServerStanding::Stale, "stale"),
        ];
        for (state, word) in old {
            assert_eq!(state.as_str(), word);
            assert_eq!(serde_json::to_value(state).unwrap(), word);
            assert_eq!(
                serde_json::from_value::<ServerStanding>(word.into()).unwrap(),
                state
            );
            assert_eq!(state.refines(), state, "a lease word refines itself");
        }
        let new = [
            (
                ServerStanding::Building,
                "building",
                ServerStanding::Starting,
            ),
            (
                ServerStanding::MeshAttaching,
                "mesh_attaching",
                ServerStanding::Ready,
            ),
            (
                ServerStanding::Operational,
                "operational",
                ServerStanding::Ready,
            ),
            (ServerStanding::Degraded, "degraded", ServerStanding::Ready),
            (ServerStanding::Stopping, "stopping", ServerStanding::Ready),
            (ServerStanding::Stopped, "stopped", ServerStanding::Absent),
            (ServerStanding::Failed, "failed", ServerStanding::Stale),
        ];
        for (state, word, base) in new {
            assert_eq!(state.as_str(), word);
            assert_eq!(serde_json::to_value(state).unwrap(), word);
            assert_eq!(
                serde_json::from_value::<ServerStanding>(word.into()).unwrap(),
                state
            );
            assert_eq!(state.refines(), base, "{word}");
            assert!(old.iter().all(|(_, w)| *w != word));
        }
        // the lease's decision still says only its own five words
        for file in [
            LeaseFile::Absent,
            LeaseFile::Empty,
            LeaseFile::Corrupt("x".into()),
        ] {
            let (s, _) = standing_of(&file, Duration::ZERO, |_| true, crate::VERSION);
            assert_eq!(s.refines(), s, "{s:?}");
        }
    }

    /// One reading of [`runtime_of`]: what it is, what it decided, and what it must decide.
    type Case = (
        &'static str,
        (ServerStanding, Option<String>),
        ServerStanding,
    );

    /// Each of the twelve states is produced by one reading, and every branch of the
    /// decision is reached (I3179's acceptance: each state has a test that produces it).
    #[test]
    fn every_runtime_state_is_produced_by_a_reading() {
        use ServerStanding as S;
        let up = lease_of(7, Some("http://127.0.0.1:1"));
        let gone = lease_of(DEAD, Some("http://127.0.0.1:1"));
        let ok = readiness(true, HealthStatus::Ok, None);
        let not_ready = Readiness {
            reasons: vec!["lease_lost: another".into()],
            ..readiness(false, HealthStatus::Ok, None)
        };
        let cases: Vec<Case> = vec![
            (
                "no lease, no history",
                runtime_of(S::Absent, &LeaseFile::Absent, None, None, alive),
                S::Absent,
            ),
            (
                "no lease, a live process recorded a live state",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Ready, Some(7))),
                    None,
                    alive,
                ),
                S::Absent,
            ),
            (
                "binding",
                runtime_of(S::Starting, &LeaseFile::Empty, None, None, alive),
                S::Starting,
            ),
            (
                "answers, readiness not asked",
                runtime_of(S::Ready, &up, None, None, alive),
                S::Ready,
            ),
            (
                "answers from other code",
                runtime_of(S::Outdated, &up, None, None, alive),
                S::Outdated,
            ),
            (
                "corrupt lease",
                runtime_of(S::Stale, &LeaseFile::Corrupt("x".into()), None, None, dead),
                S::Stale,
            ),
            (
                "silent but alive",
                runtime_of(S::Stale, &up, None, None, alive),
                S::Stale,
            ),
            (
                "pid 0 names nobody to judge",
                runtime_of(S::Stale, &lease_of(0, None), None, None, dead),
                S::Stale,
            ),
            (
                "a build recorded by a live builder",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Building, Some(7))),
                    None,
                    alive,
                ),
                S::Building,
            ),
            (
                "a build recorded by nobody in particular",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Building, None)),
                    None,
                    dead,
                ),
                S::Building,
            ),
            (
                "a build whose builder is gone",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Building, Some(7))),
                    None,
                    dead,
                ),
                S::Absent,
            ),
            (
                "mesh deciding",
                runtime_of(
                    S::Ready,
                    &up,
                    None,
                    Some(&readiness(true, HealthStatus::Ok, Some(mesh(true, true)))),
                    alive,
                ),
                S::MeshAttaching,
            ),
            (
                "ready, no mesh",
                runtime_of(S::Ready, &up, None, Some(&ok), alive),
                S::Operational,
            ),
            (
                "ready, mesh running",
                runtime_of(
                    S::Ready,
                    &up,
                    None,
                    Some(&readiness(true, HealthStatus::Ok, Some(mesh(false, true)))),
                    alive,
                ),
                S::Operational,
            ),
            (
                "not ready, with reasons",
                runtime_of(S::Ready, &up, None, Some(&not_ready), alive),
                S::Degraded,
            ),
            (
                "not ready, without reasons",
                runtime_of(
                    S::Ready,
                    &up,
                    None,
                    Some(&readiness(false, HealthStatus::Ok, None)),
                    alive,
                ),
                S::Degraded,
            ),
            (
                "a degraded layer",
                runtime_of(
                    S::Ready,
                    &up,
                    None,
                    Some(&readiness(true, HealthStatus::Warn, None)),
                    alive,
                ),
                S::Degraded,
            ),
            (
                "a declared mesh not running",
                runtime_of(
                    S::Ready,
                    &up,
                    None,
                    Some(&readiness(true, HealthStatus::Ok, Some(mesh(false, false)))),
                    alive,
                ),
                S::Degraded,
            ),
            (
                "stopping, still here",
                runtime_of(
                    S::Ready,
                    &up,
                    Some(&recorded(S::Stopping, Some(7))),
                    None,
                    alive,
                ),
                S::Stopping,
            ),
            (
                "another generation's stopping",
                runtime_of(
                    S::Ready,
                    &up,
                    Some(&recorded(S::Stopping, Some(8))),
                    None,
                    alive,
                ),
                S::Ready,
            ),
            (
                "gone after stopping, lease left",
                runtime_of(
                    S::Stale,
                    &gone,
                    Some(&recorded(S::Stopping, Some(DEAD))),
                    None,
                    dead,
                ),
                S::Stopped,
            ),
            (
                "gone, recorded stopped already",
                runtime_of(
                    S::Stale,
                    &gone,
                    Some(&recorded(S::Stopped, None)),
                    None,
                    dead,
                ),
                S::Stopped,
            ),
            (
                "no lease after stopping",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Stopping, Some(7))),
                    None,
                    dead,
                ),
                S::Stopped,
            ),
            (
                "no lease, stopped",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Stopped, Some(7))),
                    None,
                    dead,
                ),
                S::Stopped,
            ),
            (
                "gone without stopping",
                runtime_of(
                    S::Stale,
                    &gone,
                    Some(&recorded(S::Ready, Some(DEAD))),
                    None,
                    dead,
                ),
                S::Failed,
            ),
            (
                "gone, another pid's stopping",
                runtime_of(
                    S::Stale,
                    &gone,
                    Some(&recorded(S::Stopping, Some(8))),
                    None,
                    dead,
                ),
                S::Failed,
            ),
            (
                "no lease, failed",
                runtime_of(
                    S::Absent,
                    &LeaseFile::Absent,
                    Some(&recorded(S::Failed, Some(7))),
                    None,
                    dead,
                ),
                S::Failed,
            ),
        ];
        let mut produced = std::collections::BTreeSet::new();
        for (what, (state, reason), want) in &cases {
            assert_eq!(state, want, "{what}: {reason:?}");
            let explained = !matches!(
                state,
                S::Absent | S::Starting | S::Ready | S::Outdated | S::Stale | S::Operational
            );
            assert_eq!(reason.is_some(), explained, "{what}: {reason:?}");
            produced.insert(state.as_str());
        }
        assert_eq!(produced.len(), 12, "{produced:?}");
        // the reasons say what they rest on
        assert_eq!(cases[14].1 .1.as_deref(), Some("lease_lost: another"));
        assert_eq!(
            cases[15].1 .1.as_deref(),
            Some("the server's readiness report says it is not ready")
        );
        assert!(cases[24]
            .1
             .1
            .as_deref()
            .unwrap()
            .contains("never recorded stopping"));
    }

    /// One whole life, recorded by the writers the issue names: the serving process records
    /// its own `starting`, `ready` and `stopping` through its lease; the reader finds it
    /// gone and records `stopped`; a server that dies without stopping is found by a reader
    /// that records `failed`; and nothing is ever written into the lease's own directory.
    #[test]
    fn test_runtime_state_transitions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
        std::fs::write(
            dir.path().join(".ai/manifest.yaml"),
            "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n",
        )
        .unwrap();
        let repo = Repository::discover(dir.path()).unwrap();
        let root = repo.root().to_path_buf();
        let local = repo.local_path();
        let file = runtime_file(&root, &local);
        let state = || read_transition(&file).map(|t| (t.state, t.by, t.pid));
        let me = Some(std::process::id());

        let lease::Role::Server(held) = lease::elect(&repo).unwrap() else {
            panic!("nobody serves a fresh directory")
        };
        assert_eq!(
            state(),
            Some((ServerStanding::Starting, RuntimeWriter::Server, me))
        );
        held.publish("http://127.0.0.1:1").unwrap();
        let t = read_transition(&file).unwrap();
        assert_eq!(
            (t.state, t.from),
            (ServerStanding::Ready, Some(ServerStanding::Starting))
        );
        assert_eq!(t.reason, "published http://127.0.0.1:1");
        held.release();
        assert_eq!(
            state(),
            Some((ServerStanding::Stopping, RuntimeWriter::Server, me))
        );

        // the lease is gone: a reader finds the server stopped, and records it as itself
        let r = runtime_at(
            &root,
            &local,
            ServerStanding::Absent,
            &LeaseFile::Absent,
            None,
            RuntimeWriter::ServerStatus,
        );
        assert_eq!(r.state, ServerStanding::Stopped);
        assert_eq!(
            state(),
            Some((ServerStanding::Stopped, RuntimeWriter::ServerStatus, me))
        );

        // a server that left its lease and its pid behind, never having said it stops
        let lease_path = lease::lease_file(&root, &local);
        let gone = lease_of(DEAD, Some("http://127.0.0.1:1"));
        std::fs::write(
            &lease_path,
            serde_json::to_string(gone.document().unwrap()).unwrap(),
        )
        .unwrap();
        record_transition(
            &file,
            ServerStanding::Ready,
            "published",
            RuntimeWriter::Server,
            Some(DEAD),
        )
        .unwrap();
        let r = runtime_at(
            &root,
            &local,
            ServerStanding::Stale,
            &gone,
            None,
            RuntimeWriter::ServeEnsure,
        );
        assert_eq!(r.state, ServerStanding::Failed);
        let t = r.transition.expect("the reader recorded it");
        assert_eq!(
            (t.state, t.from, t.by, t.pid),
            (
                ServerStanding::Failed,
                Some(ServerStanding::Ready),
                RuntimeWriter::ServeEnsure,
                Some(DEAD)
            )
        );

        // the record lives beside the lease's directory, never in it (.envrc watches that)
        assert!(file.starts_with(root.join(&local).join("state/runtime")));
        let in_lease_dir: Vec<_> = std::fs::read_dir(lease_path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        assert!(
            in_lease_dir.iter().all(|n| !n.contains("state")),
            "{in_lease_dir:?}"
        );
    }

    /// An unchanged state writes nothing: neither a writer recording the state it already
    /// recorded nor a reader finding the same failure twice touches the file.
    #[test]
    fn an_unchanged_state_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = runtime_file(dir.path(), ".ai/local");
        assert!(record_transition(
            &file,
            ServerStanding::Failed,
            "gone",
            RuntimeWriter::ServerStatus,
            Some(DEAD)
        )
        .unwrap());
        let before = std::fs::read(&file).unwrap();
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!record_transition(
            &file,
            ServerStanding::Failed,
            "gone again",
            RuntimeWriter::ReapOrphans,
            None
        )
        .unwrap());
        let gone = lease_of(DEAD, Some("http://127.0.0.1:1"));
        let r = runtime_at(
            dir.path(),
            ".ai/local",
            ServerStanding::Stale,
            &gone,
            None,
            RuntimeWriter::ServeEnsure,
        );
        assert_eq!(r.state, ServerStanding::Failed);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            modified
        );
        // a reader that derives a live state records nothing at all
        let other = tempfile::tempdir().unwrap();
        let r = runtime_at(
            other.path(),
            ".ai/local",
            ServerStanding::Ready,
            &lease_of(7, Some("http://x")),
            Some(&readiness(true, HealthStatus::Ok, None)),
            RuntimeWriter::ServerStatus,
        );
        assert_eq!(r.state, ServerStanding::Operational);
        assert!(!runtime_file(other.path(), ".ai/local").exists());
    }

    /// The writers are named: a reader records only `stopped` and `failed`, as the reader it
    /// is; `server.status` takes its name from the environment only for the reaper, never for
    /// the serving process; the serving process records only while its lease is its own.
    #[test]
    fn each_transition_names_its_writer() {
        assert_eq!(
            RuntimeWriter::reader_named(Some("reap_orphans")),
            RuntimeWriter::ReapOrphans
        );
        for word in [None, Some("server"), Some("serve_ensure"), Some("")] {
            assert_eq!(
                RuntimeWriter::reader_named(word),
                RuntimeWriter::ServerStatus,
                "{word:?}"
            );
        }
        for by in [
            RuntimeWriter::ServeEnsure,
            RuntimeWriter::ReapOrphans,
            RuntimeWriter::ServerStatus,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let gone = lease_of(DEAD, None);
            let r = runtime_at(
                dir.path(),
                ".ai/local",
                ServerStanding::Stale,
                &gone,
                None,
                by,
            );
            assert_eq!(r.transition.map(|t| t.by), Some(by));
        }
    }

    /// A record that cannot be written leaves the reading intact: the state is still said,
    /// and no transition is claimed that was never recorded.
    #[test]
    fn a_record_that_cannot_be_written_is_reported_not_claimed() {
        let dir = tempfile::tempdir().unwrap();
        // a file where the record's directory would be
        std::fs::create_dir_all(dir.path().join(".ai/local/state")).unwrap();
        std::fs::write(
            dir.path().join(".ai/local/state/runtime"),
            "not a directory",
        )
        .unwrap();
        let file = runtime_file(dir.path(), ".ai/local");
        assert!(record_transition(
            &file,
            ServerStanding::Failed,
            "gone",
            RuntimeWriter::ServerStatus,
            None
        )
        .is_err());
        let r = runtime_at(
            dir.path(),
            ".ai/local",
            ServerStanding::Stale,
            &lease_of(DEAD, None),
            None,
            RuntimeWriter::ServerStatus,
        );
        assert_eq!(r.state, ServerStanding::Failed);
        assert!(r.transition.is_none());
        // a file of another schema is no record
        let other = tempfile::tempdir().unwrap();
        let path = other.path().join("state.json");
        let mut foreign = serde_json::to_value(recorded(ServerStanding::Ready, None)).unwrap();
        foreign["schema"] = "something-else/v1".into();
        std::fs::write(&path, foreign.to_string()).unwrap();
        assert!(read_transition(&path).is_none());
        std::fs::write(&path, "not JSON").unwrap();
        assert!(read_transition(&path).is_none());
    }

    /// A loopback HTTP server answering each request by its path; its URL.
    fn serve_http(answer: impl Fn(&str) -> (u16, String) + Send + 'static) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&head);
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = answer(&path);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        url
    }

    /// `server.status` carries the runtime of the checkout and of each server: asked of
    /// another process's server over its readiness route, and recorded as `failed` by the
    /// reader when the lease names a process that is gone.
    #[test]
    fn server_status_carries_the_runtime_and_records_a_server_it_finds_gone() {
        use crate::synthetic::{Shape, SyntheticRepository};
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let root = repo.root().canonicalize().unwrap();
        let local = local_half(&root);
        let lease_path = lease::lease_file(&root, &local);
        std::fs::create_dir_all(lease_path.parent().unwrap()).unwrap();
        let this = serde_json::json!({ "checkouts": "this" });

        // nobody: absent, and nothing recorded
        let v = ctx.execute("server.status", this.clone()).unwrap();
        assert_eq!(v["standing"], "absent");
        assert_eq!(v["runtime"]["state"], "absent");
        assert_eq!(v["servers"][0]["runtime"]["state"], "absent");

        // another live process's server that answers current and ready: operational
        let id = repository::identity(&root);
        let ready = serde_json::to_string(&readiness(true, HealthStatus::Ok, None)).unwrap();
        let url = serve_http(move |path| match path {
            "/" => (
                200,
                serde_json::json!({ "name": "majordomus", "repository_id": id }).to_string(),
            ),
            "/api/v1/ready" => (200, ready.clone()),
            _ => (404, "{}".into()),
        });
        let live = lease_of(std::process::id(), Some(&url));
        std::fs::write(
            &lease_path,
            serde_json::to_string(live.document().unwrap()).unwrap(),
        )
        .unwrap();
        let v = ctx.execute("server.status", this.clone()).unwrap();
        assert_eq!(v["standing"], "ready", "{v}");
        assert_eq!(v["runtime"]["state"], "operational", "{v}");
        assert!(
            !runtime_file(&root, &local).exists(),
            "a live state is not the reader's to record"
        );
        assert!(readiness_of(&ctx, &url).is_some());

        // one whose readiness route does not answer is still ready, as the lease says
        let silent = serve_http(|_| (500, "{}".into()));
        assert!(readiness_of(&ctx, &silent).is_none());
        assert!(readiness_of(&ctx, "http://127.0.0.1:1").is_none());

        // a lease naming a process that is gone: failed, recorded by server.status
        let gone = lease_of(DEAD, Some("http://127.0.0.1:1"));
        std::fs::write(
            &lease_path,
            serde_json::to_string(gone.document().unwrap()).unwrap(),
        )
        .unwrap();
        let v = ctx.execute("server.status", this).unwrap();
        assert_eq!(v["standing"], "stale", "the lease's word is unchanged");
        assert_eq!(v["runtime"]["state"], "failed");
        assert_eq!(v["runtime"]["transition"]["by"], "server_status");
        assert_eq!(
            read_transition(&runtime_file(&root, &local)).map(|t| t.state),
            Some(ServerStanding::Failed)
        );
    }

    /// The serving process answers its own runtime from memory: its readiness report and its
    /// mesh, never a request to itself; and the briefing shows the same state (I3179). The
    /// address it publishes answers too, so that the reading holds even when another test of
    /// this binary has marked the process's lease lost and it is asked over the wire instead.
    #[test]
    fn the_serving_process_answers_its_own_runtime_and_the_brief_shows_it() {
        use crate::synthetic::{Shape, SyntheticRepository};
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let root = repo.root().canonicalize().unwrap();
        let id = repository::identity(&root);
        let ready = serde_json::to_string(&readiness(true, HealthStatus::Ok, None)).unwrap();
        let url = serve_http(move |path| match path {
            "/" => (
                200,
                serde_json::json!({ "name": "majordomus", "repository_id": id }).to_string(),
            ),
            "/api/v1/ready" => (200, ready.clone()),
            _ => (404, "{}".into()),
        });
        let discovered = Repository::discover(&root).unwrap();
        let lease::Role::Server(held) = lease::elect(&discovered).unwrap() else {
            panic!("nobody serves a fresh synthetic repository")
        };
        held.publish(&url).unwrap();
        let v = ctx
            .execute("server.status", serde_json::json!({ "checkouts": "this" }))
            .unwrap();
        let state: ServerStanding = serde_json::from_value(v["runtime"]["state"].clone()).unwrap();
        assert_eq!(v["standing"], "ready", "{v}");
        assert_eq!(state.refines(), ServerStanding::Ready, "{v}");
        assert_ne!(
            state,
            ServerStanding::Ready,
            "the readiness report was read: {v}"
        );
        assert_eq!(
            v["runtime"]["transition"]["state"], "ready",
            "recorded by the server: {v}"
        );
        assert_eq!(v["runtime"]["transition"]["by"], "server");
        let b = ctx
            .execute(
                "brief.session",
                serde_json::json!({ "checkouts": "this", "local": true }),
            )
            .unwrap();
        assert_eq!(b["runtime"]["state"], v["runtime"]["state"], "{b}");
        assert!(
            b["lines"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l.as_str().is_some_and(|l| l.starts_with("runtime      "))),
            "{b}"
        );
        held.release();
    }
}
