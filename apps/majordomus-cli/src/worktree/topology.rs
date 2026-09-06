//! `git worktree list --porcelain`, parsed once into typed records.
//!
//! The porcelain format is the machine format: one record per work tree, one attribute per
//! line, records separated by a blank line, and no colour, no localisation and no column
//! alignment in it. Everything downstream — the standing of each worktree, the guard, the
//! migration plan — reads these records; nothing anywhere else runs `git worktree list` or
//! looks at its human output.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::{Result, WorktreeError};
use super::git;

/// One work tree as git registered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeRecord {
    /// The absolute path git holds for it. It may not exist on disk any more; that is what
    /// `prunable` reports.
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit checked out, when there is one.
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch, short — `refs/heads/` removed. Absent when detached or bare.
    pub branch: Option<String>,
    /// The bare repository itself, which `git worktree list` reports as the first record of
    /// a bare repository.
    pub bare: bool,
    /// HEAD is detached here.
    pub detached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Locked, with the reason git recorded — an empty string when it recorded none.
    pub locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Prunable, with git's reason (the work tree is gone, usually).
    pub prunable: Option<String>,
}

/// Parse porcelain records from attribute lines. The separator is the caller's: `\n` for
/// `--porcelain`, `\0` for `--porcelain -z`. A blank attribute ends a record.
///
/// Unknown attributes are ignored rather than refused: git adds them over time, and a new
/// attribute is not a reason to stop answering questions about the ones that exist.
///
/// ```
/// use majordomus_cli::worktree::parse_porcelain;
/// let text = "worktree /a/foo\nHEAD abc\nbranch refs/heads/main\n\n\
///             worktree /a/foo-wt/x\nHEAD def\ndetached\nlocked in use\n\n";
/// let r = parse_porcelain(text);
/// assert_eq!(r.len(), 2);
/// assert_eq!(r[0].branch.as_deref(), Some("main"));
/// assert!(r[1].detached);
/// assert_eq!(r[1].locked.as_deref(), Some("in use"));
/// ```
pub fn parse_porcelain(text: &str) -> Vec<WorktreeRecord> {
    parse_lines(text.split('\n'))
}

/// The same, from the NUL-separated form, where a path may contain a newline.
pub fn parse_porcelain_nul(bytes: &[u8]) -> Vec<WorktreeRecord> {
    let text = String::from_utf8_lossy(bytes);
    parse_lines(text.split('\0'))
}

fn parse_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<WorktreeRecord> {
    let mut out: Vec<WorktreeRecord> = Vec::new();
    let mut cur: Option<WorktreeRecord> = None;
    for raw in lines {
        // `--porcelain` writes `\n`-terminated lines; under `-z` a stray `\r` cannot appear,
        // and under `--porcelain` on a Windows shell it can.
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            if let Some(r) = cur.take() {
                out.push(r);
            }
            continue;
        }
        let (key, value) = match line.split_once(' ') {
            Some((k, v)) => (k, Some(v)),
            None => (line, None),
        };
        match key {
            "worktree" => {
                if let Some(r) = cur.take() {
                    out.push(r);
                }
                cur = Some(WorktreeRecord {
                    path: PathBuf::from(value.unwrap_or_default()),
                    head: None,
                    branch: None,
                    bare: false,
                    detached: false,
                    locked: None,
                    prunable: None,
                });
            }
            _ => {
                let Some(r) = cur.as_mut() else { continue };
                match key {
                    "HEAD" => r.head = value.map(str::to_string),
                    "branch" => {
                        r.branch =
                            value.map(|v| v.strip_prefix("refs/heads/").unwrap_or(v).to_string())
                    }
                    "bare" => r.bare = true,
                    "detached" => r.detached = true,
                    "locked" => r.locked = Some(value.unwrap_or_default().to_string()),
                    "prunable" => r.prunable = Some(value.unwrap_or_default().to_string()),
                    _ => {}
                }
            }
        }
    }
    if let Some(r) = cur {
        out.push(r);
    }
    out
}

