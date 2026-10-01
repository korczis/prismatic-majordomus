//! direnv's approval of a worktree's `.envrc`, carried to the path the worktree now has.
//!
//! direnv approves an `.envrc` by path and content: the same file at a new path is a file it
//! has not seen, and it refuses to load it until a person runs `direnv allow` there. A
//! worktree this module creates or moves therefore starts blocked, and the first `cd` into it
//! is answered with `direnv: error .envrc is blocked` — in a repository with sixty linked
//! worktrees, the recurring "direnv does not work again". Measured on 2026-09-09: of the 37
//! worktrees that had an `.envrc`, 36 were blocked and the primary checkout was the one
//! approved.
//!
//! The approval is carried, not granted. The person approved the `.envrc` of the primary
//! checkout; a worktree whose `.envrc` is byte-for-byte that file gets the same approval at
//! its own path, at the moment the path comes into being. A worktree whose `.envrc` differs
//! — an older branch, a branch that is not theirs — is left as direnv leaves it, and the
//! report says so, because approving content the person has not seen is the one thing
//! `direnv allow` exists to prevent. When the primary checkout's own `.envrc` is not
//! approved, nothing is approved anywhere: the person does not load this file, and a
//! worktree is not the place to start.
//!
//! Every outcome is reported and none is fatal. A worktree that exists and is blocked is
//! still a worktree; what this adds is that a person learns it from the report rather than
//! from the next `cd`.

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What became of the `.envrc` of a worktree that was just created, found, or moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum EnvrcApproval {
    /// direnv accepted it: the next `cd` loads the environment.
    Approved,
    /// The worktree has no `.envrc`; there is nothing to approve and nothing is blocked.
    NoEnvrc,
    /// direnv is not on the PATH; nothing was done, and nothing is blocked either, because
    /// nothing would load the file.
    DirenvAbsent,
    /// The worktree's `.envrc` is not the primary checkout's, so the approval given there
    /// does not carry. `direnv allow` in the worktree, after reading it, is the person's.
    Differs,
    /// The primary checkout's `.envrc` is not approved, so there is no approval to carry.
    NotApprovedInPrimary,
    /// direnv was asked and refused, or could not be run; the message is its own.
    Failed {
        /// direnv's standard error, or the error running it.
        message: String,
    },
}

impl EnvrcApproval {
    /// One line for a person, after the path the worktree is at.
    pub fn describe(&self) -> String {
        match self {
            Self::Approved => {
                "envrc   approved for direnv (the primary checkout's, at this path)".into()
            }
            Self::NoEnvrc => "envrc   none; nothing for direnv to load".into(),
            Self::DirenvAbsent => "envrc   direnv is not installed; nothing to approve".into(),
            Self::Differs => {
                "envrc   differs from the primary checkout's and stays blocked; read it, then \
                 `direnv allow` there"
                    .into()
            }
            Self::NotApprovedInPrimary => {
                "envrc   the primary checkout's is not approved, so nothing was approved here; \
                 `direnv allow` in the primary checkout first"
                    .into()
            }
            Self::Failed { message } => format!("envrc   direnv refused: {message}"),
        }
    }
}

/// What became of the primary checkout's machine-local `.envrc.local` for a worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum LocalOverrides {
    /// Linked: the worktree's `.envrc.local` is a symlink to the primary checkout's, so the
    /// person's machine-local exports (keychain-backed secrets among them) load here too.
    Linked,
    /// The worktree already has its own `.envrc.local`; it is left alone.
    OwnKept,
    /// The primary checkout has none; there is nothing to share.
    NonePrimary,
    /// Not linked, because git in the worktree does not ignore `.envrc.local` — a link a
    /// commit could pick up is the one way this could leak.
    NotIgnored,
    /// The link could not be made; the error.
    Failed {
        /// Why.
        message: String,
    },
}

impl LocalOverrides {
    /// One line for a person, after the envrc line.
    pub fn describe(&self) -> String {
        match self {
            Self::Linked => "local   .envrc.local linked to the primary checkout's".into(),
            Self::OwnKept => "local   .envrc.local of its own, kept".into(),
            Self::NonePrimary => "local   no .envrc.local in the primary checkout to share".into(),
            Self::NotIgnored => {
                "local   .envrc.local is not ignored here, so it was not linked".into()
            }
            Self::Failed { message } => format!("local   .envrc.local not linked: {message}"),
        }
    }
}

