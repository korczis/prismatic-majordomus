//! One shared server per repository: the lease that decides who it is. The first
//! `majordomus mcp` (or `serve`) to create `state/mcp/server.json` under the checkout's
//! local half owns the server and publishes its URL there; every later process reads the
//! file, checks that the server answers for this root, and attaches to it. A lease whose
//! server does not answer is stale, and the next process takes it over; so is a file that
//! is not a lease document, an empty one, or one whose owner never published a URL. The
//! file is the only thing the server writes anywhere, it lives under `.ai/local/` (never
//! tracked, by the layer's contract), and it is removed when the server stops, or when
//! the server dies of `SIGTERM`, `SIGINT` or `SIGHUP`.

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

/// How long a process keeps trying to acquire or join the lease before it gives up and
/// says so: the bind grace with a margin for the probes. The caller then serves its
/// client alone.
pub const JOIN_TIMEOUT: Duration = Duration::from_secs(20);

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
    /// The process id of the server. Informational: a live pid is not a live server, and
    /// nothing decides liveness from it.
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
                signals::hold(&path);
                return Ok(Role::Server(lease));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let (found, seen) = inspect(&path, &root);
                match found {
                    Found::Live(url) => return Ok(Role::Peer { url }),
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
        if waited_since.elapsed() > JOIN_TIMEOUT {
            return Err(Error::Lease {
                reason: format!(
                    "could not acquire or join the lease at {} within {} seconds",
                    path.display(),
                    JOIN_TIMEOUT.as_secs()
                ),
            });
        }
    }
}

/// What a lease file that already exists says.
enum Found {
    /// A server answers at this URL for this root.
    Live(String),
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

/// The executable this process runs, as recorded at start, for a server to compare against
/// the file later: same path, same file — or a rebuild happened underneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableIdentity(Option<Value>);

impl ExecutableIdentity {
    /// What the file at this process's own path is right now.
    pub fn now() -> Self {
        ExecutableIdentity(executable_identity())
    }

