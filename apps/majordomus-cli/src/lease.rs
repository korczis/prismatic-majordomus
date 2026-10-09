//! One shared server per checkout: the lease that decides who it is. The first
//! `majordomus mcp` (or `serve`) to create `state/mcp/server.json` under the checkout's
//! local half owns the server and publishes its URL there; every later process reads the
//! file, checks that the server answers for this root, and attaches to it. A lease whose
//! server does not answer is stale, and the next process takes it over; so is a file that
//! is not a lease document, an empty one, or one whose owner never published a URL. A
//! server that is slow to answer while its process is alive is busy, not stale: the
//! election waits for it for [`BUSY_GRACE`] before it takes anything over. The file lives
//! under `.ai/local/` (never tracked, by the layer's contract), and it is removed when the
//! server stops — including when `SIGTERM`, `SIGINT` or `SIGHUP` asks it to — but only by
//! the process that still holds it: every removal and every rewrite re-reads the file under
//! the lock beside it (`server.lock`) and acts only on a lease that carries the actor's own
//! token (`take_over_with` says why it is a lock).
//!
//! Per *checkout*, and this file said "per repository" until ADR 0044: a linked worktree
//! is a checkout, so it has a manifest, a root, a lease and a server of its own, and on
//! 2026-09-11 this repository had seven of them at once. What is repository-wide is the
//! board, which `peers.list` gathers over these very leases; the election is unchanged.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::mcp::bridge;
use crate::repository::Repository;

/// The lease file, relative to the checkout-local half (`.ai/local/`).
pub const LEASE_PATH: &str = "state/mcp/server.json";

/// The lease file's `schema`.
pub const SCHEMA: &str = "majordomus-mcp-lease/v1";

/// How long a lease without a URL may be (the owner is still binding) before it counts as
/// abandoned.
pub const BIND_GRACE: Duration = Duration::from_secs(15);

/// How long the probe of a published URL waits.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// How long a live server that does not answer its probe is waited for before its lease is
/// taken over.
///
/// A probe that times out is not a dead server. On 2026-09-15 ten MCP clients attached at
/// once to a server a shell entry had started; nine were bridged to it, and the tenth
/// probed while the server was answering the other nine, heard nothing within
/// [`PROBE_TIMEOUT`], took the lease over and started a second server for one checkout
/// (test/cases/192). So a silent probe of a lease whose process is still alive is waited
/// on, and asked again; only a server that stays silent for this long, or whose process is
/// gone, loses its lease. The bound keeps the recovery from a hung server, and it sits
/// well inside [`JOIN_TIMEOUT`] so that the take-over still happens within one election.
pub const BUSY_GRACE: Duration = Duration::from_secs(10);

/// How long a process keeps trying to acquire or join the lease before it gives up and
/// says so: the bind grace with a margin for the probes. The caller then serves its
/// client alone.
pub const JOIN_TIMEOUT: Duration = Duration::from_secs(20);

/// The timings above are the defaults. What a process actually judges a lease contest by is
/// declared in `.ai/repo/policy.yaml`'s `server:` block and read once, here.
///
/// They were compiled constants until 2026-09-15: unchangeable without a rebuild, stated
/// nowhere a reader would look, and invisible to every projection — while `probe_timeout` is
/// the number that decides whether a live but slow owner keeps its lease or is taken over
/// while it is still serving. That is a decision about how this repository is supervised, not
/// an implementation detail, so it belongs in the model.
///
/// A `OnceLock`, because a process reads one policy: the repository is opened once and the
/// answer must not change underneath a contest that is already being judged. Absent keys keep
/// the constants, so a policy that cannot be read does not silently change behaviour.
///
/// ```
/// use majordomus_cli::lease::{Timings, BIND_GRACE, BUSY_GRACE, JOIN_TIMEOUT, PROBE_TIMEOUT};
/// // the defaults are the compiled constants, one for one
/// let t = Timings::default();
/// assert_eq!(t.bind_grace, BIND_GRACE);
/// assert_eq!(t.probe_timeout, PROBE_TIMEOUT);
/// assert_eq!(t.join_timeout, JOIN_TIMEOUT);
/// assert_eq!(t.busy_grace, BUSY_GRACE);
/// // a busy owner is given up on before the election that waits on it gives up itself
/// assert!(t.busy_grace < t.join_timeout);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// `server.bind_grace_seconds:`
    pub bind_grace: Duration,
    /// `server.probe_timeout_seconds:`
    pub probe_timeout: Duration,
    /// `server.join_timeout_seconds:`
    pub join_timeout: Duration,
    /// `server.busy_grace_seconds:`
    pub busy_grace: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            bind_grace: BIND_GRACE,
            probe_timeout: PROBE_TIMEOUT,
            join_timeout: JOIN_TIMEOUT,
            busy_grace: BUSY_GRACE,
        }
    }
}

static TIMINGS: OnceLock<Timings> = OnceLock::new();

/// What this process judges a lease contest by. The declaration when one was read, the
/// constants otherwise.
///
/// ```
/// use majordomus_cli::lease::{timings, Timings};
/// // this example's process declared nothing: it judges by the constants, and asking again
/// // answers the same, because the first answer is the one the process keeps
/// assert_eq!(timings(), Timings::default());
/// assert_eq!(timings(), timings());
/// ```
pub fn timings() -> Timings {
    *TIMINGS.get_or_init(Timings::default)
}

impl Timings {
    /// What a declaration means, as a value rather than as a side effect.
    ///
    /// The mapping is the part that can be wrong; the `OnceLock` below is plumbing. Kept
    /// separate so it can be tested directly — a test that went through the lock would
    /// depend on which test ran first, since a process reads one policy by construction.
    ///
    /// ```
    /// use majordomus_cli::lease::Timings;
    /// use majordomus_cli::policy::ServerPolicy;
    /// use std::time::Duration;
    ///
    /// // an empty declaration keeps every default
    /// assert_eq!(Timings::from_policy(&ServerPolicy::default()), Timings::default());
    ///
    /// // and a declared value is the one used, rather than the constant
    /// let declared = ServerPolicy {
    ///     probe_timeout_seconds: Some(9),
    ///     ..ServerPolicy::default()
    /// };
    /// let t = Timings::from_policy(&declared);
    /// assert_eq!(t.probe_timeout, Duration::from_secs(9));
    /// assert_ne!(t.probe_timeout, Timings::default().probe_timeout);
    /// // the keys it says nothing about are untouched
    /// assert_eq!(t.bind_grace, Timings::default().bind_grace);
    /// ```
    pub fn from_policy(policy: &crate::policy::ServerPolicy) -> Self {
        let d = Self::default();
        Self {
            bind_grace: policy
                .bind_grace_seconds
                .map(Duration::from_secs)
                .unwrap_or(d.bind_grace),
            probe_timeout: policy
                .probe_timeout_seconds
                .map(Duration::from_secs)
                .unwrap_or(d.probe_timeout),
            join_timeout: policy
                .join_timeout_seconds
                .map(Duration::from_secs)
                .unwrap_or(d.join_timeout),
            busy_grace: policy
                .busy_grace_seconds
                .map(Duration::from_secs)
                .unwrap_or(d.busy_grace),
        }
    }
}

/// Declare the timings for this process from a repository's policy. The first call decides;
/// later ones are ignored, which is what makes [`timings`] answer the same thing all the way
/// through one contest.
///
/// ```
/// use majordomus_cli::lease::{declare_timings, timings};
/// use majordomus_cli::policy::ServerPolicy;
/// use std::time::Duration;
/// // the first declaration is the one in force
/// declare_timings(&ServerPolicy { busy_grace_seconds: Some(4), ..ServerPolicy::default() });
/// assert_eq!(timings().busy_grace, Duration::from_secs(4));
/// // a later one cannot move a judgement already being made
/// declare_timings(&ServerPolicy { busy_grace_seconds: Some(9), ..ServerPolicy::default() });
/// assert_eq!(timings().busy_grace, Duration::from_secs(4));
/// ```
pub fn declare_timings(policy: &crate::policy::ServerPolicy) {
    let _ = TIMINGS.set(Timings::from_policy(policy));
}

/// The environment variable a server is started with when the process that started it had
/// already waited on the lease it found and judged it stale; its value is that lease's
/// token. The new server's election then takes exactly that lease over without spending the
/// same patience on it again. Any other lease, including one that changed in between, still
/// gets [`probe_patiently`]'s wait.
pub const JUDGED_STALE_ENV: &str = "MAJORDOMUS_LEASE_JUDGED_STALE";

