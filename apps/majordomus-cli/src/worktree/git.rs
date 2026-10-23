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
/// deadline is killed. The answer is whether the refs arrived and, when not, why; a git
/// that cannot be run did not reach the remote either, and is said as `Unreachable` with
/// the reason, so the caller falls back exactly as it does for a dead network.
pub fn fetch_bounded(
    cwd: &Path,
    remote: &str,
    refspecs: &[String],
    timeout: Duration,
) -> std::result::Result<(), FetchFailure> {
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
    let finished = match wait_bounded(&mut cmd, timeout) {
        Ok(finished) => finished,
        Err(e) => {
            return Err(FetchFailure::Unreachable(format!(
                "git fetch could not be run in {}: {e}",
                cwd.display()
            )))
        }
    };
    // A wait that failed is read as a fetch that failed, in whatever words git left.
    let succeeded = finished.status.map(|s| s.unwrap_or(false));
    classify_fetch(succeeded, finished.stderr.trim(), timeout)
}

/// How long `git worktree add` may take before it is killed. A checkout of this size takes
/// seconds; one still running after this is hung (a filter waiting on input, a filesystem
/// gone away) and holds the worktree lock, and with it every other create, until it ends.
pub const WORKTREE_ADD_TIMEOUT: Duration = Duration::from_secs(600);