/// Share the primary checkout's `.envrc.local` with `worktree` as a symlink, so the value
/// lives in one file and a change there reaches every worktree. `.envrc` reads it relative
/// to the directory it is entered from, and the adapter rule keeps that file free of the
/// program a path lookup would need; a linked worktree therefore gets the link when it
/// comes into being. Only when git ignores the name there: a symlink to secrets that a
/// commit could add is refused rather than made.
pub fn link_local(primary: &Path, worktree: &Path) -> LocalOverrides {
    let source = primary.join(".envrc.local");
    let target = worktree.join(".envrc.local");
    if target.symlink_metadata().is_ok() {
        return LocalOverrides::OwnKept;
    }
    if !source.is_file() {
        return LocalOverrides::NonePrimary;
    }
    let ignored = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["check-ignore", "-q", ".envrc.local"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ignored {
        return LocalOverrides::NotIgnored;
    }
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&source, &target);
    #[cfg(not(unix))]
    let made: std::io::Result<()> = Err(std::io::Error::other("symlinks are unix-only here"));
    match made {
        Ok(()) => LocalOverrides::Linked,
        Err(e) => LocalOverrides::Failed {
            message: e.to_string(),
        },
    }
}

/// Carry the primary checkout's approval to the `.envrc` of `worktree`, with the direnv on
/// the PATH. Never fails: the worst outcome is a worktree left as direnv left it, reported.
pub fn approve(primary: &Path, worktree: &Path) -> EnvrcApproval {
    match direnv_on_path() {
        Some(direnv) => approve_with(&direnv, primary, worktree),
        None => EnvrcApproval::DirenvAbsent,
    }
}

/// The same, with a given direnv executable. What the tests drive, with a direnv of their
/// own making, so the decision is proved without depending on the machine's.
pub fn approve_with(direnv: &Path, primary: &Path, worktree: &Path) -> EnvrcApproval {
    let envrc = worktree.join(".envrc");
    let Ok(here) = std::fs::read(&envrc) else {
        return EnvrcApproval::NoEnvrc;
    };
    let Ok(there) = std::fs::read(primary.join(".envrc")) else {
        return EnvrcApproval::Differs;
    };
    if here != there {
        return EnvrcApproval::Differs;
    }
    match allowed_in(direnv, primary) {
        Some(true) => {}
        Some(false) => return EnvrcApproval::NotApprovedInPrimary,
        None => {
            return EnvrcApproval::Failed {
                message: "`direnv status` in the primary checkout did not say whether its \
                          .envrc is allowed"
                    .into(),
            }
        }
    }
    // The absolute path, and run from the worktree: direnv resolves a bare `allow` against
    // its working directory, and the absolute argument says which file is meant even so.
    let output = Command::new(direnv)
        .arg("allow")
        .arg(&envrc)
        .current_dir(worktree)
        .output();
    match output {
        Ok(o) if o.status.success() => EnvrcApproval::Approved,
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            EnvrcApproval::Failed {
                message: if stderr.is_empty() {
                    format!("exit {}", o.status)
                } else {
                    stderr
                },
            }
        }
        Err(e) => EnvrcApproval::Failed {
            message: e.to_string(),
        },
    }
}

/// Whether direnv has the `.envrc` of `dir` approved, read from `direnv status` run there.
/// `None` when the answer is not in the output: an unknown is never taken for a yes.
fn allowed_in(direnv: &Path, dir: &Path) -> Option<bool> {
    let output = Command::new(direnv)
        .arg("status")
        .current_dir(dir)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    // `Found RC allowed 0` is allowed; 1 is not allowed; 2 is denied. The line is direnv's,
    // and has been since the allow/deny split; anything else is an unknown.
    text.lines()
        .find_map(|l| l.trim().strip_prefix("Found RC allowed "))
        .and_then(|v| v.trim().parse::<u8>().ok())
        .map(|v| v == 0)
}

