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
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};

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

/// The lease this process holds. Dropping it removes the file (when the file is still
/// this process's), so a failed start never leaves a stale lease behind.
#[derive(Debug)]
pub struct Lease {
    path: PathBuf,
    token: String,
    root: PathBuf,
    released: bool,
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
    repo.root().join(repo.local_path()).join(LEASE_PATH)
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
                };
                let text = lease.document(None).to_string();
                file.write_all(text.as_bytes())
                    .map_err(|e| Error::io(&path, e))?;
                signals::hold(&path);
                return Ok(Role::Server(lease));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                match inspect(&path, &root) {
                    Found::Live(url) => return Ok(Role::Peer { url }),
                    Found::Stale(reason) => {
                        tracing::warn!(lease = %path.display(), "{reason}; taking it over");
                        take_over(&path)?;
                    }
                    Found::Binding if waited_since.elapsed() > BIND_GRACE => {
                        tracing::warn!(lease = %path.display(), "abandoned lease: its owner never published a URL; taking it over");
                        take_over(&path)?;
                    }
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

/// Read and classify an existing lease file.
fn inspect(path: &Path, root: &Path) -> Found {
    let text = fs::read_to_string(path).unwrap_or_default();
    let age = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| m.elapsed().ok())
        .unwrap_or(Duration::ZERO);
    if text.trim().is_empty() {
        return if age > BIND_GRACE {
            Found::Stale("empty lease: its owner never wrote it".into())
        } else {
            Found::Binding
        };
    }
    let doc: Value = match serde_json::from_str(&text) {
        Ok(doc) => doc,
        Err(e) => return Found::Stale(format!("corrupt lease: not JSON ({e})")),
    };
    if !doc.is_object() || doc["schema"] != SCHEMA {
        return Found::Stale(format!("corrupt lease: not a {SCHEMA} document"));
    }
    match doc["url"].as_str() {
        Some(url) if probe(url, root) => Found::Live(url.to_string()),
        Some(url) => Found::Stale(format!(
            "stale lease: the server it names at {url} does not answer for this repository"
        )),
        None if age > BIND_GRACE => {
            Found::Stale("abandoned lease: its owner never published a URL".into())
        }
        None => Found::Binding,
    }
}

/// Remove a lease that cannot be used, so that the next attempt creates a fresh one.
fn take_over(path: &Path) -> Result<()> {
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

/// Does a Majordomus server answer at `url` for the repository at `root`?
pub fn probe(url: &str, root: &Path) -> bool {
    match bridge::request(url, "GET", "/", &[], None, PROBE_TIMEOUT) {
        Ok(reply) if reply.status == 200 => {
            let v: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
            v["name"] == "majordomus" && v["root"].as_str() == root.to_str()
        }
        _ => false,
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

    fn document(&self, url: Option<&str>) -> Value {
        json!({
            "schema": SCHEMA,
            "pid": std::process::id(),
            "token": self.token,
            "root": self.root,
            "url": url,
            "started_at": crate::peers::rfc3339(SystemTime::now()),
        })
    }

    /// Record the URL the server is listening on, atomically, so that a peer reading the
    /// file sees either no URL or the whole one.
    pub fn publish(&self, url: &str) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, self.document(Some(url)).to_string()).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &self.path).map_err(|e| Error::io(&self.path, e))
    }

    /// Is the file on disk still this process's lease?
    fn is_mine(&self) -> bool {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v["token"] == self.token.as_str())
    }

    /// Remove the lease: the server has stopped.
    pub fn release(mut self) {
        self.release_now();
    }

    fn release_now(&mut self) {
        if !self.released && self.is_mine() {
            let _ = fs::remove_file(&self.path);
        }
        self.released = true;
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