/// What a bounded run left behind.
struct Finished {
    /// `None` when the deadline ended it; `Some(None)` when the wait itself failed;
    /// otherwise whether it exited successfully, with its code.
    status: Option<Option<bool>>,
    /// The exit code, when it exited normally.
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

/// Run a prepared git command with no stdin, both outputs captured, and kill it at the
/// deadline. The one wait-and-kill loop of this module: the bounded fetch and the bounded
/// run are this, read two ways.
fn wait_bounded(cmd: &mut Command, timeout: Duration) -> std::io::Result<Finished> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    // Drained beside the wait, so a chatty process cannot fill a pipe and stall itself.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let deadline = Instant::now() + timeout;
    let waited = loop {
        match child.try_wait() {
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(Some(status)) => break Some(Some(status)),
            Err(_) => break Some(None),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&err.join().unwrap_or_default()).to_string();
    Ok(Finished {
        status: waited.map(|s| s.map(|s| s.success())),
        code: waited.flatten().and_then(|s| s.code()),
        stdout,
        stderr,
    })
}

/// Run `git -C <cwd> <args>` as [`run`] does, and refuse it when it has not finished
/// within `timeout`: the process is killed and the refusal says so, with the command a
/// person runs to clear a registration the kill may have left half-made.
pub fn run_bounded<S: AsRef<OsStr>>(cwd: &Path, args: &[S], timeout: Duration) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.arg("-C").arg(cwd);
    for a in args {
        cmd.arg(a);
    }
    let command = args
        .iter()
        .map(|a| a.as_ref().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let finished = wait_bounded(&mut cmd, timeout).map_err(|e| WorktreeError::GitUnavailable {
        reason: format!("{e} (running git in {})", cwd.display()),
    })?;
    let stderr = finished.stderr.trim().to_string();
    match finished.status {
        Some(Some(true)) => Ok(Output {
            stdout: finished.stdout,
            stderr,
            status: finished.code,
        }),
        None => Err(WorktreeError::GitCommandFailed {
            command,
            status: format!("killed after {}s without finishing", timeout.as_secs()),
            stderr: format!(
                "{}; a registration it may have left half-made is cleared by `git worktree prune`",
                if stderr.is_empty() { "git said nothing" } else { stderr.as_str() }
            ),
        }),
        Some(_) => Err(WorktreeError::GitCommandFailed {
            command,
            status: finished
                .code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "terminated by a signal".into()),
            stderr: if stderr.is_empty() {
                "git said nothing".into()
            } else {
                stderr
            },
        }),
    }
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

/// How many commits `to` has that `from` lacks, when both resolve and git can say; `None`
/// when it cannot, which a caller reads as "how far behind is unknown".
pub fn commits_between(cwd: &Path, from: &str, to: &str) -> Option<u64> {
    let range = format!("{from}..{to}");
    try_run(cwd, &["rev-list", "--count", "--end-of-options", &range])
        .ok()
        .filter(|out| out.status == Some(0))
        .and_then(|out| out.text().ok())
        .and_then(|text| text.trim().parse().ok())
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
        );
        assert!(matches!(got, Err(FetchFailure::Unreachable(_))), "{got:?}");
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    fn git_in(cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn a_git_that_cannot_be_run_is_an_unreachable_fetch_and_an_unknown_distance() {
        // A NUL byte cannot be handed to a process: the spawn itself fails.
        let nowhere = Path::new("no\0where");
        let got = fetch_bounded(nowhere, "origin", &[], Duration::from_secs(5));
        match got {
            Err(FetchFailure::Unreachable(why)) => {
                assert!(why.contains("git fetch could not be run"), "{why}")
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(commits_between(nowhere, "a", "b"), None);
    }

    #[test]
    fn a_fetch_still_running_at_the_deadline_is_killed_and_said_so() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        git_in(root, &["init", "-q", "--bare", "slow.git"]);
        git_in(root, &["init", "-q", "-b", "master", "repo"]);
        let repo = root.join("repo");
        let url = root.join("slow.git").to_string_lossy().to_string();
        git_in(&repo, &["config", "remote.slow.url", &url]);
        // The remote side never answers within the deadline.
        git_in(&repo, &["config", "remote.slow.uploadpack", "sleep 2; :"]);
        let got = fetch_bounded(
            &repo,
            "slow",
            &["+refs/heads/master:refs/remotes/slow/master".to_string()],
            Duration::from_millis(200),
        );
        assert_eq!(
            got,
            Err(FetchFailure::Unreachable("no answer within 0s".into()))
        );
    }

    /// The bounded run answers as `run` does when git finishes, and refuses with the
    /// deadline and the remedy when it does not (I4002).
    #[test]
    fn a_bounded_run_answers_refuses_and_is_killed_at_its_deadline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        git_in(repo, &["init", "-q", "-b", "master", "."]);
        let t = Duration::from_secs(30);

        let ok = run_bounded(repo, &["rev-parse", "--is-inside-work-tree"], t).expect("runs");
        assert_eq!(ok.text().expect("text"), "true");
        assert_eq!(ok.status, Some(0));

        match run_bounded(repo, &["rev-parse", "--verify", "no-such-ref"], t) {
            Err(WorktreeError::GitCommandFailed { status, stderr, .. }) => {
                assert_eq!(status, "128");
                assert!(!stderr.is_empty());
            }
            other => panic!("{other:?}"),
        }

        let started = Instant::now();
        let hung = run_bounded(
            repo,
            &["-c", "alias.hang=!sleep 2", "hang"],
            Duration::from_millis(200),
        );
        match hung {
            Err(WorktreeError::GitCommandFailed { status, stderr, .. }) => {
                assert!(status.starts_with("killed after 0s"), "{status}");
                assert!(stderr.contains("git worktree prune"), "{stderr}");
            }
            other => panic!("{other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(20));

        match run_bounded(Path::new("no\0where"), &["status"], t) {
            Err(WorktreeError::GitUnavailable { reason }) => {
                assert!(reason.contains("running git in"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_distance_between_two_refs_is_counted_and_unknown_when_one_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        git_in(repo, &["init", "-q", "-b", "master", "."]);
        git_in(repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
        git_in(repo, &["branch", "old"]);
        git_in(repo, &["commit", "-q", "--allow-empty", "-m", "two"]);
        git_in(repo, &["commit", "-q", "--allow-empty", "-m", "three"]);
        assert_eq!(commits_between(repo, "old", "master"), Some(2));
        assert_eq!(commits_between(repo, "master", "old"), Some(0));
        assert_eq!(
            commits_between(repo, "master", "refs/remotes/x/master"),
            None
        );
    }
}