fn direnv_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join("direnv"))
        .find(|p| p.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A direnv of the test's own: answers `status` with the given allowed value and records
    /// every `allow` it is asked for in a file, one path per line.
    fn fake_direnv(dir: &Path, allowed: u8, allow_exit: i32) -> (PathBuf, PathBuf) {
        let record = dir.join("allowed.log");
        let script = dir.join("direnv");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ncase \"$1\" in\n  status) printf 'Loaded RC allowed 0\\nFound RC allowed {allowed}\\n' ;;\n  allow) printf '%s\\n' \"$2\" >> '{}'; exit {allow_exit} ;;\n  *) exit 2 ;;\nesac\n",
                record.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Linux refuses to exec a file that any process holds open for writing (ETXTBSY). A
        // test thread that forks a child while the write above has the file open leaves that
        // child holding the descriptor until it execs, so the first exec of this fake could
        // fail, and `approve_with` read the failure as "direnv did not say" (master's rust job
        // in run 36557029377). One exec that succeeds proves no writer is left: the write has
        // closed, and no later fork can inherit it.
        wait_until_executable(&script);
        (script, record)
    }

    fn wait_until_executable(script: &Path) {
        for _ in 0..100 {
            match Command::new(script).arg("probe").output() {
                Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                _ => return,
            }
        }
        panic!("{} stayed busy for writing", script.display());
    }

    fn checkout(root: &Path, name: &str, envrc: Option<&str>) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(text) = envrc {
            std::fs::write(dir.join(".envrc"), text).unwrap();
        }
        dir
    }

    fn git_repo(dir: &Path, ignore: &str) {
        std::fs::create_dir_all(dir).unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        std::fs::write(dir.join(".gitignore"), ignore).unwrap();
    }

    #[test]
    fn the_primary_local_overrides_are_linked_only_where_git_ignores_them() {
        let t = tempfile::tempdir().unwrap();
        let primary = t.path().join("primary");
        std::fs::create_dir_all(&primary).unwrap();
        // nothing to share
        let wt = t.path().join("wt-a");
        git_repo(&wt, ".envrc.local\n");
        assert_eq!(link_local(&primary, &wt), LocalOverrides::NonePrimary);
        std::fs::write(primary.join(".envrc.local"), "export A=1\n").unwrap();
        // linked, and the link reads the primary's file
        assert_eq!(link_local(&primary, &wt), LocalOverrides::Linked);
        let link = wt.join(".envrc.local");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "export A=1\n");
        // a second call keeps what is there
        assert_eq!(link_local(&primary, &wt), LocalOverrides::OwnKept);
        // a worktree whose git would track the file gets no link
        let open = t.path().join("wt-b");
        git_repo(&open, "");
        assert_eq!(link_local(&primary, &open), LocalOverrides::NotIgnored);
        assert!(open.join(".envrc.local").symlink_metadata().is_err());
    }

    #[test]
    fn the_primary_checkouts_approval_is_carried_to_an_identical_envrc() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, record) = fake_direnv(tmp.path(), 0, 0);
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/feature/x", Some("PATH_add bin\n"));
        assert_eq!(
            approve_with(&direnv, &primary, &worktree),
            EnvrcApproval::Approved
        );
        let asked = std::fs::read_to_string(record).unwrap();
        assert_eq!(asked.trim(), worktree.join(".envrc").display().to_string());
    }

    #[test]
    fn a_different_envrc_is_left_blocked_and_direnv_is_not_asked() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, record) = fake_direnv(tmp.path(), 0, 0);
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/old", Some("eval \"$(something)\"\n"));
        assert_eq!(
            approve_with(&direnv, &primary, &worktree),
            EnvrcApproval::Differs
        );
        assert!(!record.exists(), "direnv allow must not have been run");
    }

    /// Linux alone refuses to exec a file held open for writing, so this holds there: the
    /// fake is run only once no writer holds it, rather than failing on the first try.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_fake_still_open_for_writing_is_waited_for_before_it_is_run() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, _) = fake_direnv(tmp.path(), 0, 0);
        let held = std::fs::OpenOptions::new()
            .write(true)
            .open(&direnv)
            .unwrap();
        let busy = Command::new(&direnv).arg("probe").output().unwrap_err();
        assert_eq!(busy.raw_os_error(), Some(libc::ETXTBSY), "{busy}");
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            drop(held);
        });
        wait_until_executable(&direnv);
        let after = Command::new(&direnv).arg("probe").output();
        release.join().unwrap();
        assert!(
            after.is_ok(),
            "the fake was handed on while still busy: {after:?}"
        );
    }

    #[test]
    fn nothing_is_approved_when_the_primary_checkout_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, record) = fake_direnv(tmp.path(), 1, 0);
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/feature/x", Some("PATH_add bin\n"));
        assert_eq!(
            approve_with(&direnv, &primary, &worktree),
            EnvrcApproval::NotApprovedInPrimary
        );
        assert!(!record.exists(), "direnv allow must not have been run");
    }

    #[test]
    fn a_worktree_without_an_envrc_has_nothing_to_approve() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, _) = fake_direnv(tmp.path(), 0, 0);
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/bare", None);
        assert_eq!(
            approve_with(&direnv, &primary, &worktree),
            EnvrcApproval::NoEnvrc
        );
    }

    #[test]
    fn a_refusal_by_direnv_is_reported_not_swallowed() {
        let tmp = tempfile::tempdir().unwrap();
        let (direnv, _) = fake_direnv(tmp.path(), 0, 1);
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/feature/x", Some("PATH_add bin\n"));
        match approve_with(&direnv, &primary, &worktree) {
            EnvrcApproval::Failed { message } => assert!(message.contains("exit"), "{message}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_unreadable_status_is_an_unknown_never_a_yes() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("direnv");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let primary = checkout(tmp.path(), "repo", Some("PATH_add bin\n"));
        let worktree = checkout(tmp.path(), "repo-wt/feature/x", Some("PATH_add bin\n"));
        assert!(matches!(
            approve_with(&script, &primary, &worktree),
            EnvrcApproval::Failed { .. }
        ));
    }

    #[test]
    fn every_outcome_describes_itself_on_one_line_starting_with_the_field() {
        for outcome in [
            EnvrcApproval::Approved,
            EnvrcApproval::NoEnvrc,
            EnvrcApproval::DirenvAbsent,
            EnvrcApproval::Differs,
            EnvrcApproval::NotApprovedInPrimary,
            EnvrcApproval::Failed {
                message: "x".into(),
            },
        ] {
            let line = outcome.describe();
            assert!(line.starts_with("envrc   "), "{line}");
            assert!(!line.contains('\n'), "{line}");
        }
    }
}