/// The identity of the file an executable was started from: where it is, and the mtime
/// and size of the file at that path. A server outlives its own binary — a rebuild replaces
/// the file under a process that keeps serving the code it loaded hours ago — and nothing
/// about the process itself says so. This is what makes that visible.
///
/// ```
/// use majordomus_cli::lease::ExecutableIdentity;
/// let recorded = ExecutableIdentity { path: "/opt/majordomus".into(), mtime: 1_700_000_000, size: 42 };
/// let text = serde_json::to_string(&recorded).unwrap();
/// assert_eq!(serde_json::from_str::<ExecutableIdentity>(&text).unwrap(), recorded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutableIdentity {
    /// The path the process was started from.
    pub path: PathBuf,
    /// The file's modification time, seconds since the epoch.
    pub mtime: u64,
    /// The file's size in bytes.
    pub size: u64,
}

impl ExecutableIdentity {
    /// Has the file at the recorded path been replaced or removed since it was recorded?
    /// The reason, for a reader; `None` when the same file is still there.
    ///
    /// ```
    /// use majordomus_cli::lease::ExecutableIdentity;
    /// let gone = ExecutableIdentity { path: "/nowhere/majordomus".into(), mtime: 1, size: 1 };
    /// assert!(gone.replaced().unwrap().contains("no longer on disk"));
    /// ```
    pub fn replaced(&self) -> Option<String> {
        let Ok(meta) = fs::metadata(&self.path) else {
            return Some(format!(
                "the executable it was started from, {}, is no longer on disk",
                self.path.display()
            ));
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if mtime == self.mtime && meta.len() == self.size {
            return None;
        }
        Some(format!(
            "the executable it was started from, {}, has been replaced since (it is serving \
             code that is no longer on disk)",
            self.path.display()
        ))
    }
}

/// The lease file, typed: what the process serving a checkout wrote down about itself.
/// Every reader of the file — the election, a command asking which server is running, the
/// environment snapshot, the server's own status — reads it through [`LeaseFile::read`],
/// so that a field added here is added once and a file that is not a lease is refused in
/// one place. A key this version does not know is ignored, so a lease written by a newer
/// executable still reads; a key it needs that an older lease lacks takes its default.
///
/// ```
/// use majordomus_cli::lease::{LeaseDocument, SCHEMA};
/// // a lease an older server wrote: no pid, no version, a key this version has never heard of
/// let older = format!(r#"{{"schema":"{SCHEMA}","token":"t","root":"/r","url":"http://127.0.0.1:1","later":1}}"#);
/// let doc: LeaseDocument = serde_json::from_str(&older).unwrap();
/// assert_eq!(doc.pid, 0);
/// assert!(doc.version.is_none(), "older than any executable that reads this");
/// assert_eq!(doc.url.as_deref(), Some("http://127.0.0.1:1"));
/// // and what this version writes reads back as itself
/// let text = serde_json::to_string(&doc).unwrap();
/// assert_eq!(serde_json::from_str::<LeaseDocument>(&text).unwrap(), doc);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaseDocument {
    /// [`SCHEMA`].
    pub schema: String,
    /// The process id of the server. A live pid is not a live server: only an answering
    /// probe says that. What a live pid does decide is patience — a probe that goes
    /// unanswered by a live owner is asked again within [`BUSY_GRACE`] before the lease
    /// counts as stale (the election's busy wait, [`probe_patiently`]); a dead or zero pid
    /// gets no patience at all.
    #[serde(default)]
    pub pid: u32,
    /// What makes the file this process's: only the process holding this token removes it.
    #[serde(default)]
    pub token: String,
    /// The checkout root the server serves.
    #[serde(default)]
    pub root: PathBuf,
    /// The address, once bound; `null` while the owner is still binding.
    pub url: Option<String>,
    /// When the server took the lease, RFC 3339.
    #[serde(default)]
    pub started_at: String,
    /// The executable the server was started from, when it could be located.
    #[serde(default)]
    pub executable: Option<ExecutableIdentity>,
    /// The executable's version. Absent in a lease written before this field existed,
    /// which is itself a fact: that server is older than any executable that reads this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// What a lease file holds, read once. Every state of the file is a variant, so that a
/// reader decides what to do about each rather than learning it from an error.
///
/// ```
/// use majordomus_cli::lease::LeaseFile;
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("server.json");
/// assert_eq!(LeaseFile::read(&path), LeaseFile::Absent);
/// assert!(LeaseFile::read(&path).document().is_none(), "no document to hand back");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseFile {
    /// There is no file.
    Absent,
    /// A file with nothing in it: its owner created it and has not written it yet.
    Empty,
    /// A file that is not a lease document; the reason, for the log.
    Corrupt(String),
    /// A lease document.
    Document(LeaseDocument),
}

impl LeaseFile {
    /// Read and type the file at `path`. Never fails: every state of the file is one of the
    /// variants, and what to do about each is the reader's decision.
    ///
    /// ```
    /// use majordomus_cli::lease::{LeaseFile, SCHEMA};
    /// let dir = tempfile::tempdir().unwrap();
    /// let path = dir.path().join("server.json");
    /// assert_eq!(LeaseFile::read(&path), LeaseFile::Absent);
    /// std::fs::write(&path, "").unwrap();
    /// assert_eq!(LeaseFile::read(&path), LeaseFile::Empty);
    /// std::fs::write(&path, "{not json").unwrap();
    /// assert!(matches!(LeaseFile::read(&path), LeaseFile::Corrupt(r) if r.starts_with("not JSON")));
    /// std::fs::write(&path, r#"{"schema":"other/v1"}"#).unwrap();
    /// assert!(matches!(LeaseFile::read(&path), LeaseFile::Corrupt(r) if r.contains(SCHEMA)));
    /// std::fs::write(&path, format!(r#"{{"schema":"{SCHEMA}","token":"x","root":"/r","url":null}}"#)).unwrap();
    /// match LeaseFile::read(&path) {
    ///     LeaseFile::Document(d) => { assert_eq!(d.token, "x"); assert!(d.url.is_none()); assert!(d.version.is_none()); }
    ///     other => panic!("{other:?}"),
    /// }
    /// ```
    pub fn read(path: &Path) -> LeaseFile {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LeaseFile::Absent,
            Err(e) => return LeaseFile::Corrupt(format!("cannot be read ({e})")),
        };
        if text.trim().is_empty() {
            return LeaseFile::Empty;
        }
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => return LeaseFile::Corrupt(format!("not JSON ({e})")),
        };
        if !value.is_object() || value["schema"] != SCHEMA {
            return LeaseFile::Corrupt(format!("not a {SCHEMA} document"));
        }
        match serde_json::from_value::<LeaseDocument>(value) {
            Ok(document) => LeaseFile::Document(document),
            Err(e) => LeaseFile::Corrupt(format!("not a {SCHEMA} document ({e})")),
        }
    }

    /// The document, when the file holds one; `None` for an absent, empty or corrupt file,
    /// for a reader that has no different answer for those three.
    ///
    /// ```
    /// use majordomus_cli::lease::{LeaseDocument, LeaseFile, SCHEMA};
    /// let doc = LeaseDocument {
    ///     schema: SCHEMA.into(), pid: 7, token: "t".into(), root: "/r".into(), url: None,
    ///     started_at: "2026-09-10T00:00:00Z".into(), executable: None, version: None,
    /// };
    /// assert_eq!(LeaseFile::Document(doc.clone()).document(), Some(&doc));
    /// assert_eq!(LeaseFile::Corrupt("not JSON".into()).document(), None);
    /// ```
    pub fn document(&self) -> Option<&LeaseDocument> {
        match self {
            LeaseFile::Document(d) => Some(d),
            _ => None,
        }
    }
}

/// The lease this process holds and has published, when it is a server. Set by
/// [`Lease::publish`], cleared when the lease is released, so that the server can say what
/// it wrote down about itself without reading its own file back.
static PUBLISHED: OnceLock<Mutex<Option<LeaseDocument>>> = OnceLock::new();

fn published() -> &'static Mutex<Option<LeaseDocument>> {
    PUBLISHED.get_or_init(|| Mutex::new(None))
}