/// Every work tree git has registered for the repository `cwd` is in, read in one
/// subprocess. `-z` is tried first, because it is the only form that survives a path with a
/// newline in it; a git too old to know the option answers the line-separated form.
pub fn read(cwd: &Path) -> Result<Vec<WorktreeRecord>> {
    let z = git::try_run(cwd, &["worktree", "list", "--porcelain", "-z"])?;
    if z.status == Some(0) {
        return Ok(parse_porcelain_nul(&z.stdout));
    }
    let plain = git::run(cwd, &["worktree", "list", "--porcelain"])?;
    Ok(parse_porcelain(&String::from_utf8_lossy(&plain.stdout)))
}

/// The main work tree of the repository: the record git lists first, which is the primary
/// checkout of a non-bare repository. A bare repository lists its bare directory there, and
/// that is refused by name rather than treated as a checkout.
pub fn primary(records: &[WorktreeRecord], git_common_dir: &Path) -> Result<PathBuf> {
    match records.first() {
        Some(r) if r.bare => Err(WorktreeError::BareRepositoryUnsupported {
            git_dir: r.path.clone(),
        }),
        Some(r) => Ok(r.path.clone()),
        None => Err(WorktreeError::CannotDeterminePrimaryWorktree {
            git_common_dir: git_common_dir.to_path_buf(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_without_a_trailing_blank_line_is_still_a_record() {
        let r = parse_porcelain("worktree /a/foo\nHEAD abc\nbranch refs/heads/main");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, PathBuf::from("/a/foo"));
        assert_eq!(r[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn locked_and_prunable_without_a_reason_are_still_locked_and_prunable() {
        let r = parse_porcelain("worktree /a/x\nlocked\nprunable\n\n");
        assert_eq!(r[0].locked.as_deref(), Some(""));
        assert_eq!(r[0].prunable.as_deref(), Some(""));
    }

    #[test]
    fn a_bare_repository_lists_its_git_dir_first_and_is_refused() {
        let r = parse_porcelain("worktree /a/foo.git\nbare\n\nworktree /a/wt\nHEAD abc\n\n");
        assert!(r[0].bare);
        let e = primary(&r, Path::new("/a/foo.git")).unwrap_err();
        assert_eq!(e.code(), "BareRepositoryUnsupported");
    }

    #[test]
    fn no_records_at_all_is_named_rather_than_guessed() {
        let e = primary(&[], Path::new("/a/.git")).unwrap_err();
        assert_eq!(e.code(), "CannotDeterminePrimaryWorktree");
    }

    #[test]
    fn an_unknown_attribute_does_not_stop_the_parse() {
        let r = parse_porcelain(
            "worktree /a/x\nHEAD abc\nsomething-new value\nbranch refs/heads/b\n\n",
        );
        assert_eq!(r[0].branch.as_deref(), Some("b"));
        assert_eq!(r[0].head.as_deref(), Some("abc"));
    }

    #[test]
    fn a_path_with_a_newline_survives_the_nul_form() {
        let bytes = b"worktree /a/we\nird\0HEAD abc\0\0";
        let r = parse_porcelain_nul(bytes);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, PathBuf::from("/a/we\nird"));
    }

    #[test]
    fn a_detached_worktree_carries_no_branch() {
        let r = parse_porcelain("worktree /a/x\nHEAD abc\ndetached\n\n");
        assert!(r[0].detached);
        assert!(r[0].branch.is_none());
    }

    #[test]
    fn a_hierarchical_branch_keeps_its_slashes() {
        let r = parse_porcelain(
            "worktree /a/foo-wt/feature/x/y\nHEAD abc\nbranch refs/heads/feature/x/y\n\n",
        );
        assert_eq!(r[0].branch.as_deref(), Some("feature/x/y"));
    }
}
