//! One shared server per repository: the lease that decides who it is. The first
//! `majordomus mcp` (or `serve`) to create `state/mcp/server.json` under the checkout's
//! local half owns the server and publishes its URL there; every later process reads the
//! file, checks that the server answers for this root, and attaches to it. A lease whose
//! server does not answer is stale, and the next process takes it over. The file is the
//! only thing the server writes anywhere, it lives under `.ai/local/` (never tracked, by
//! the layer's contract), and it is removed when the server stops.

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
pub fn elect(repo: &Repository) -> Result<Role> {
    let path = lease_path(repo);
    let root = repo.root().to_path_buf();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let waited_since = Instant::now();
    for _ in 0..50 {
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
                return Ok(Role::Server(lease));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let text = fs::read_to_string(&path).unwrap_or_default();
                let doc: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match doc["url"].as_str() {
                    Some(url) if probe(url, &root) => {
                        return Ok(Role::Peer {
                            url: url.to_string(),
                        })
                    }
                    Some(url) => {
                        tracing::warn!(lease = %path.display(), url, "stale lease: the server it names does not answer for this repository; taking it over");
                        let _ = fs::remove_file(&path);
                    }
                    None => {
                        let age = fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|m| m.elapsed().ok())
                            .unwrap_or(Duration::ZERO);
                        if age > BIND_GRACE || waited_since.elapsed() > BIND_GRACE {
                            tracing::warn!(lease = %path.display(), "abandoned lease: its owner never published a URL; taking it over");
                            let _ = fs::remove_file(&path);
                        } else {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            Err(e) => return Err(Error::io(&path, e)),
        }
    }
    Err(Error::Lease {
        reason: format!(
            "could not acquire or join the lease at {} after repeated attempts",
            path.display()
        ),
    })
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
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.release_now();
    }
}