/// The lease this process holds and has published: `None` in a process that serves nothing.
///
/// ```
/// // a process that has published no lease holds none
/// assert!(majordomus_cli::lease::held().is_none());
/// ```
pub fn held() -> Option<LeaseDocument> {
    published().lock().ok()?.clone()
}

/// Is `doc` the lease this very process holds, published and not lost?
///
/// A server asked where its own checkout stands must answer from memory. Probing the
/// address its lease names is a request to itself, served by the same small pool of HTTP
/// workers the question arrived on: with every worker answering such a question at once,
/// none is left to answer the probes, each waits out [`PROBE_TIMEOUT`], the server calls
/// itself stale — and a client electing in that window times out on the same queue and
/// takes a live lease over. Measured on 2026-09-15: ten MCP clients calling
/// `majordomus_peers` at once, and one of them started a second server of the checkout.
///
/// ```
/// use majordomus_cli::lease::{is_own, LeaseDocument};
/// let doc: LeaseDocument = serde_json::from_str(
///     r#"{"schema":"lease/v1","pid":1,"token":"t","url":"http://127.0.0.1:1"}"#,
/// )
/// .unwrap();
/// // a process that holds no lease owns no document
/// assert!(!is_own(&doc));
/// ```
pub fn is_own(doc: &LeaseDocument) -> bool {
    !was_lost()
        && held().is_some_and(|h| h.token == doc.token && h.pid == doc.pid && h.url == doc.url)
}

/// The lease this process holds. Dropping it removes the file (when the file is still
/// this process's), so a failed start never leaves a stale lease behind.
#[derive(Debug)]
pub struct Lease {
    path: PathBuf,
    token: String,
    root: PathBuf,
    released: bool,
    /// When this process took the lease, RFC 3339. Fixed here rather than at publish time
    /// because it is the identity of this server's generation.
    started_at: String,
    /// `true` from the election until [`Lease::publish`]: the window in which
    /// [`Lease::keep_alive`] rewrites the file so that it never looks abandoned, and the
    /// lock under which it and `publish` write, so that a touch can never land on top of
    /// the URL.
    binding: Arc<Mutex<bool>>,
}

/// What the election decided for this process.
#[derive(Debug)]
pub enum Role {
    /// Nobody serves this repository: this process does, holding the lease.
    Server(Lease),
    /// A server answers at this URL: attach to it.
    Peer {
        /// `http://host:port` of the running server.
        url: String,
    },
}

/// Where the lease of a repository lives.
pub fn lease_path(repo: &Repository) -> PathBuf {
    lease_file(repo.root(), &repo.local_path())
}

/// Where the lease of the checkout at `root` lives, given the repository-relative path of
/// its local half (`.ai/local`). The one composition of that path; every reader of a lease
/// that is not this process's own repository — another checkout's, in the server's status
/// or the environment snapshot — goes through here.
///
/// ```
/// use majordomus_cli::lease::lease_file;
/// use std::path::Path;
/// assert_eq!(
///     lease_file(Path::new("/r"), ".ai/local"),
///     Path::new("/r/.ai/local/state/mcp/server.json")
/// );
/// ```
pub fn lease_file(root: &Path, local_half: &str) -> PathBuf {
    root.join(local_half).join(LEASE_PATH)
}

/// Which server is serving this repository right now, if any.
///
/// Read-only, unlike [`elect`]: it takes no lease, creates no file and waits for nobody, so
/// a command that only wants to *ask* the running server something cannot accidentally
/// become it. `None` means no lease, no URL in it, or a lease whose server does not answer
/// for this root.
pub fn serving(repo: &Repository) -> Option<String> {
    let url = LeaseFile::read(&lease_path(repo)).document()?.url.clone()?;
    probe(&url, repo.root()).then_some(url)
}

/// Decide whether this process serves the repository or attaches to the process that does.
///
/// An existing file is read on every attempt and classified: the lease of a live server
/// (it answers for this root) is attached to; a stale one (its server does not answer),
/// a corrupt one (not a lease document), an empty one, or an abandoned one (no URL after
/// [`BIND_GRACE`]) is taken over; a fresh lease without a URL means its owner is still
/// binding, and this process waits for it. Nothing a client leaves behind can lock the
/// others out; when the file can neither be created nor removed within [`JOIN_TIMEOUT`],
/// the error names the path and the caller serves its client alone.
pub fn elect(repo: &Repository) -> Result<Role> {
    let path = lease_path(repo);
    let root = repo.root().to_path_buf();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let waited_since = Instant::now();
    // when a live server first failed to answer this election; cleared whenever it answers,
    // or the file stops naming a busy server
    let mut busy_since: Option<Instant> = None;
    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let token = format!(
                    "{}-{:x}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                );
                let lease = Lease {
                    path: path.clone(),
                    token,
                    root,
                    released: false,
                    started_at: crate::peers::rfc3339(SystemTime::now()),
                    binding: Arc::new(Mutex::new(true)),
                };
                let text =
                    serde_json::to_string(&lease.document(None)).map_err(|e| Error::Lease {
                        reason: format!("cannot render the lease: {e}"),
                    })?;
                file.write_all(text.as_bytes())
                    .map_err(|e| Error::io(&path, e))?;
                hold(&path, &lease.token);
                return Ok(Role::Server(lease));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let (found, seen) = inspect(&path, &root);
                if !matches!(found, Found::Busy(_)) {
                    busy_since = None;
                }
                match found {
                    Found::Live(url) => return Ok(Role::Peer { url }),
                    Found::Busy(url) => {
                        let since = *busy_since.get_or_insert_with(Instant::now);
                        if since.elapsed() < timings().busy_grace {
                            std::thread::sleep(Duration::from_millis(200));
                        } else {
                            tracing::warn!(
                                lease = %path.display(),
                                "the server at {url} is alive and has not answered for {} seconds; taking it over",
                                timings().busy_grace.as_secs()
                            );
                            take_over(&path, &seen)?;
                            busy_since = None;
                        }
                    }
                    Found::Stale(reason) => {
                        tracing::warn!(lease = %path.display(), "{reason}; taking it over");
                        take_over(&path, &seen)?;
                    }
                    // an owner that is still binding keeps its file young (`keep_alive`), so
                    // a file that is old enough to be abandoned is classified so by `inspect`
                    // and this arm only ever waits; the whole attempt is bounded below
                    Found::Binding => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            Err(e) => return Err(Error::io(&path, e)),
        }
        if waited_since.elapsed() > timings().join_timeout {
            return Err(Error::Lease {
                reason: format!(
                    "could not acquire or join the lease at {} within {} seconds",
                    path.display(),
                    timings().join_timeout.as_secs()
                ),
            });
        }
    }
}

/// What a lease file that already exists says.
enum Found {
    /// A server answers at this URL for this root.
    Live(String),
    /// The server at this URL did not answer in time, and the process that holds the lease
    /// is alive: busy rather than gone, and waited on for [`BUSY_GRACE`].
    Busy(String),
    /// The file is not a usable lease; the reason says why, for the log.
    Stale(String),
    /// A lease without a URL, young enough that its owner may still be binding.
    Binding,
}

/// What the running executable is, for the lease to record: the path it was started from
/// and the identity of the file at that path. A server outlives its own binary — a rebuild
/// replaces the file under a process that keeps serving the code it loaded hours ago — and
/// nothing about the process itself says so. This is what makes that visible.
///
/// `None` when the executable cannot be located or stat'ed; the lease then carries no
/// claim, and the election's `superseded` check makes none either.
///
/// ```
/// use majordomus_cli::lease::executable_identity;
/// // the process running this example can locate itself, and the file it names is there
/// let mine = executable_identity().expect("a test binary knows where it is");
/// assert!(mine.path.is_file());
/// assert!(mine.replaced().is_none(), "the file that is running has not been replaced");
/// ```
pub fn executable_identity() -> Option<ExecutableIdentity> {
    let path = std::env::current_exe().ok()?;
    let meta = fs::metadata(&path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(ExecutableIdentity {
        path,
        mtime,
        size: meta.len(),
    })
}

/// The identity [`started_as`] pins: set once, by the first caller, and never again.
static STARTED_AS: OnceLock<Option<ExecutableIdentity>> = OnceLock::new();

/// The executable this process was started from, as it stood on disk when first asked.
///
/// `main` asks before anything else, so the answer is the file the process was loaded from,
/// not whatever sits at that path later. A server that has been running for a day still
/// knows which bytes it is: the comparison [`serving_replaced_code`] makes is against this,
/// never against a second executable a client would have to hold (I1502).
///
/// ```
/// use majordomus_cli::lease::started_as;
/// let first = started_as().cloned();
/// assert_eq!(started_as().cloned(), first, "pinned: the second answer is the first");
/// ```
pub fn started_as() -> Option<&'static ExecutableIdentity> {
    STARTED_AS.get_or_init(executable_identity).as_ref()
}

