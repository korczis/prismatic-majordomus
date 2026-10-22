//! A minimal, optional, read-only view of git.
//!
//! Where the work tree is, what HEAD is, and which tracked files match a pathspec — and
//! nothing else. Everything shells out to the `git` binary; no library is linked, nothing
//! is written, and GitHub is not involved at any point.
//!
//! # Optional means optional
//!
//! Every function here answers when git is absent, when the directory is not a repository,
//! and when the repository has no commits. [`inspect`] returns a [`GitState`] that says
//! which of those it is rather than an error, because a repository of the layer does not
//! have to be version controlled — discovery falls back to a filesystem walk with the same
//! glob semantics, and the index says which mode it used.
//!
//! ```
//! use majordomus_cli::git::{inspect, GitState};
//!
//! // a directory that is not a work tree is a state with a reason, not a failure
//! let plain = tempfile::tempdir().unwrap();
//! match inspect(plain.path()) {
//!     GitState::Unavailable { reason } => assert!(!reason.is_empty(), "it says why"),
//!     GitState::Available(_) => panic!("a fresh temporary directory is not a work tree"),
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// What git says about the repository, or why it could not be asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum GitState {
    /// `git` answered; the facts follow.
    Available(GitInfo),
    /// `git` could not be asked, or the root is not a work tree; the reason says which.
    Unavailable {
        /// What `git` said, or why it could not be run.
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What `git` said about the repository.
pub struct GitInfo {
    /// Absolute path of the work tree top level.
    pub toplevel: PathBuf,
    /// Full commit id of HEAD, or `None` in an unborn repository.
    pub head: Option<String>,
    /// Branch name, or `None` when detached or unborn.
    pub branch: Option<String>,
    /// `clean` or `dirty`, from `git status --porcelain`.
    pub working_tree: String,
}

/// Ask git about `root`. Never fails: a missing `git` or a directory that is not a work
/// tree becomes [`GitState::Unavailable`] with the reason.
pub fn inspect(root: &Path) -> GitState {
    let toplevel = match run(root, &["rev-parse", "--show-toplevel"]) {
        Ok(s) => PathBuf::from(s.trim_end()),
        Err(e) => {
            return GitState::Unavailable {
                reason: e.to_string(),
            }
        }
    };
    let head = run(root, &["rev-parse", "--verify", "-q", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());
    let branch = run(root, &["symbolic-ref", "-q", "--short", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());
    let working_tree = match run(root, &["status", "--porcelain"]) {
        Ok(s) if s.trim().is_empty() => "clean",
        Ok(_) => "dirty",
        Err(_) => "unknown",
    }
    .to_string();
    GitState::Available(GitInfo {
        toplevel,
        head,
        branch,
        working_tree,
    })
}

/// Every tracked file under `root`, repository-relative, in the byte order the index keeps
/// them in: one subprocess per build, matched against each class's pathspec in process.
pub fn ls_files_all(root: &Path) -> Result<Vec<String>> {
    ls_files_with(root, &[])
}

/// Tracked files under `root` matching one pathspec, repository-relative, in the byte order
/// the index keeps them in. The pathspec is passed to git verbatim.
pub fn ls_files(root: &Path, pathspec: &str) -> Result<Vec<String>> {
    ls_files_with(root, &[pathspec])
}

/// Tracked files under `root` matching any of several pathspecs, repository-relative, in
/// the byte order the index keeps them in — one subprocess, one pass, each file once.
///
/// Asking per pathspec and concatenating would give neither: a file two pathspecs both
/// select would appear twice, and the order would be the caller's rather than git's. A
/// hash taken over the result depends on both, which is why this exists as its own call.
///
/// ```
/// use majordomus_cli::git::ls_files_any;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap()
/// };
/// git(&["init", "-q"]);
/// std::fs::create_dir(dir.path().join("lib")).unwrap();
/// std::fs::write(dir.path().join("lib/b.sh"), "b").unwrap();
/// std::fs::write(dir.path().join("a.md"), "a").unwrap();
/// git(&["add", "-A"]);
///
/// // git's own index order, and each file once however many pathspecs select it
/// let files = ls_files_any(dir.path(), &["a.md", "lib/**", "*.md"]).unwrap();
/// assert_eq!(files, vec!["a.md".to_string(), "lib/b.sh".to_string()]);
/// ```
pub fn ls_files_any(root: &Path, pathspecs: &[&str]) -> Result<Vec<String>> {
    ls_files_with(root, pathspecs)
}

/// A `git` invocation against `root` that only ever reads: the one constructor every
/// read in this crate goes through.
///
/// # Why a reader needs a constructor at all
///
/// `git status` refreshes the staging index as a side effect — when an entry's recorded
/// stat data no longer matches the file, or is *racily* close to the index's own
/// modification time, git rewrites `.git/index` with the refreshed data. Nothing about
/// the repository changed; the reader changed the file it read.
///
/// That file is one of the six [`crate::live::WORKTREE_FILES`] the shared server stamps to
/// decide whether the repository has moved. So a served page that asked git for the state
/// of the working tree moved the stamp, and the *next* request found a stamp it did not
/// recognise and paid a whole [`crate::app::App::load`] — discovery, every declared file
/// read and validated, the registry, the Why catalogue, the product model, the web
/// topology — to rebuild a picture that was already current. An observer that disturbs
/// what it observes, and then charges the next caller for the disturbance.
///
/// `GIT_OPTIONAL_LOCKS=0` is git's own name for this: complete the request without any
/// optional sub-operation that would take a lock, which is exactly the index write-back.
/// Locks a command genuinely requires — `commit`, `add` — are unaffected, so this is safe
/// on any invocation and is not a reason to keep two constructors.
///
/// `GIT_DIR` and `GIT_WORK_TREE` are removed for a second reason: inherited from a hook's
/// environment they would silently redirect `-C root` at another repository.
///
/// ```
/// use majordomus_cli::git::read_only;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap()
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// std::fs::write(dir.path().join("a"), "a").unwrap();
/// git(&["add", "-A"]);
/// git(&["commit", "-qm", "one"]);
///
/// // the same answer a plain `git status` gives, with the staging index left alone —
/// // which is the whole difference, since that file is what the server watches
/// let index = dir.path().join(".git/index");
/// let before = std::fs::metadata(&index).unwrap().modified().unwrap();
/// let out = read_only(dir.path()).args(["status", "--porcelain"]).output().unwrap();
/// assert!(out.status.success());
/// assert!(out.stdout.is_empty(), "a committed tree is clean");
/// assert_eq!(
///     std::fs::metadata(&index).unwrap().modified().unwrap(),
///     before,
///     "reading the working tree must not rewrite the staging index"
/// );
/// ```
pub fn read_only(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.arg("-C").arg(root);
    cmd
}

fn ls_files_with(root: &Path, pathspecs: &[&str]) -> Result<Vec<String>> {
    let out = read_only(root)
        .args(["ls-files", "-z", "--"])
        .args(pathspecs)
        .output()
        .map_err(|e| Error::Git {
            reason: format!("cannot run git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Git {
            reason: format!(
                "git ls-files -- {}: {}",
                pathspecs.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    let mut files = Vec::new();
    for raw in out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        match std::str::from_utf8(raw) {
            Ok(s) => files.push(s.to_string()),
            Err(_) => {
                return Err(Error::Git {
                    reason: format!(
                        "a tracked path is not UTF-8: {}",
                        String::from_utf8_lossy(raw)
                    ),
                })
            }
        }
    }
    Ok(files)
}

/// Is `ancestor` reachable from `descendant`? `None` when git cannot answer at all — a
/// missing executable, a commit this clone does not have — which is a third answer and not
/// a `false`: "not an ancestor" and "unknown ancestry" lead a reader to different actions,
/// and collapsing them is how a record from a rewritten history gets read as current.
///
/// Exposed rather than left to `run` because ancestry is the one git question the
/// divergence label depends on, and a caller that had the whole command runner in order to
/// ask it could ask anything.
pub fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> Option<bool> {
    let out = read_only(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output()
        .ok()?;
    match out.status.code() {
        Some(0) => Some(true),
        // 1 is "no". Anything else git exits with — 128 for an object this clone does not
        // have, for instance — is also not a yes, and answering `None` there would be
        // generous in the one direction that costs something: a record naming a commit
        // this repository has never seen is exactly the record that must not be read as
        // current. `None` is reserved for git not running at all.
        Some(_) => Some(false),
        None => None,
    }
}

fn run(root: &Path, args: &[&str]) -> Result<String> {
    let out = read_only(root)
        .args(args)
        .output()
        .map_err(|e| Error::Git {
            reason: format!("cannot run git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Git {
            reason: format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    String::from_utf8(out.stdout).map_err(|e| Error::Git {
        reason: e.to_string(),
    })
}
