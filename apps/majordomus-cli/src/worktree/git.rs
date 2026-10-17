//! The one place the worktree topology runs `git`.
//!
//! Arguments are passed as arguments — never assembled into a shell string — so a path with
//! a space, a quote, a newline or a leading hyphen is passed to git exactly as it is. Every
//! path handed to a `git worktree` subcommand is absolute: git resolves a relative path
//! against its own working directory and matches a bare name against the *suffix* of any
//! registered worktree, and either of those is how a worktree ends up inside the primary
//! checkout. Every run captures status, stdout and stderr and turns a failure into a typed
//! [`WorktreeError::GitCommandFailed`] carrying git's own words; nothing here parses
//! human-oriented output. One function reaches the network, [`fetch_bounded`], and only
//! `worktree create` calls it: a new branch starts from the remote's trunk as it is now.

use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::error::{Result, WorktreeError};

/// What a git run produced.
#[derive(Debug, Clone)]
pub struct Output {
    /// Standard output, as bytes: a path git prints is bytes, not necessarily UTF-8.
    pub stdout: Vec<u8>,
    /// Standard error, trimmed, for the error message.
    pub stderr: String,
    /// The process's exit status code, when it exited normally.
    pub status: Option<i32>,
}

impl Output {
    /// Standard output as text, with the trailing newline removed. A path git cannot print
    /// as UTF-8 is a path this tool cannot name, and saying so beats a lossy rendering.
    pub fn text(&self) -> Result<String> {
        match std::str::from_utf8(&self.stdout) {
            Ok(s) => Ok(s.trim_end_matches(['\n', '\r']).to_string()),
            Err(_) => Err(WorktreeError::GitUnavailable {
                reason: "git answered with a path that is not valid UTF-8; \
                         this tool names paths as text and cannot represent it"
                    .into(),
            }),
        }
    }
}

/// Run `git -C <cwd> <args>` and answer with its output whatever it exited with.
///
/// The caller decides whether a non-zero status is a failure: `git rev-parse` exiting
/// non-zero is an answer ("no such ref"), while `git worktree add` exiting non-zero is not.
pub fn try_run<S: AsRef<OsStr>>(cwd: &Path, args: &[S]) -> Result<Output> {
    let mut cmd = Command::new("git");
    // Never let a user's environment change what these commands mean, and never take an
    // optional lock that would make a read block behind somebody's fetch.
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.arg("-C").arg(cwd);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().map_err(|e| WorktreeError::GitUnavailable {
        reason: format!("{e} (running git in {})", cwd.display()),
    })?;
    Ok(Output {
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        status: out.status.code(),
    })
}

/// Run `git -C <cwd> <args>` and refuse anything but success.
pub fn run<S: AsRef<OsStr>>(cwd: &Path, args: &[S]) -> Result<Output> {
    let out = try_run(cwd, args)?;
    if out.status == Some(0) {
        return Ok(out);
    }
    Err(WorktreeError::GitCommandFailed {
        command: args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" "),
        status: out
            .status
            .map(|c| c.to_string())
            .unwrap_or_else(|| "terminated by a signal".into()),
        stderr: if out.stderr.is_empty() {
            "git said nothing".into()
        } else {
            out.stderr
        },
    })
}

/// Does `rev` resolve to a commit in this repository? Never fetches: an unknown ref is
/// "no", not "let me look upstream".
pub fn rev_exists(cwd: &Path, rev: &str) -> Result<bool> {
    // `--end-of-options` so a ref that begins with a hyphen is a ref, not an option.
    let out = try_run(
        cwd,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )?;
    Ok(out.status == Some(0))
}

/// Does a local branch of this name exist?
pub fn branch_exists(cwd: &Path, branch: &str) -> Result<bool> {
    let out = try_run(
        cwd,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("refs/heads/{branch}"),
        ],
    )?;
    Ok(out.status == Some(0))
}

/// Is this branch name one git will accept for `refs/heads/`? Asked of git as the authority
/// before a branch is created; [`super::path::BranchName`] carries the same rules so that a
/// path can be derived offline and without a subprocess, and this is the check that the
/// two never disagree in the direction that matters.
pub fn branch_name_is_valid(cwd: &Path, branch: &str) -> Result<bool> {
    // The full reference is checked rather than `--branch`: `--branch` also expands
    // shorthands like `@{-1}`, which is a different question, and prefixing `refs/heads/`
    // means the argument never starts with a hyphen.
    let out = try_run(cwd, &["check-ref-format", &format!("refs/heads/{branch}")])?;
    Ok(out.status == Some(0))
}

/// The NUL-separated porcelain status of a work tree: what `git status --porcelain -z`
/// printed, unparsed. Empty means clean, tracked and untracked alike.
pub fn status_porcelain_z(worktree: &Path, untracked: &str) -> Result<Vec<u8>> {
    let out = run(
        worktree,
        &[
            "status",
            "--porcelain",
            "-z",
            "--no-renames",
            &format!("--untracked-files={untracked}"),
        ],
    )?;
    Ok(out.stdout)
}