/// Why this process is serving code that is no longer on disk, or `None` while the file it
/// was started from is still the file at that path. The same judgement the lease reader
/// makes from outside ([`ExecutableIdentity::replaced`]), made by the process about itself,
/// so that the endpoint a client asks for readiness can say so.
///
/// ```
/// use majordomus_cli::lease::serving_replaced_code;
/// assert!(serving_replaced_code().is_none(), "a test binary has not been replaced under it");
/// ```
pub fn serving_replaced_code() -> Option<String> {
    started_as().and_then(stale_reason)
}

/// The judgement [`serving_replaced_code`] makes, over an identity it is handed rather than
/// the one this process pinned, so that each of its answers — unchanged, replaced, removed —
/// can be shown against a file a test controls.
///
/// The reason is served to whoever can reach the socket, so it says what happened and not
/// where the file is: the host's paths are of no use to a client and of some use to others.
fn stale_reason(me: &ExecutableIdentity) -> Option<String> {
    me.replaced()?;
    Some(if me.path.exists() {
        "the executable this process was started from has been replaced since it started: \
         it is serving code that is no longer on disk"
            .to_string()
    } else {
        "the executable this process was started from has been removed since it started: \
         it is serving code that is no longer on disk"
            .to_string()
    })
}

/// Has the executable behind a lease been replaced since that server started?
///
/// Only a lease naming *this process's own* executable path can answer: same path, a file
/// that is now a different file, and the server on the other end is provably running code
/// that no longer exists on disk. A lease naming some other path — a release install next
/// to a debug build — makes no claim either way and is left alone, so two legitimate
/// binaries never fight over the lease.
fn superseded(doc: &LeaseDocument) -> Option<String> {
    let recorded = doc.executable.as_ref()?;
    let mine = executable_identity()?;
    if recorded.path != mine.path {
        return None;
    }
    if recorded.mtime == mine.mtime && recorded.size == mine.size {
        return None;
    }
    Some(format!(
        "superseded lease: the server was started from {} and that file has been replaced since \
         (it is serving code that is no longer on disk)",
        mine.path.display()
    ))
}

/// Read and classify an existing lease file. What was read comes back beside the verdict,
/// so that a take-over can insist on removing the file it judged and not one that arrived
/// in the meantime.
fn inspect(path: &Path, root: &Path) -> (Found, LeaseFile) {
    let age = file_age(path);
    let seen = LeaseFile::read(path);
    let doc = match &seen {
        // gone between the failed create and this read: the next attempt creates it
        LeaseFile::Absent => return (Found::Binding, seen),
        LeaseFile::Empty if age > timings().bind_grace => {
            return (
                Found::Stale("empty lease: its owner never wrote it".into()),
                seen,
            )
        }
        LeaseFile::Empty => return (Found::Binding, seen),
        LeaseFile::Corrupt(reason) => {
            return (Found::Stale(format!("corrupt lease: {reason}")), seen)
        }
        LeaseFile::Document(doc) => doc.clone(),
    };
    // before asking whether it answers: a server that answers from a binary that has been
    // replaced answers with yesterday's code, which is the harder failure to see
    if let Some(reason) = superseded(&doc) {
        return (Found::Stale(reason), seen);
    }
    let found = match doc.url.as_deref() {
        Some(url) => match ask(url, root, timings().probe_timeout) {
            Answer::Ours(_) => Found::Live(url.to_string()),
            // a live owner that is silent is busy and waited on — unless the process that
            // started this one already waited out that patience on this very lease
            Answer::Silent if alive(doc.pid) && !judged_stale(&doc) => Found::Busy(url.to_string()),
            Answer::Silent | Answer::NotOurs => Found::Stale(format!(
                "stale lease: the server it names at {url} does not answer for this repository"
            )),
        },
        None if age > timings().bind_grace => {
            Found::Stale("abandoned lease: its owner never published a URL".into())
        }
        None => Found::Binding,
    };
    (found, seen)
}

/// How long ago the file was last written; zero when that cannot be read, so that a file
/// whose age is unknown is never taken for an old one.
///
/// ```
/// use majordomus_cli::lease::file_age;
/// use std::time::Duration;
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("server.json");
/// assert_eq!(file_age(&path), Duration::ZERO, "no file: no age");
/// std::fs::write(&path, "").unwrap();
/// assert!(file_age(&path) < Duration::from_secs(60), "just written");
/// ```
pub fn file_age(path: &Path) -> Duration {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| m.elapsed().ok())
        .unwrap_or(Duration::ZERO)
}

/// Remove a lease that cannot be used, so that the next attempt creates a fresh one — but
/// only the file that was judged: when another process has replaced it since, the file on
/// disk is somebody's fresh lease, and the next round of the election reads that one.
fn take_over(path: &Path, seen: &LeaseFile) -> Result<()> {
    take_over_with(path, seen, LOCK_PATIENCE, || {})
}

/// [`take_over`], with the patience for the [change lock](lock_for_change) stated and a seam
/// between the comparison and the removal, where the unit tests put a second elector.
///
/// # Why a lock, and not a rename aside
///
/// Until I2128 this was three steps — read, compare with what was judged, remove — and two
/// electors that had both judged one lease stale could interleave: A removes the stale lease
/// and creates its own, and B, which had already compared, removes A's. A then serves with no
/// lease on disk, and the next client starts a second server for the checkout.
///
/// The textbook repair for a compare-and-delete on a file is to rename it aside under a name
/// of one's own and look at what was moved: the rename is atomic, so whatever was moved is
/// the mover's to judge. It was considered and not taken, because it cannot be used for the
/// other half of the problem. A lease is also *rewritten* — published, kept young while the
/// layer loads — and a rewrite that moved the file aside to check it would leave the path
/// empty for a moment on every rewrite, during which any elector's `create_new` succeeds:
/// the owner would lose its lease to its own bookkeeping. A moved-aside lease that turned out
/// to be somebody else's has the same hole on the way back.
///
/// So every change to the file — this removal, [`Lease::publish`], the keep-alive rewrite,
/// the release — is made under one advisory lock beside the lease (`server.lock`), and each
/// re-reads the file under it before acting. Creating the lease needs no lock: `create_new`
/// only succeeds on an empty path, and a path holding a lease can only be emptied under the
/// lock, by a holder that has just read it. Two electors that judged one lease stale then end
/// with exactly one lease: whichever takes the lock first removes the stale file, the other
/// finds a file that is not the one it judged and leaves it, and `create_new` lets exactly
/// one of them create the next.
///
/// What it does not cover is an executable older than this one, which takes no lock; two
/// builds side by side keep the old window between them until the older one is gone.
fn take_over_with(
    path: &Path,
    seen: &LeaseFile,
    patience: Duration,
    between: impl FnOnce(),
) -> Result<()> {
    let Some(_lock) = lock_for_change(path, patience) else {
        tracing::debug!(lease = %path.display(), "another process is changing the lease; reading it again");
        return Ok(());
    };
    if LeaseFile::read(path) != *seen {
        tracing::debug!(lease = %path.display(), "the lease changed under the take-over; reading it again");
        return Ok(());
    }
    between();
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Lease {
            reason: format!(
                "cannot remove the unusable lease at {}: {e}",
                path.display()
            ),
        }),
    }
}

/// How long a change to the lease waits for another process's change to finish. A change
/// holds the lock for one small read and one rename or removal, so anything near this bound
/// is a holder that has been stopped (`SIGSTOP`, a debugger), and the change is then not
/// made: a lease left in place is recovered by the next election, a lease removed or
/// overwritten in error is not.
const LOCK_PATIENCE: Duration = Duration::from_secs(2);

/// The advisory lock every change to a lease file is made under: an exclusive `flock(2)` on
/// `server.lock` beside it, held for as long as the value lives. The kernel releases it when
/// the descriptor closes, so a holder that is killed cannot keep it.
///
/// Where the lock file cannot be opened at all (a directory that refuses writes) or the
/// filesystem refuses `flock`, the change goes ahead without it: the check that the file is
/// the actor's own is still made, as it was before the lock existed.
struct ChangeLock {
    #[cfg(unix)]
    _file: Option<fs::File>,
}

