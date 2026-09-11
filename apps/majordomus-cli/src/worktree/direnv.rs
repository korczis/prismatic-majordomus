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
///
/// Six outcomes and no boolean, because "not approved" is four different situations with
/// four different next actions: there is no `.envrc`, direnv is not installed, the file
/// differs from the one the person approved, or the primary checkout's own file was never
/// approved either. Collapsing them would leave a person with a worktree that does not
/// load its environment and no way to tell which of the four to fix.
///
/// None of them is a failure of the operation that produced them. A blocked worktree is
/// still a worktree, and the value exists so that the report says so before the next `cd`
/// does.
///
/// ```
/// use majordomus_cli::worktree::EnvrcApproval;
/// // the outcomes a person has to act on say what the action is
/// assert!(EnvrcApproval::Differs.describe().contains("direnv allow"));
/// assert!(EnvrcApproval::NotApprovedInPrimary.describe().contains("direnv allow"));
/// // and the outcomes that need nothing ask for nothing
/// assert!(!EnvrcApproval::Approved.describe().contains("allow"));
/// assert!(!EnvrcApproval::NoEnvrc.describe().contains("allow"));
/// ```
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
    ///
    /// Every line begins with the same `envrc` label and the same padding, so a report
    /// listing several worktrees keeps one column and a reader scans it rather than
    /// parsing it. The line is the whole message: nothing above this adds to it, which is
    /// why the ones a person must act on carry the command instead of implying it.
    ///
    /// ```
    /// use majordomus_cli::worktree::EnvrcApproval;
    /// let failed = EnvrcApproval::Failed { message: "cannot write .direnv".into() };
    /// assert_eq!(failed.describe(), "envrc   direnv refused: cannot write .direnv");
    /// assert!(EnvrcApproval::DirenvAbsent.describe().starts_with("envrc   "));
    /// ```
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
        (script, record)
    }

    fn checkout(root: &Path, name: &str, envrc: Option<&str>) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(text) = envrc {
            std::fs::write(dir.join(".envrc"), text).unwrap();
        }
        dir
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