/// The commit a work tree's HEAD names, or `None` in an unborn repository.
pub fn head_of(worktree: &Path) -> Result<Option<String>> {
    let out = try_run(worktree, &["rev-parse", "--verify", "-q", "HEAD"])?;
    if out.status == Some(0) {
        Ok(Some(out.text()?))
    } else {
        Ok(None)
    }
}

/// The branch a work tree's HEAD names, or `None` when detached.
pub fn branch_of(worktree: &Path) -> Result<Option<String>> {
    let out = try_run(worktree, &["symbolic-ref", "-q", "--short", "HEAD"])?;
    if out.status == Some(0) {
        Ok(Some(out.text()?))
    } else {
        Ok(None)
    }
}

/// How long a fetch made on a person's behalf may take before it is abandoned. A create
/// that waits on a dead network for longer than this is worse than one that says so.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a fetch did not bring the refs it was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchFailure {
    /// The remote answered and does not have the ref: a fact, not a fault.
    NoSuchRemoteRef,
    /// The remote could not be read: git's own words, or the timeout that ended it.
    Unreachable(String),
}

/// Fetch `refspecs` from `remote`, quietly and within `timeout`.
///
/// No tags, no submodules, no `FETCH_HEAD`, and never a prompt: a credential git would
/// have to ask a person for is a failure here, not a hang. A fetch still running at the
/// deadline is killed. The answer is whether the refs arrived and, when not, why; the only
/// `Err` is git itself not running.
pub fn fetch_bounded(
    cwd: &Path,
    remote: &str,
    refspecs: &[String],
    timeout: Duration,
) -> Result<std::result::Result<(), FetchFailure>> {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.arg("-C").arg(cwd);
    cmd.args([
        "fetch",
        "--quiet",
        "--no-tags",
        "--no-recurse-submodules",
        "--no-write-fetch-head",
        "--end-of-options",
        remote,
    ]);
    cmd.args(refspecs);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| WorktreeError::GitUnavailable {
        reason: format!("{e} (running git fetch in {})", cwd.display()),
    })?;
    // Drained beside the wait, so a chatty failure cannot fill the pipe and stall it.
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut text);
        }
        text
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                return Err(WorktreeError::GitUnavailable {
                    reason: format!("{e} (waiting for git fetch in {})", cwd.display()),
                })
            }
        }
    };
    let said = reader.join().unwrap_or_default().trim().to_string();
    Ok(classify_fetch(status.map(|s| s.success()), &said, timeout))
}

/// What a fetch's exit and its words mean: `None` is a fetch the deadline ended.
fn classify_fetch(
    succeeded: Option<bool>,
    said: &str,
    timeout: Duration,
) -> std::result::Result<(), FetchFailure> {
    match succeeded {
        Some(true) => Ok(()),
        None => Err(FetchFailure::Unreachable(format!(
            "no answer within {}s",
            timeout.as_secs()
        ))),
        Some(false) if said.contains("couldn't find remote ref") => {
            Err(FetchFailure::NoSuchRemoteRef)
        }
        Some(false) => Err(FetchFailure::Unreachable(
            said.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("git fetch failed and said nothing")
                .to_string(),
        )),
    }
}

/// How many commits `to` has that `from` lacks, when both resolve.
pub fn commits_between(cwd: &Path, from: &str, to: &str) -> Result<Option<u64>> {
    let out = try_run(
        cwd,
        &[
            "rev-list",
            "--count",
            "--end-of-options",
            &format!("{from}..{to}"),
        ],
    )?;
    if out.status != Some(0) {
        return Ok(None);
    }
    Ok(out.text()?.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fetch_is_read_as_arrived_absent_or_unreachable() {
        let t = Duration::from_secs(7);
        assert_eq!(classify_fetch(Some(true), "", t), Ok(()));
        assert_eq!(
            classify_fetch(
                Some(false),
                "fatal: couldn't find remote ref refs/heads/feature/x",
                t
            ),
            Err(FetchFailure::NoSuchRemoteRef)
        );
        assert_eq!(
            classify_fetch(
                Some(false),
                "\nfatal: '/nowhere' does not appear to be a git repository\nmore",
                t
            ),
            Err(FetchFailure::Unreachable(
                "fatal: '/nowhere' does not appear to be a git repository".into()
            ))
        );
        assert_eq!(
            classify_fetch(None, "", t),
            Err(FetchFailure::Unreachable("no answer within 7s".into()))
        );
        assert_eq!(
            classify_fetch(Some(false), "", t),
            Err(FetchFailure::Unreachable(
                "git fetch failed and said nothing".into()
            ))
        );
    }

    #[test]
    fn a_fetch_from_a_remote_that_is_not_there_is_unreachable_and_quick() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-q"])
            .status()
            .expect("git runs")
            .success();
        assert!(ok);
        let nowhere = dir.path().join("no-such-remote.git");
        let started = Instant::now();
        let got = fetch_bounded(
            dir.path(),
            &nowhere.to_string_lossy(),
            &["+refs/heads/master:refs/remotes/x/master".to_string()],
            Duration::from_secs(30),
        )
        .expect("git runs");
        assert!(matches!(got, Err(FetchFailure::Unreachable(_))), "{got:?}");
        assert!(started.elapsed() < Duration::from_secs(30));
    }
}