/// The lock file beside a lease.
fn lock_path(lease: &Path) -> PathBuf {
    lease.with_extension("lock")
}

/// Take the [`ChangeLock`] of the lease at `lease`, waiting up to `patience` for another
/// holder; `None` when it is still held after that. A `patience` of zero is one attempt.
fn lock_for_change(lease: &Path, patience: Duration) -> Option<ChangeLock> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let Ok(file) = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path(lease))
        else {
            return Some(ChangeLock { _file: None });
        };
        let deadline = Instant::now() + patience;
        loop {
            // SAFETY: flock(2) on a descriptor this function owns; no memory is touched.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Some(ChangeLock { _file: Some(file) });
            }
            let error = std::io::Error::last_os_error();
            let contended = error.raw_os_error() == Some(libc::EWOULDBLOCK)
                || error.kind() == std::io::ErrorKind::Interrupted;
            if !contended {
                return Some(ChangeLock { _file: None });
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (lease, patience);
        Some(ChangeLock {})
    }
}

/// Does the file at `path` hold the lease named by `token`? The token is minted once per
/// election, from the pid and the clock, so it names one process's one generation of the
/// lease: a process that took the lease, lost it and took it again holds a different token.
fn holds(path: &Path, token: &str) -> bool {
    LeaseFile::read(path)
        .document()
        .is_some_and(|d| d.token == token)
}

/// Remove the lease at `path` if, and only if, it is still the one `token` names — under the
/// [`ChangeLock`], so that nobody can replace the file between the check and the removal.
/// Whether it was removed.
fn remove_if_held(path: &Path, token: &str) -> bool {
    let Some(_lock) = lock_for_change(path, LOCK_PATIENCE) else {
        tracing::warn!(lease = %path.display(), "the lease's lock stayed held; leaving the lease for the next election to judge");
        return false;
    };
    holds(path, token) && fs::remove_file(path).is_ok()
}

/// A temporary file of one writer's own beside the lease: named for the token, so that two
/// writers never write into, or rename, each other's half-written file.
fn tmp_path(path: &Path, token: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "server.json".into());
    path.with_file_name(format!("{name}.{token}.tmp"))
}

/// Write `text` to `tmp` and sync it to the disk, so that the rename that follows publishes
/// a whole file even across a crash of the machine.
fn write_synced(tmp: &Path, text: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// Sync the directory a rename happened in, so that the rename itself is durable. Best
/// effort: a platform that cannot open a directory for syncing loses only the durability.
fn sync_dir(path: &Path) {
    if let Some(dir) = path.parent() {
        if let Ok(dir) = fs::File::open(dir) {
            let _ = dir.sync_all();
        }
    }
}

/// The lease this process holds, by path and token, from the election until it is released
/// or lost: what [`release_held`] removes when the process is asked to stop by a signal and
/// the [`Lease`] itself belongs to a thread that will not get to drop it.
static HOLDING: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);

/// This process now holds the lease at `path` under `token`; install the signal handlers the
/// first time.
fn hold(path: &Path, token: &str) {
    if let Ok(mut holding) = HOLDING.lock() {
        *holding = Some((path.to_path_buf(), token.to_string()));
    }
    signals::install();
}

/// This process no longer holds the lease `token` names (or, with `None`, any lease).
fn forget(token: Option<&str>) {
    if let Ok(mut holding) = HOLDING.lock() {
        if token.is_none_or(|t| holding.as_ref().is_some_and(|(_, h)| h == t)) {
            *holding = None;
        }
    }
}

/// Release the lease this process holds, if the file is still its own: the release a
/// process makes when it is stopped by a signal, from a thread that does not own the
/// [`Lease`]. A lease taken over since is somebody else's and stays where it is.
pub(crate) fn release_held() {
    let taken = HOLDING.lock().ok().and_then(|mut h| h.take());
    release_record(taken);
}

/// [`release_held`] over a record it is handed rather than the process-wide one, so that a
/// test can show it without touching what a concurrent test's election recorded.
fn release_record(record: Option<(PathBuf, String)>) {
    if let Some((path, token)) = record {
        remove_if_held(&path, &token);
        if let Ok(mut slot) = published().lock() {
            if slot.as_ref().is_some_and(|d| d.token == token) {
                *slot = None;
            }
        }
    }
}

/// How long a server asked to stop by a signal is given to stop in order before the process
/// is ended anyway. It sits below `serve stop`'s default wait of ten seconds, so that a stop
/// that hangs — a mesh link that will not drain — still ends within what `serve stop` waits.
pub(crate) const STOP_BOUND: Duration = Duration::from_secs(8);

/// The signal that asked this process to stop, once one has: `SIGTERM` from `serve stop`,
/// `SIGINT` from Ctrl-C, `SIGHUP` from a closing terminal. A loop that stops the server in
/// order polls this.
pub(crate) fn stop_requested() -> Option<i32> {
    signals::requested()
}

/// Declare that this process answers a stop request in order: the first signal is then left
/// to it for [`STOP_BOUND`], instead of ending the process at once.
pub(crate) fn answer_stop_requests() {
    signals::answer();
}

/// End the process the way the signal would have: release the lease if it is still this
/// process's, restore the signal's default disposition and send it again, so that the exit
/// status still names the signal.
pub(crate) fn end_by_signal(signal: i32) -> ! {
    release_held();
    #[cfg(unix)]
    // SAFETY: restoring a default disposition and signalling this process; no memory is
    // touched.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::kill(libc::getpid(), signal);
    }
    // the default disposition of every signal the handlers take ends the process; this is
    // the moment between the sending and the delivery
    std::thread::sleep(Duration::from_secs(1));
    std::process::exit(128 + signal)
}

/// What the signal watcher does about the signals received so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watch {
    /// Nothing to do yet.
    Wait,
    /// End the process now.
    End,
}

/// The watcher's decision, as a value: no signal is nothing to do; the first, in a process
/// that answers stop requests, is left to it for [`STOP_BOUND`]; a first one nobody answers,
/// a second one, or the bound passing ends the process.
fn watch_verdict(received: usize, answered: bool, since_first: Duration) -> Watch {
    match received {
        0 => Watch::Wait,
        1 if answered && since_first < STOP_BOUND => Watch::Wait,
        _ => Watch::End,
    }
}

/// Set by [`lost`], never cleared: a lease this process was given and no longer has.
///
/// Distinct from `held().is_none()`, which is also true of a process that never took a
/// lease at all — a test, a one-off command, a bridged client. Only a process that *had*
/// the lease and lost it must change how it behaves, so only that one is recorded here.
static LOST: AtomicBool = AtomicBool::new(false);

/// The key the index answers [`was_lost`]'s converse under: whether the server answering
/// still holds the lease of the checkout it serves. Named once, read by [`probe`] and
/// written by the index route, so the two cannot drift apart.
pub const LEASEHOLDER_KEY: &str = "leaseholder";

/// This process's lease was taken over by another process: from now on nothing this process
/// does on its way out touches the file, which is somebody else's, and it stops claiming to be the
/// checkout's server — [`held`] answers `None` and [`was_lost`] answers `true` from here
/// on. It serves the peers it has and ends with them; what it must not do is take on new
/// ones, or answer a stranger's probe as though it were still the one.
///
/// ```
/// // a process that holds no lease has nothing to lose; saying so twice changes nothing
/// majordomus_cli::lease::lost();
/// majordomus_cli::lease::lost();
/// assert!(majordomus_cli::lease::held().is_none());
/// assert!(majordomus_cli::lease::was_lost(), "it was told the lease is gone");
/// ```
pub fn lost() {
    LOST.store(true, Ordering::SeqCst);
    if let Ok(mut published) = published().lock() {
        *published = None;
    }
    forget(None);
}

/// Did this process hold the checkout's lease and lose it?
///
/// The one question a server asks before taking on work that outlives the request. A
/// process that never held a lease answers `false` and goes on serving whoever asked it —
/// a test, a bridged client, a one-off command. One that was superseded answers `true`,
/// and from then on refuses to become anybody's server again. It never resets: a lease
/// that was taken over is not given back.
///
/// ```
/// // nothing has taken a lease away from the process running this example; `lost()` in
/// // the doctest above runs in a process of its own
/// assert!(!majordomus_cli::lease::was_lost());
/// ```
pub fn was_lost() -> bool {
    LOST.load(Ordering::SeqCst)
}

