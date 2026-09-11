//! What git says about the state of a work tree and of every branch: uncommitted work from
//! `status --porcelain`, and every local branch with its upstream, its distance from it and
//! the work tree holding it from one `for-each-ref`. Nothing here fetches.
//!
//! The split that matters is between asking git and reading its answer. Each subprocess is
//! run once, by one function, and handed to a parser that is pure: the parsers are where
//! every decision about what an entry means lives, so those decisions are provable without
//! a repository, and a change to them cannot be masked by whatever the machine's checkout
//! happens to contain.
//!
//! ```
//! use majordomus_cli::worktree::state::{parse_branches, parse_status_z};
//!
//! // what `status --porcelain -z --untracked-files=all --no-renames` answered
//! let dirty = parse_status_z(b"M  staged\0 M edited\0?? new\0");
//! assert_eq!((dirty.staged, dirty.unstaged, dirty.untracked), (1, 1, 1));
//! assert!(!dirty.clean, "any of the three is enough to refuse a move");
//!
//! // and what `for-each-ref` answered about the branch that lives in a worktree
//! let refs = parse_branches("feature/x\0abc\0origin/feature/x\0ahead 2\0/a/foo-wt/feature/x\0\n");
//! assert_eq!(refs[0].worktree.as_deref(), Some(std::path::Path::new("/a/foo-wt/feature/x")));
//! assert_eq!(refs[0].upstream.as_ref().unwrap().ahead, Some(2));
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::error::Result;
use super::git;
use super::model::{DirtyState, UpstreamState};

/// Count the entries of `git status --porcelain -z` for a work tree. Untracked files are
/// counted as files (`--untracked-files=all`), because a directory of untracked work is the
/// thing a move must not lose and "1 untracked" for a directory of forty files would
/// understate it.
///
/// Renames are disabled so that one change is one entry whichever way git chooses to
/// describe it, and the operation in progress is folded in here rather than left for a
/// caller to ask about separately: a worktree in the middle of a rebase is not clean even
/// when its counts are all zero, and every caller that gates on cleanliness needs both.
///
/// ```no_run
/// use majordomus_cli::worktree::state::dirty_state;
/// use std::path::Path;
/// let d = dirty_state(Path::new("/a/foo-wt/feature/x")).unwrap();
/// let counted = d.staged + d.unstaged + d.untracked + d.conflicted;
/// assert_eq!(d.clean, counted == 0, "`clean` is the counts and nothing else");
/// ```
pub fn dirty_state(worktree: &Path) -> Result<DirtyState> {
    let bytes = git::status_porcelain_z(worktree, "all")?;
    let mut state = parse_status_z(&bytes);
    state.in_progress = operation_in_progress(worktree);
    Ok(state)
}

/// Parse the NUL-separated porcelain v1 status. Renames are disabled by the caller
/// (`--no-renames`), so every entry is `XY path\0` and no entry is followed by a second
/// path.
///
/// ```
/// use majordomus_cli::worktree::state::parse_status_z;
/// let s = parse_status_z(b"M  a\0 M b\0?? c\0UU d\0MM e\0");
/// assert_eq!((s.staged, s.unstaged, s.untracked, s.conflicted), (2, 2, 1, 1));
/// assert!(!s.clean);
/// assert!(parse_status_z(b"").clean);
/// ```
pub fn parse_status_z(bytes: &[u8]) -> DirtyState {
    let mut state = DirtyState::default();
    for entry in bytes.split(|b| *b == 0).filter(|e| e.len() >= 3) {
        let x = entry[0] as char;
        let y = entry[1] as char;
        match (x, y) {
            ('?', '?') => state.untracked += 1,
            ('!', '!') => {}
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => state.conflicted += 1,
            _ => {
                if x != ' ' {
                    state.staged += 1;
                }
                if y != ' ' {
                    state.unstaged += 1;
                }
            }
        }
    }
    state.clean =
        state.staged == 0 && state.unstaged == 0 && state.untracked == 0 && state.conflicted == 0;
    state
}

