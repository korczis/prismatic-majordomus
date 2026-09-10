//! The one place worktree management runs `git`.
//!
//! Arguments are passed as arguments — never assembled into a shell string — so a path with
//! a space, a quote, a newline or a leading hyphen is passed to git exactly as it is. Every
//! run captures status, stdout and stderr and turns a failure into a typed
//! [`WorktreeError::GitCommandFailed`] carrying git's own words; nothing here parses
//! human-oriented output, and nothing here reaches the network.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

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
    // Never let a user's environment change what these commands mean.
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
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

/// Is this branch name one git will accept for `refs/heads/`?  Asked of git rather than
/// re-implemented: git's rules for a reference name are git's, they are not short, and a
/// second copy of them here would be wrong in some corner nobody would find until it
/// created a branch git could not read.
pub fn branch_name_is_valid(cwd: &Path, branch: &str) -> Result<bool> {
    // The full reference is checked rather than `--branch`, for two reasons. `--branch`
    // also expands shorthands like `@{-1}`, which is a different question from "is this a
    // valid name", and it takes no `--end-of-options`, so a branch beginning with a hyphen
    // would be read as an option. Prefixing `refs/heads/` makes the argument never start
    // with a hyphen and asks exactly the question that matters.
    let out = try_run(cwd, &["check-ref-format", &format!("refs/heads/{branch}")])?;
    Ok(out.status == Some(0))
}

/// The porcelain status of a work tree: what `git status --porcelain` printed, unparsed.
/// Empty means clean, tracked and untracked alike.
pub fn status_porcelain(worktree: &Path) -> Result<String> {
    let out = run(
        worktree,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