/// Does a Majordomus server answer at `url` for the repository at `root`, *as* that
/// repository's server?
///
/// Three questions, and the third is the one a remembered address needs: it is a
/// Majordomus server, it serves this root, and it still holds this checkout's lease.
/// Without the third, a process whose lease was taken over answers every probe exactly as
/// the current server does — same name, same `repository_id`, its own peer board, its own
/// generation of the layer — and a client holding an address from before the takeover is
/// served plausible answers by a server nobody else is talking to. That was measured on
/// 2026-09-10: two servers of one checkout, one lease, and the older one indistinguishable
/// from the current one on this endpoint.
///
/// A server too old to answer the question is accepted. It cannot be told from a current
/// one here, and refusing it would be the worse failure: a live server taken for dead is
/// taken over, which is how one checkout comes to have two.
pub fn probe(url: &str, root: &Path) -> bool {
    matches!(ask(url, root, timings().probe_timeout), Answer::Ours(_))
}

/// [`probe`], keeping what the server answered: its index document when it answers as the
/// leaseholder of this checkout, `None` otherwise. One decision, so a caller that also wants
/// the surfaces the server lists makes neither a second request nor a second judgement.
///
/// ```
/// use majordomus_cli::lease::probe_reply;
/// use std::time::Duration;
/// // nothing listens on port 1 of the loopback address
/// let root = std::path::Path::new("/r");
/// assert!(probe_reply("http://127.0.0.1:1", root, Duration::from_millis(50)).is_none());
/// ```
pub fn probe_reply(url: &str, root: &Path, timeout: Duration) -> Option<Value> {
    match ask(url, root, timeout) {
        Answer::Ours(index) => Some(index),
        Answer::NotOurs | Answer::Silent => None,
    }
}

/// What a probe of a published URL heard.
enum Answer {
    /// A Majordomus server that serves this root and holds its lease, with the index
    /// document it answered.
    Ours(Value),
    /// Something answered that is not this checkout's server, or nothing is listening.
    NotOurs,
    /// The connection was made and no answer came within the timeout: a server that is
    /// busy looks exactly like this, and so does a hung one.
    Silent,
}

/// Ask a published URL the probe's three questions within `timeout`, telling a server that
/// said no apart from one that said nothing in time.
fn ask(url: &str, root: &Path, timeout: Duration) -> Answer {
    match bridge::request(url, "GET", "/", &[], None, timeout) {
        Ok(reply) if reply.status == 200 => {
            let v: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
            // the identity and not the path: the index names the repository it serves
            // without telling every caller where the checkout sits
            let ours = v["name"] == "majordomus"
                && v["repository_id"].as_str() == Some(crate::repository::identity(root).as_str())
                && v[LEASEHOLDER_KEY] != Value::Bool(false);
            if ours {
                Answer::Ours(v)
            } else {
                Answer::NotOurs
            }
        }
        Ok(_) => Answer::NotOurs,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            Answer::Silent
        }
        Err(_) => Answer::NotOurs,
    }
}

/// Is the process with this id alive on this machine? A lease is a file of this checkout,
/// so its pid is a pid of this host. `kill(pid, 0)` delivers nothing and asks only whether
/// the process exists; a process owned by somebody else answers `EPERM`, which is alive.
/// Whether a process with this pid exists (signal 0: checked, nothing sent).
pub(crate) fn alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 performs the existence and permission check and sends nothing.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// [`probe`], with patience for an owner that is alive but slow.
///
/// A probe that times out says nothing about whether the server is gone: a server loading
/// a large layer, or one on a machine under load (an instrumented test run, a loaded CI
/// runner), answers late. Judging it gone from one silent probe makes `serve ensure` start
/// a second server beside it and report the old one's address as though it had started it.
/// So when the probe fails and the lease's `pid` is a live process on this machine, the
/// probe is repeated every second for up to `patience` — the caller's share of
/// [`Timings::busy_grace`], the same wait the election gives a busy owner. The answer is
/// `false` only when every attempt fails.
///
/// The patience is bounded on purpose. A live pid can be a wedged server that never answers
/// again, and a probe that waited on it forever would make every client hang on a ghost. A
/// dead pid, or pid 0, gets no patience at all, so a killed server is recovered at once,
/// and a `patience` of zero is a single probe.
///
/// ```
/// use majordomus_cli::lease::probe_patiently;
/// use std::path::Path;
/// use std::time::{Duration, Instant};
/// // nothing listens on port 1 and pid 0 names no process: one refused probe, no patience
/// let t0 = Instant::now();
/// let patience = Duration::from_secs(10);
/// assert!(!probe_patiently("http://127.0.0.1:1", Path::new("/nowhere"), 0, patience));
/// assert!(t0.elapsed() < Duration::from_secs(5));
/// ```
pub fn probe_patiently(url: &str, root: &Path, pid: u32, patience: Duration) -> bool {
    if probe(url, root) {
        return true;
    }
    let deadline = Instant::now() + patience;
    while alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(1));
        if probe(url, root) {
            return true;
        }
    }
    false
}

/// Was this lease already waited on, and judged stale, by the process that started this
/// one? Its token is then in [`JUDGED_STALE_ENV`], and the election takes exactly that
/// lease over at once instead of spending the same patience on it a second time: waiting
/// twice would only double the delay before a wedged owner is replaced. Any other lease —
/// a new owner, or one that changed in between — still gets the full wait.
fn judged_stale(doc: &LeaseDocument) -> bool {
    !doc.token.is_empty() && std::env::var(JUDGED_STALE_ENV).is_ok_and(|token| token == doc.token)
}

impl Lease {
    /// The file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The repository root the lease is for.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// When this process took the lease, RFC 3339: the identity of this server's
    /// generation, the same value the lease file carries as `started_at`.
    pub fn started_at(&self) -> &str {
        &self.started_at
    }

    /// What makes the file this process's. For the server's own reader, which checks from
    /// another thread whether the lease is still its own.
    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    /// Keep the lease young while the layer loads. A peer that finds a lease without a URL
    /// waits for it only as long as the file is younger than [`BIND_GRACE`]; a cold start
    /// that takes longer than that — a large layer, a machine under load — would otherwise
    /// be taken for an abandoned one and taken over while its owner is still binding. So
    /// until [`Lease::publish`], a thread rewrites the same document every third of the
    /// grace, under the lock `publish` also takes, so that a touch can never land after the
    /// URL. It writes only while the file is still this process's: a lease that was taken
    /// over anyway is somebody else's to write.
    ///
    /// ```
    /// use majordomus_cli::lease::{elect, held, LeaseFile, Role};
    /// use majordomus_cli::Repository;
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
    /// std::fs::write(dir.path().join(".ai/manifest.yaml"),
    ///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
    /// let repo = Repository::discover(dir.path()).unwrap();
    /// let Role::Server(lease) = elect(&repo).unwrap() else { panic!("nobody else serves a fresh directory") };
    /// lease.keep_alive();                         // returns at once; the touching runs beside the load
    /// assert!(held().is_none(), "nothing is published yet");
    /// lease.publish("http://127.0.0.1:1").unwrap();  // stops the touching, under the same lock
    /// assert_eq!(held().unwrap().url.as_deref(), Some("http://127.0.0.1:1"));
    /// assert!(matches!(LeaseFile::read(lease.path()), LeaseFile::Document(d) if d.url.is_some()));
    /// lease.release();
    /// assert_eq!(LeaseFile::read(&dir.path().join(".ai/local/state/mcp/server.json")), LeaseFile::Absent);
    /// ```
    pub fn keep_alive(&self) {
        let path = self.path.clone();
        let token = self.token.clone();
        let binding = Arc::clone(&self.binding);
        let document = serde_json::to_string(&self.document(None)).unwrap_or_default();
        let tick = timings().bind_grace / 3;
        let _ = std::thread::Builder::new()
            .name("majordomus-lease-keep-alive".into())
            .spawn(move || loop {
                std::thread::sleep(tick);
                let Ok(guard) = binding.lock() else { return };
                if !*guard {
                    return;
                }
                if !holds(&path, &token) {
                    return;
                }
                // the same discipline as `publish`: a file of its own, synced, renamed only
                // while the lease is still this process's, under the lock a take-over takes;
                // a lock held by somebody else skips this touch rather than wait on it
                let tmp = tmp_path(&path, &token);
                if write_synced(&tmp, &document).is_err() {
                    let _ = fs::remove_file(&tmp);
                    continue;
                }
                let renamed = lock_for_change(&path, Duration::from_millis(200))
                    .filter(|_| holds(&path, &token))
                    .is_some_and(|_lock| fs::rename(&tmp, &path).is_ok());
                if !renamed {
                    let _ = fs::remove_file(&tmp);
                }
            });
    }