/// The operation git is in the middle of in this work tree, if any: what `git status`
/// would report as "rebase in progress" and so on, read from the per-worktree git
/// directory the way git itself does.
///
/// The vocabulary is closed — `rebase`, `merge`, `cherry-pick`, `revert`, `bisect` — and
/// `None` means both "nothing in progress" and "this is not a work tree git will talk
/// about", because neither is a reason to stop describing a topology. It is a directory
/// probe rather than a `status` call: the answer is wanted for every worktree at once, and
/// a subprocess per worktree is what makes a topology read expensive.
///
/// ```no_run
/// use majordomus_cli::worktree::state::operation_in_progress;
/// use std::path::Path;
/// let op = operation_in_progress(Path::new("/a/foo-wt/feature/x"));
/// assert!(
///     matches!(
///         op.as_deref(),
///         None | Some("rebase") | Some("merge") | Some("cherry-pick") | Some("revert")
///             | Some("bisect")
///     ),
///     "the answer is one of git's operations or nothing: {op:?}"
/// );
/// ```
pub fn operation_in_progress(worktree: &Path) -> Option<String> {
    let out = git::try_run(worktree, &["rev-parse", "--absolute-git-dir"]).ok()?;
    if out.status != Some(0) {
        return None;
    }
    let dir = PathBuf::from(out.text().ok()?);
    let probes: &[(&str, &str)] = &[
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ];
    probes
        .iter()
        .find(|(file, _)| dir.join(file).exists())
        .map(|(_, op)| (*op).to_string())
}

/// One local branch as `for-each-ref` reports it.
///
/// The two optional fields are the interesting ones, and both are absent for a reason a
/// caller has to keep apart from zero: no `upstream` means the branch was never pushed and
/// not that it is up to date, and no `worktree` means no checkout holds it and not that
/// its checkout is the primary. Those are the two facts the guard and the migration plan
/// are built on, so neither is defaulted here.
///
/// ```
/// use majordomus_cli::worktree::state::{parse_branches, BranchRef};
/// let refs: Vec<BranchRef> = parse_branches("main\0def\0\0\0/a/foo\0\n");
/// assert_eq!(refs[0].name, "main");
/// assert!(refs[0].upstream.is_none(), "an unpushed branch has no distance, not zero");
/// assert_eq!(refs[0].worktree.as_deref(), Some(std::path::Path::new("/a/foo")));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRef {
    /// Short name.
    pub name: String,
    /// The commit.
    pub head: String,
    /// The upstream, when configured.
    pub upstream: Option<UpstreamState>,
    /// The work tree holding it, when one does.
    pub worktree: Option<PathBuf>,
}

/// Every local branch, in one subprocess: name, commit, upstream with its ahead/behind
/// distance, and the work tree holding it.
///
/// One call for the whole repository, not one per branch. A topology read describes every
/// branch and every worktree at once, and asking git per branch is what turns that read
/// from milliseconds into seconds on a repository with a few dozen of them. The parsing is
/// [`parse_branches`], which is where the format is interpreted and proved.
///
/// ```no_run
/// use majordomus_cli::worktree::state::branches;
/// use std::path::Path;
/// let all = branches(Path::new("/a/foo")).unwrap();
/// assert!(
///     all.iter().all(|b| !b.name.is_empty() && !b.head.is_empty()),
///     "every branch git listed has a short name and a commit"
/// );
/// ```
pub fn branches(primary: &Path) -> Result<Vec<BranchRef>> {
    let out = git::run(
        primary,
        &[
            "for-each-ref",
            "refs/heads",
            "--format=%(refname:short)%00%(objectname)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(worktreepath)%00",
        ],
    )?;
    Ok(parse_branches(&String::from_utf8_lossy(&out.stdout)))
}

/// Parse the format above: five NUL-terminated fields per branch, one branch per line.
///
/// ```
/// use majordomus_cli::worktree::state::parse_branches;
/// let b = parse_branches("feature/x\0abc\0origin/feature/x\0ahead 2, behind 1\0/a/foo-wt/feature/x\0\nmain\0def\0\0\0/a/foo\0\n");
/// assert_eq!(b.len(), 2);
/// let u = b[0].upstream.as_ref().unwrap();
/// assert_eq!((u.ahead, u.behind, u.gone), (Some(2), Some(1), false));
/// assert!(b[1].upstream.is_none());
/// ```
pub fn parse_branches(text: &str) -> Vec<BranchRef> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        let fields: Vec<&str> = line.split('\0').collect();
        if fields.len() < 5 || fields[0].is_empty() {
            continue;
        }
        let upstream = if fields[2].is_empty() {
            None
        } else {
            let track = fields[3].trim();
            let mut ahead = None;
            let mut behind = None;
            let mut gone = false;
            for part in track.split(',').map(str::trim) {
                if part == "gone" {
                    gone = true;
                } else if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.trim().parse().ok();
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.trim().parse().ok();
                }
            }
            if !gone {
                ahead = ahead.or(Some(0));
                behind = behind.or(Some(0));
            }
            Some(UpstreamState {
                name: fields[2].to_string(),
                ahead,
                behind,
                gone,
            })
        };
        out.push(BranchRef {
            name: fields[0].to_string(),
            head: fields[1].to_string(),
            upstream,
            worktree: if fields[4].is_empty() {
                None
            } else {
                Some(PathBuf::from(fields[4]))
            },
        });
    }
    out
}

