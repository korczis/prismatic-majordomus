//! The one place the worktree topology runs `git`.
//!
//! Arguments are passed as arguments — never assembled into a shell string — so a path with
//! a space, a quote, a newline or a leading hyphen is passed to git exactly as it is. Every
//! path handed to a `git worktree` subcommand is absolute: git resolves a relative path
//! against its own working directory and matches a bare name against the *suffix* of any
//! registered worktree, and either of those is how a worktree ends up inside the primary
//! checkout. Every run captures status, stdout and stderr and turns a failure into a typed
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository with one commit on `master`, built with `git` itself.
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "-q", "-b", "master", "."]).unwrap();
        run(
            dir.path(),
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        )
        .unwrap();
        dir
    }

    /// Every path this tool prints is copied into a command, so it is text; a byte sequence
    /// that is not UTF-8 is refused by name rather than rendered lossily into a path that
    /// looks plausible and does not exist. The trailing newline git writes is git's, not
    /// part of the answer.
    #[test]
    fn output_text_trims_the_newline_and_refuses_bytes_it_cannot_name() {
        let out = Output {
            stdout: b"/a/foo\n".to_vec(),
            stderr: String::new(),
            status: Some(0),
        };
        assert_eq!(out.text().unwrap(), "/a/foo");
        let crlf = Output {
            stdout: b"/a/foo\r\n".to_vec(),
            stderr: String::new(),
            status: Some(0),
        };
        assert_eq!(crlf.text().unwrap(), "/a/foo");

        let invalid = Output {
            stdout: vec![b'/', 0xff, 0xfe],
            stderr: String::new(),
            status: Some(0),
        };
        let e = invalid.text().unwrap_err();
        assert_eq!(e.code(), "GitUnavailable");
        assert!(
            !e.to_string().contains('\u{fffd}'),
            "a lossy replacement character in the message would look like a real path: {e}"
        );
    }

    /// `run` refuses anything but success and carries git's own words out. Losing the
    /// stderr would leave a person with "git failed" and no reason; losing the arguments
    /// would leave them unable to reproduce it.
    #[test]
    fn a_failing_git_becomes_a_typed_error_carrying_gits_own_words() {
        let dir = repo();
        let e = run(dir.path(), &["rev-parse", "--verify", "no-such-ref"]).unwrap_err();
        assert_eq!(e.code(), "GitCommandFailed");
        let msg = e.to_string();
        assert!(msg.contains("rev-parse --verify no-such-ref"), "{msg}");
        assert!(
            msg.contains("no-such-ref"),
            "git's own stderr is what says why: {msg}"
        );

        // The same call through `try_run` is an answer, not a failure: the caller decides.
        let out = try_run(dir.path(), &["rev-parse", "--verify", "no-such-ref"]).unwrap();
        assert_ne!(out.status, Some(0));
    }

    /// Nothing in the worktree topology reaches the network. A ref that does not resolve
    /// locally is "no" — never "let me look upstream" — because `worktree create` runs on a
    /// laptop with no connection and a fetch there would hang rather than answer.
    #[test]
    fn a_ref_that_does_not_resolve_locally_is_answered_no_without_a_fetch() {
        let dir = repo();
        assert!(rev_exists(dir.path(), "HEAD").unwrap());
        assert!(rev_exists(dir.path(), "master").unwrap());
        assert!(!rev_exists(dir.path(), "origin/does-not-exist").unwrap());
        assert!(!rev_exists(dir.path(), "v9.9.9").unwrap());
        assert!(
            !rev_exists(dir.path(), "--upload-pack=touch /tmp/pwned").unwrap(),
            "a ref beginning with a hyphen is a ref, not an option to git"
        );

        assert!(branch_exists(dir.path(), "master").unwrap());
        assert!(!branch_exists(dir.path(), "feature/nope").unwrap());
        assert!(
            !branch_exists(dir.path(), "-x").unwrap(),
            "the name is prefixed with refs/heads/, so it never starts a hyphen"
        );
    }

    /// Git is the authority on a branch name at the moment a branch is created, and
    /// [`super::path::validate`] carries the same rules offline so a path can be derived
    /// without a subprocess. This is the check that the two agree in the direction that
    /// matters: nothing this tool accepts may be something git refuses.
    #[test]
    fn nothing_the_offline_rules_accept_is_a_name_git_refuses() {
        let dir = repo();
        for name in [
            "master",
            "feature/x",
            "feature/providers/openai-streaming",
            "fix/I0931-something",
            "žluťoučký/kůň",
        ] {
            assert!(
                super::super::path::validate(name).is_ok(),
                "{name} was refused offline"
            );
            assert!(
                branch_name_is_valid(dir.path(), name).unwrap(),
                "{name} is accepted offline and refused by git"
            );
        }
        for name in ["a b", "a..b", "a~b", ".hidden", "a.lock", "a/"] {
            assert!(
                super::super::path::validate(name).is_err(),
                "{name} was accepted offline"
            );
            assert!(
                !branch_name_is_valid(dir.path(), name).unwrap(),
                "{name} was refused offline and git accepts it"
            );
        }
        // The direction that does *not* have to match: the offline rules are allowed to be
        // stricter. `-x` is a well-formed reference name, and this tool still refuses it,
        // because the danger is not the ref format — it is the name arriving at a command
        // line as an option. Loosening this to "whatever git accepts" would let a branch
        // name become an argument to git.
        assert!(
            branch_name_is_valid(dir.path(), "-x").unwrap(),
            "git's reference format accepts a leading hyphen"
        );
        assert!(
            super::super::path::validate("-x").is_err(),
            "and this tool refuses it anyway"
        );
    }

    /// A repository with no commit yet has no HEAD to name, and a detached one has no
    /// branch. Both are answered `None` rather than as failures: an unborn repository is a
    /// state the topology reports, not an error it refuses.
    #[test]
    fn an_unborn_head_and_a_detached_head_are_absences_rather_than_failures() {
        let empty = tempfile::tempdir().unwrap();
        run(empty.path(), &["init", "-q", "-b", "master", "."]).unwrap();
        assert_eq!(head_of(empty.path()).unwrap(), None);
        assert_eq!(
            branch_of(empty.path()).unwrap(),
            Some("master".into()),
            "an unborn branch is still the branch HEAD points at"
        );

        let dir = repo();
        let head = head_of(dir.path()).unwrap().expect("a commit");
        assert_eq!(head.len(), 40, "the full object name: {head}");
        assert_eq!(branch_of(dir.path()).unwrap(), Some("master".into()));
        run(dir.path(), &["checkout", "-q", "--detach", "HEAD"]).unwrap();
        assert_eq!(branch_of(dir.path()).unwrap(), None);
        assert_eq!(
            head_of(dir.path()).unwrap(),
            Some(head),
            "detaching moves no commit"
        );
    }

    /// The status is read raw and parsed elsewhere, and untracked content is the thing a
    /// careless move loses, so it is counted as files rather than folded into a directory.
    /// `--no-renames` is what makes every entry one record with one path.
    #[test]
    fn the_porcelain_status_is_empty_for_a_clean_tree_and_lists_untracked_files_one_by_one() {
        let dir = repo();
        assert!(status_porcelain_z(dir.path(), "all").unwrap().is_empty());

        std::fs::create_dir_all(dir.path().join("new/deep")).unwrap();
        std::fs::write(dir.path().join("new/deep/a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("new/deep/b.txt"), "b").unwrap();
        let all = status_porcelain_z(dir.path(), "all").unwrap();
        let entries: Vec<&[u8]> = all.split(|b| *b == 0).filter(|e| !e.is_empty()).collect();
        assert_eq!(
            entries.len(),
            2,
            "a directory of untracked work counted as one entry would understate what a move loses"
        );

        let normal = status_porcelain_z(dir.path(), "normal").unwrap();
        let entries: Vec<&[u8]> = normal
            .split(|b| *b == 0)
            .filter(|e| !e.is_empty())
            .collect();
        assert_eq!(entries.len(), 1, "which is what `normal` would have said");
    }
}