    /// Has the file this process was started from been replaced since `self` was taken?
    ///
    /// `false` when either side is unknown: a process that cannot stat itself makes no
    /// claim, rather than a false one.
    pub fn replaced_on_disk(&self) -> bool {
        match (&self.0, executable_identity()) {
            (Some(then), Some(now)) => {
                then["path"] == now["path"]
                    && (then["mtime"] != now["mtime"] || then["size"] != now["size"])
            }
            _ => false,
        }
    }
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
        LeaseFile::Empty if age > BIND_GRACE => {
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
        Some(url) if probe(url, root) => Found::Live(url.to_string()),
        Some(url) => Found::Stale(format!(
            "stale lease: the server it names at {url} does not answer for this repository"
        )),
        None if age > BIND_GRACE => {
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
    if LeaseFile::read(path) != *seen {
        tracing::debug!(lease = %path.display(), "the lease changed under the take-over; reading it again");
        return Ok(());
    }
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

/// This process's lease was taken over by another process: from now on a signal must not
/// unlink the file, which is somebody else's, and this process stops claiming to be the
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
    signals::release();
}

/// Does a Majordomus server answer at `url` for the repository at `root`?
///
/// Version included: a server of another version is [`Probed::OtherVersion`], never
/// `Ours`. It is the second half of the executable check in [`superseded`] — that one sees
/// a binary replaced under a server started from this process's own path, this one sees a
/// server started from any path at all, hours ago, by a release install or another
/// checkout's build. Both are the same failure: a process that answers, from code that is
/// not the code on disk.
pub fn probe(url: &str, root: &Path) -> bool {
    probe_detail(url, root) == Probed::Ours
}

/// [`probe`], with the reason.
pub fn probe_detail(url: &str, root: &Path) -> Probed {
    match bridge::request(url, "GET", "/", &[], None, PROBE_TIMEOUT) {
        Ok(reply) if reply.status == 200 => {
            let v: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
            // the identity and not the path: the index names the repository it serves
            // without telling every caller where the checkout sits
            let ours = v["name"] == "majordomus"
                && v["repository_id"].as_str() == Some(crate::repository::identity(root).as_str());
            if !ours {
                return Probed::Absent;
            }
            match v["version"].as_str() {
                Some(version) if version == crate::VERSION => Probed::Ours,
                Some(version) => Probed::OtherVersion(version.to_string()),
                // a server too old to say: everything before the version was published
                // on the index, and so older than any executable that asks
                None => Probed::OtherVersion("unknown".into()),
            }
        }
        _ => Probed::Absent,
    }
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
        let tick = BIND_GRACE / 3;
        let _ = std::thread::Builder::new()
            .name("majordomus-lease-keep-alive".into())
            .spawn(move || loop {
                std::thread::sleep(tick);
                let Ok(guard) = binding.lock() else { return };
                if !*guard {
                    return;
                }
                let mine = LeaseFile::read(&path)
                    .document()
                    .is_some_and(|d| d.token == token);
                if !mine {
                    return;
                }
                let tmp = path.with_extension("json.tmp");
                if fs::write(&tmp, &document).is_ok() {
                    let _ = fs::rename(&tmp, &path);
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
    pub fn publish(&self, url: &str) -> Result<()> {
        let mut binding = self.binding.lock().map_err(|_| Error::Lease {
            reason: "the lease's binding lock is poisoned".into(),
        })?;
        *binding = false;
        if !self.is_mine() {
            return Err(Error::Lease {
                reason: format!(
                    "the lease at {} was taken over while this server was starting; another process serves this checkout",
                    self.path.display()
                ),
            });
        }
        let tmp = self.path.with_extension("json.tmp");
        let document = self.document(Some(url));
        let text = serde_json::to_string(&document).map_err(|e| Error::Lease {
            reason: format!("cannot render the lease: {e}"),
        })?;
        fs::write(&tmp, text).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &self.path).map_err(|e| Error::io(&self.path, e))?;
        if let Ok(mut slot) = published().lock() {
            *slot = Some(document);
        }
        Ok(())
    }

    /// Is the file on disk still this process's lease?
    fn is_mine(&self) -> bool {
        LeaseFile::read(&self.path)
            .document()
            .is_some_and(|d| d.token == self.token)
    }

    /// Remove the lease: the server has stopped.
    pub fn release(mut self) {
        self.release_now();
    }

    fn release_now(&mut self) {
        if let Ok(mut binding) = self.binding.lock() {
            *binding = false;
        }
        if !self.released && self.is_mine() {
            let _ = fs::remove_file(&self.path);
        }
        self.released = true;
        if let Ok(mut slot) = published().lock() {
            if slot.as_ref().is_some_and(|d| d.token == self.token) {
                *slot = None;
            }
        }
        signals::release();
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.release_now();
    }
}

/// The lease is removed when the process dies of `SIGTERM`, `SIGINT` or `SIGHUP`: a client
/// killing its server, a person pressing Ctrl-C, a terminal closing. The handler does only
/// what is safe inside a signal handler: `unlink` the path recorded when the lease was
/// taken, restore the default disposition, and raise the signal again so that the exit
/// status still says which signal it was. The path is recorded once per process (a
/// repository's lease never moves) and is never freed, because a handler may be reading
/// it. A `kill -9` cannot be caught: the next process finds the stale lease and takes it
/// over.
#[cfg(unix)]
mod signals {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    use std::sync::Once;

    static HELD: AtomicBool = AtomicBool::new(false);
    static PATH: AtomicPtr<libc::c_char> = AtomicPtr::new(std::ptr::null_mut());
    static INSTALL: Once = Once::new();

    /// This process now holds the lease at `path`; install the handlers the first time.
    pub fn hold(path: &Path) {
        let Ok(c) = CString::new(path.as_os_str().as_bytes()) else {
            return;
        };
        let raw = c.into_raw();
        if PATH
            .compare_exchange(
                std::ptr::null_mut(),
                raw,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            // the path is already recorded (the same repository, taken over again): this
            // copy is not needed, and the recorded one stays reachable for the handler
            drop(unsafe { CString::from_raw(raw) });
        }
        HELD.store(true, Ordering::SeqCst);
        INSTALL.call_once(|| {
            let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                // SAFETY: installing a handler that only calls async-signal-safe functions
                unsafe { libc::signal(signal, handler) };
            }
        });
    }

    /// The lease is released (or was never this process's any more): stop removing it.
    pub fn release() {
        HELD.store(false, Ordering::SeqCst);
    }

    extern "C" fn on_signal(signal: libc::c_int) {
        if HELD.load(Ordering::SeqCst) {
            let path = PATH.load(Ordering::SeqCst);
            if !path.is_null() {
                // SAFETY: a valid NUL-terminated path that is never freed; unlink is
                // async-signal-safe
                unsafe { libc::unlink(path) };
            }
        }
        // SAFETY: restoring the default disposition and re-raising are async-signal-safe
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
    }
}

#[cfg(not(unix))]
mod signals {
    use std::path::Path;

    pub fn hold(_: &Path) {}

    pub fn release() {}
}