    fn document(&self, url: Option<&str>) -> LeaseDocument {
        LeaseDocument {
            schema: SCHEMA.into(),
            pid: std::process::id(),
            token: self.token.clone(),
            root: self.root.clone(),
            url: url.map(str::to_string),
            started_at: self.started_at.clone(),
            executable: executable_identity(),
            version: Some(crate::VERSION.into()),
        }
    }

    /// Record the URL the server is listening on, atomically, so that a peer reading the
    /// file sees either no URL or the whole one. Refused when the file is no longer this
    /// process's: a lease taken over while its owner was binding belongs to whoever took it,
    /// and writing over it would leave two servers claiming one address.
    ///
    /// The document is written to a temporary file named for this lease's token and synced
    /// before anything else happens; then, under the lock every change to the lease takes
    /// (`take_over_with` says why there is one), the file is checked to be still this
    /// process's and the temporary file is renamed over it. No take-over can land between
    /// the check and the rename, and no other writer shares the temporary file.
    pub fn publish(&self, url: &str) -> Result<()> {
        self.publish_with(url, || {})
    }

    /// [`Lease::publish`], with a seam between the check and the rename, where the unit
    /// tests put an elector taking the lease over.
    fn publish_with(&self, url: &str, between: impl FnOnce()) -> Result<()> {
        let mut binding = self.binding.lock().map_err(|_| Error::Lease {
            reason: "the lease's binding lock is poisoned".into(),
        })?;
        *binding = false;
        let document = self.document(Some(url));
        let text = serde_json::to_string(&document).map_err(|e| Error::Lease {
            reason: format!("cannot render the lease: {e}"),
        })?;
        let tmp = tmp_path(&self.path, &self.token);
        let renamed = write_synced(&tmp, &text)
            .map_err(|e| Error::io(&tmp, e))
            .and_then(|()| self.replace_with(&tmp, between));
        if renamed.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        renamed?;
        sync_dir(&self.path);
        if let Ok(mut slot) = published().lock() {
            *slot = Some(document);
        }
        Ok(())
    }

    /// Rename `tmp` over the lease, under the change lock, only while the lease is still this
    /// process's.
    fn replace_with(&self, tmp: &Path, between: impl FnOnce()) -> Result<()> {
        let Some(_lock) = lock_for_change(&self.path, LOCK_PATIENCE) else {
            return Err(Error::Lease {
                reason: format!(
                    "the lock beside the lease at {} stayed held for {} seconds; not publishing over a lease another process is changing",
                    self.path.display(),
                    LOCK_PATIENCE.as_secs()
                ),
            });
        };
        if !self.is_mine() {
            return Err(Error::Lease {
                reason: format!(
                    "the lease at {} was taken over while this server was starting; another process serves this checkout",
                    self.path.display()
                ),
            });
        }
        between();
        fs::rename(tmp, &self.path).map_err(|e| Error::io(&self.path, e))
    }

    /// Is the file on disk still this process's lease?
    fn is_mine(&self) -> bool {
        holds(&self.path, &self.token)
    }

    /// Remove the lease: the server has stopped. Only while the file is still this process's,
    /// checked under the lock a take-over takes, so that a lease taken over since is never
    /// removed by the process it was taken from.
    pub fn release(mut self) {
        self.release_now();
    }

    fn release_now(&mut self) {
        if let Ok(mut binding) = self.binding.lock() {
            *binding = false;
        }
        if !self.released {
            remove_if_held(&self.path, &self.token);
        }
        self.released = true;
        if let Ok(mut slot) = published().lock() {
            if slot.as_ref().is_some_and(|d| d.token == self.token) {
                *slot = None;
            }
        }
        forget(Some(&self.token));
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.release_now();
    }
}

/// `SIGTERM`, `SIGINT` and `SIGHUP` — `serve stop`, Ctrl-C, a closing terminal — ask a
/// process that holds a lease to stop. The handler records the request and nothing else.
///
/// It used to unlink the lease's path and die of the signal, whenever the process had held a
/// lease. That was async-signal-safe and wrong: a lease taken over a moment earlier is the
/// successor's, and the old owner's reader notices the takeover only at its next tick, so a
/// signal in that window removed the *new* owner's lease (I2128). Checking the token first in
/// the handler would only shrink the window, not close it — a check and an unlink are two
/// system calls and a takeover fits between them — and reading and parsing a file is not
/// work a handler may do. So the handler counts, and normal code acts:
///
/// * the server's loop (`serve`) polls [`stop_requested`] and stops the server in order —
///   episodes closed, listeners closed, the lease released through the check under the
///   lock, the mesh drained — and the process returns from `main`;
/// * a server whose loop does not poll (`majordomus mcp`, whose main thread is its client's
///   stdio session) is stopped in the same order by its own reader thread, which then ends
///   the process with the signal it was sent;
/// * a watcher thread ends the process when nobody answers the request, when a second signal
///   arrives, or when the ordered stop takes longer than [`STOP_BOUND`] — releasing the
///   lease through the same check first. A third signal is the kernel's: the handler restores
///   the default disposition on the second.
///
/// A `kill -9` cannot be caught: the next process finds the stale lease and takes it over.
#[cfg(unix)]
mod signals {
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
    use std::sync::Once;
    use std::time::{Duration, Instant};

    static RECEIVED: AtomicUsize = AtomicUsize::new(0);
    static SIGNAL: AtomicI32 = AtomicI32::new(0);
    static ANSWERED: AtomicBool = AtomicBool::new(false);
    static INSTALL: Once = Once::new();

    /// Install the handlers and start the watcher, once per process.
    pub fn install() {
        INSTALL.call_once(|| {
            let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                // SAFETY: installing a handler that only touches lock-free atomics and calls
                // signal(2), both async-signal-safe
                unsafe { libc::signal(signal, handler) };
            }
            let _ = std::thread::Builder::new()
                .name("majordomus-signal-watcher".into())
                .spawn(watch);
        });
    }

    pub fn requested() -> Option<i32> {
        (RECEIVED.load(Ordering::SeqCst) > 0).then(|| SIGNAL.load(Ordering::SeqCst))
    }

    pub fn answer() {
        ANSWERED.store(true, Ordering::SeqCst);
    }

    fn watch() {
        let mut first: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let received = RECEIVED.load(Ordering::SeqCst);
            if received == 0 {
                continue;
            }
            let since = first.get_or_insert_with(Instant::now).elapsed();
            let answered = ANSWERED.load(Ordering::SeqCst);
            if super::watch_verdict(received, answered, since) == super::Watch::End {
                if received == 1 && answered {
                    tracing::warn!(
                        "the ordered stop did not finish within {} seconds; ending the process",
                        super::STOP_BOUND.as_secs()
                    );
                }
                super::end_by_signal(SIGNAL.load(Ordering::SeqCst));
            }
        }
    }

    extern "C" fn on_signal(signal: libc::c_int) {
        SIGNAL.store(signal, Ordering::SeqCst);
        if RECEIVED.fetch_add(1, Ordering::SeqCst) >= 1 {
            // SAFETY: restoring the default disposition is async-signal-safe; the next one
            // of this signal ends the process whatever the watcher is doing
            unsafe { libc::signal(signal, libc::SIG_DFL) };
        }
    }
}

#[cfg(not(unix))]
mod signals {
    pub fn install() {}

    pub fn requested() -> Option<i32> {
        None
    }

    pub fn answer() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identity of `path` as it stands now, the way [`executable_identity`] records the
    /// running executable.
    fn identity_of(path: &Path) -> ExecutableIdentity {
        let meta = fs::metadata(path).unwrap();
        let mtime = meta
            .modified()
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        ExecutableIdentity {
            path: path.to_path_buf(),
            mtime,
            size: meta.len(),
        }
    }