/// Every local branch reachable from `trunk`, in one subprocess.
///
/// Reachability, not integration: a branch is in this set when its tip is an ancestor of
/// the trunk, which is what makes a worktree safe to remove without losing commits. It
/// says nothing about whether the branch was merged, squashed or cherry-picked, and it is
/// deliberately a local question — nothing here fetches, so the answer is about the trunk
/// this checkout has and a caller that cares about the remote must update it first.
///
/// ```no_run
/// use majordomus_cli::worktree::state::merged_into;
/// use std::path::Path;
/// let merged = merged_into(Path::new("/a/foo"), "master").unwrap();
/// assert!(merged.contains("master"), "the trunk is reachable from itself");
/// ```
pub fn merged_into(primary: &Path, trunk: &str) -> Result<BTreeSet<String>> {
    let out = git::run(
        primary,
        &[
            "branch",
            "--list",
            "--merged",
            trunk,
            "--format=%(refname:short)",
        ],
    )?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// The issue ids this repository's project model declares: the file names under
/// `.ai/repo/project/issues/`, read from the primary checkout. A filesystem read, not an
/// index build, so the topology stays cheap.
///
/// A repository with no project model answers with an empty list rather than an error: the
/// worktree topology is defined for any git repository, and knowing which issue a branch
/// names is an enrichment on top of it. The `README` in that directory is not an issue and
/// is dropped by name, case-insensitively.
///
/// ```
/// use majordomus_cli::worktree::state::issue_ids;
/// let dir = tempfile::tempdir().unwrap();
/// assert!(issue_ids(dir.path()).is_empty(), "no project model is not a failure");
///
/// let issues = dir.path().join(".ai/repo/project/issues");
/// std::fs::create_dir_all(&issues).unwrap();
/// std::fs::write(issues.join("I0042.yaml"), "").unwrap();
/// std::fs::write(issues.join("README.yaml"), "").unwrap();
/// assert_eq!(issue_ids(dir.path()), ["I0042"], "the README is not an issue id");
/// ```
pub fn issue_ids(primary: &Path) -> Vec<String> {
    let dir = primary.join(".ai/repo/project/issues");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".yaml").map(str::to_string)
        })
        .filter(|id| !id.eq_ignore_ascii_case("readme"))
        .collect();
    ids.sort();
    ids
}

/// The issue a branch provably names: a path component equal to an issue id, or beginning
/// with the id followed by `-`. Nothing else counts.
///
/// ```
/// use majordomus_cli::worktree::state::issue_of;
/// let ids = vec!["I0042".to_string(), "I1003".to_string()];
/// assert_eq!(issue_of("feature/I1003-graph-kinds", &ids).as_deref(), Some("I1003"));
/// assert_eq!(issue_of("fix/I0042", &ids).as_deref(), Some("I0042"));
/// assert_eq!(issue_of("feature/graph-kinds", &ids), None);
/// assert_eq!(issue_of("feature/I10030", &ids), None);
/// ```
pub fn issue_of(branch: &str, ids: &[String]) -> Option<String> {
    for component in branch.split('/') {
        for id in ids {
            if component == id
                || component
                    .strip_prefix(id.as_str())
                    .is_some_and(|rest| rest.starts_with('-'))
            {
                return Some(id.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gone_upstream_has_no_distance() {
        let b = parse_branches("x\0abc\0origin/x\0gone\0\0\n");
        let u = b[0].upstream.as_ref().unwrap();
        assert!(u.gone);
        assert_eq!(u.ahead, None);
    }

    #[test]
    fn an_up_to_date_upstream_is_zero_and_zero_not_unknown() {
        let b = parse_branches("x\0abc\0origin/x\0\0\0\n");
        let u = b[0].upstream.as_ref().unwrap();
        assert_eq!((u.ahead, u.behind), (Some(0), Some(0)));
    }

    #[test]
    fn status_counts_each_column_once() {
        let s = parse_status_z(b"A  new\0D  gone\0 D removed\0?? x/y\0");
        assert_eq!((s.staged, s.unstaged, s.untracked), (2, 1, 1));
    }
}