    #[test]
    fn an_unchanged_executable_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("majordomus");
        fs::write(&exe, b"the code that was loaded").unwrap();
        assert_eq!(stale_reason(&identity_of(&exe)), None);
    }

    #[test]
    fn a_replaced_executable_is_stale_and_says_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("majordomus");
        fs::write(&exe, b"the code that was loaded").unwrap();
        let pinned = identity_of(&exe);
        // a rebuild writes a different file over the same path; the size alone differs,
        // whatever the clock's resolution makes of the mtime
        fs::write(&exe, b"the code a rebuild put there afterwards").unwrap();
        let reason = stale_reason(&pinned).expect("a replaced executable is stale");
        assert!(reason.contains("has been replaced"), "{reason}");
        assert!(reason.contains("no longer on disk"), "{reason}");
        assert!(
            !reason.contains(&exe.display().to_string()),
            "the reason names no path: {reason}"
        );
    }

    #[test]
    fn a_removed_executable_is_stale_and_says_removed() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("majordomus");
        fs::write(&exe, b"the code that was loaded").unwrap();
        let pinned = identity_of(&exe);
        fs::remove_file(&exe).unwrap();
        let reason = stale_reason(&pinned).expect("a removed executable is stale");
        assert!(reason.contains("has been removed"), "{reason}");
        assert!(
            !reason.contains(&exe.display().to_string()),
            "the reason names no path: {reason}"
        );
    }

    #[test]
    fn this_process_is_not_serving_replaced_code() {
        assert!(started_as().is_some(), "a test binary knows where it is");
        assert_eq!(serving_replaced_code(), None);
    }

    /// A lease document as another process would write it.
    fn lease_text(token: &str, url: Option<&str>) -> String {
        serde_json::to_string(&LeaseDocument {
            schema: SCHEMA.into(),
            pid: std::process::id(),
            token: token.into(),
            root: "/r".into(),
            url: url.map(str::to_string),
            started_at: "2026-10-09T00:00:00Z".into(),
            executable: None,
            version: None,
        })
        .unwrap()
    }

    /// What an elector does once the path is free: create the file exclusively and write its
    /// lease into it. `true` when this elector is the one that created it.
    fn create(path: &Path, token: &str) -> bool {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut file) => {
                file.write_all(lease_text(token, None).as_bytes()).unwrap();
                true
            }
            Err(_) => false,
        }
    }

    fn token_on_disk(path: &Path) -> Option<String> {
        LeaseFile::read(path).document().map(|d| d.token.clone())
    }

    fn binding_lease(path: &Path) -> Lease {
        Lease {
            path: path.to_path_buf(),
            token: "owner".into(),
            root: "/r".into(),
            released: false,
            started_at: "2026-10-09T00:00:00Z".into(),
            binding: Arc::new(Mutex::new(true)),
        }
    }

    /// Two electors judged the same lease stale. Elector B has read it and compared it with
    /// what it judged; elector A makes the same judgement at that very moment, takes the lease
    /// over and creates its own. When B then removes "the stale lease", it must not be A's.
    #[test]
    fn two_electors_that_judged_one_lease_stale_end_with_exactly_one_lease() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        fs::write(&path, lease_text("stale", Some("http://127.0.0.1:1"))).unwrap();
        let seen = LeaseFile::read(&path);

        let mut a_created = false;
        take_over_with(&path, &seen, LOCK_PATIENCE, || {
            // A, at B's worst moment, without waiting for anybody
            let _ = take_over_with(&path, &seen, Duration::ZERO, || {});
            a_created = create(&path, "a");
        })
        .unwrap();
        // B's next round of the election
        let b_created = create(&path, "b");

        let on_disk = token_on_disk(&path);
        assert!(
            a_created != b_created,
            "exactly one elector believes it created the lease (A {a_created}, B {b_created}); \
             on disk: {on_disk:?}"
        );
        let creator = if a_created { "a" } else { "b" };
        assert_eq!(
            on_disk.as_deref(),
            Some(creator),
            "the lease names the elector that created it"
        );
    }

    /// The lease of a process still binding, judged abandoned by an elector between the
    /// owner's check that the lease is its own and the rename that publishes the URL.
    #[test]
    fn a_takeover_between_the_check_and_the_rename_of_publish_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        fs::write(&path, lease_text("owner", None)).unwrap();
        let lease = binding_lease(&path);
        let seen = LeaseFile::read(&path);
        let mut taker_created = false;
        let published = lease.publish_with("http://127.0.0.1:2", || {
            let _ = take_over_with(&path, &seen, Duration::ZERO, || {});
            taker_created = create(&path, "taker");
        });
        let doc = LeaseFile::read(&path)
            .document()
            .expect("a lease on disk")
            .clone();
        if taker_created {
            assert_eq!(doc.token, "taker", "the taker's lease was written over");
            assert!(
                published.is_err(),
                "publish reported success over a takeover"
            );
        } else {
            assert_eq!(doc.token, "owner");
            assert_eq!(doc.url.as_deref(), Some("http://127.0.0.1:2"));
            assert!(published.is_ok(), "{published:?}");
        }
        lease.release();
    }

    /// The release a stopping process makes from a thread that does not own the [`Lease`]:
    /// its own lease is removed, a successor's is not.
    #[test]
    fn a_release_on_the_way_out_spares_a_successor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        fs::write(&path, lease_text("successor", Some("http://127.0.0.1:4"))).unwrap();
        assert!(!remove_if_held(&path, "the-old-owner"));
        assert_eq!(token_on_disk(&path).as_deref(), Some("successor"));
        assert!(
            remove_if_held(&path, "successor"),
            "the holder removes its own"
        );
        assert_eq!(LeaseFile::read(&path), LeaseFile::Absent);
    }

    /// A change waits for another holder of the lock only as long as it was told to, and
    /// makes no change when that runs out.
    #[test]
    fn a_held_change_lock_refuses_a_second_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        fs::write(&path, lease_text("owner", None)).unwrap();
        let held = lock_for_change(&path, Duration::ZERO).expect("a free lock is taken");
        assert!(lock_for_change(&path, Duration::from_millis(20)).is_none());
        assert!(
            !remove_if_held(&path, "owner"),
            "no removal without the lock"
        );
        drop(held);
        assert!(remove_if_held(&path, "owner"));
    }

    #[test]
    fn the_watcher_leaves_the_first_signal_to_a_server_that_answers_it_for_the_bound() {
        assert_eq!(watch_verdict(0, true, Duration::ZERO), Watch::Wait);
        assert_eq!(watch_verdict(1, true, Duration::ZERO), Watch::Wait);
        assert_eq!(
            watch_verdict(1, true, STOP_BOUND),
            Watch::End,
            "the bound passed"
        );
        assert_eq!(
            watch_verdict(1, false, Duration::ZERO),
            Watch::End,
            "nobody answers"
        );
        assert_eq!(
            watch_verdict(2, true, Duration::ZERO),
            Watch::End,
            "a second signal"
        );
    }

    /// The release [`release_held`] makes from the record of the election: the lease it
    /// names goes only while the file is still that lease.
    #[test]
    fn the_release_on_a_signal_removes_only_the_lease_the_record_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        let record = || Some((path.clone(), "held-by-this-test".to_string()));
        fs::write(&path, lease_text("successor", None)).unwrap();
        release_record(record());
        assert_eq!(token_on_disk(&path).as_deref(), Some("successor"));
        fs::write(&path, lease_text("held-by-this-test", None)).unwrap();
        release_record(record());
        assert_eq!(LeaseFile::read(&path), LeaseFile::Absent);
        release_record(None);
    }

    /// A temporary file is per writer: another writer's temporary file (here, something at
    /// the old fixed name that is not even a file) neither blocks this one nor is replaced.
    #[test]
    fn publish_writes_through_a_temporary_file_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.json");
        fs::write(&path, lease_text("owner", None)).unwrap();
        fs::create_dir(dir.path().join("server.json.tmp")).unwrap();
        let lease = binding_lease(&path);
        lease
            .publish("http://127.0.0.1:3")
            .expect("publish does not share a temporary name with anybody");
        assert_eq!(
            LeaseFile::read(&path)
                .document()
                .and_then(|d| d.url.clone()),
            Some("http://127.0.0.1:3".into())
        );
        lease.release();
        assert_eq!(LeaseFile::read(&path), LeaseFile::Absent);
    }
}
