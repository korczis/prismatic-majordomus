//! What git says about the state of a work tree and of every branch: uncommitted work from
//! `status --porcelain`, and every local branch with its upstream, its distance from it and
//! the work tree holding it from one `for-each-ref`. Nothing here fetches.
//!
//! Reading the uncommitted work of *one* work tree is one subprocess; reading it for every
//! work tree of a repository is one subprocess per work tree, and this machine's repository
//! has over a hundred of them. [`dirty_states`] is the only caller that matters and it runs
//! them concurrently, because the cost is git walking a hundred separate working trees on
//! disk and not this process computing anything: the work is independent per tree, it is
//! I/O-bound, and running it one tree at a time made a page that has to answer a person
//! wait for the sum of a hundred waits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::error::Result;
use super::git;
use super::model::{DirtyState, UpstreamState};

/// Count the entries of `git status --porcelain -z` for a work tree. Untracked files are
/// counted as files (`--untracked-files=all`), because a directory of untracked work is the
/// thing a move must not lose and "1 untracked" for a directory of forty files would
/// understate it.
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
pub fn operation_in_progress(worktree: &Path) -> Option<String> {
    let dir = git_dir_of(worktree)?;
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

/// The git directory of a work tree, read the way git lays it out rather than by asking
/// git. The primary checkout has a `.git` directory; a linked work tree has a `.git` *file*
/// holding one `gitdir: <path>` line, which is git's own on-disk contract and is what
/// `rev-parse --absolute-git-dir` would answer.
///
/// It is read here rather than asked because asking costs a subprocess per work tree, and
/// [`dirty_states`] asks it once for every work tree of the repository. A relative `gitdir:`
/// is resolved against the work tree, which is how git writes it when the two are moved
/// together.
///
/// ```
/// use majordomus_cli::worktree::state::git_dir_of;
/// let tmp = tempfile::tempdir().unwrap();
/// std::fs::create_dir(tmp.path().join(".git")).unwrap();
/// assert_eq!(git_dir_of(tmp.path()), Some(tmp.path().join(".git")));
/// ```
pub fn git_dir_of(worktree: &Path) -> Option<PathBuf> {
    let entry = worktree.join(".git");
    if entry.is_dir() {
        return Some(entry);
    }
    let text = std::fs::read_to_string(&entry).ok()?;
    let target = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
    let path = PathBuf::from(target);
    Some(if path.is_absolute() {
        path
    } else {
        worktree.join(path)
    })
}

/// How many work trees are read at once.
///
/// The unit of work is a `git status` subprocess walking one working tree on disk: it is
/// bound by the filesystem and by git's own process, not by this process's CPU, so the
/// useful degree is higher than the core count and is capped rather than computed. Eight
/// per core keeps a hundred trees in flight on any machine this runs on; the ceiling is
/// there because a hundred simultaneous `git status` processes is a load spike on a machine
/// several sessions share, and `1` when the parallelism cannot be read is a working answer
/// and not a failure.
fn read_concurrency(jobs: usize) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    jobs.clamp(1, (cores * 8).clamp(4, 32))
}

/// The uncommitted work of every named work tree, read concurrently.
///
/// One entry per path that answered; a work tree git refused to report on is absent rather
/// than reported clean, because "no answer" and "nothing to report" are different facts and
/// a topology that conflated them would call a broken checkout tidy.
pub fn dirty_states(worktrees: &[PathBuf]) -> BTreeMap<PathBuf, DirtyState> {
    if worktrees.is_empty() {
        return BTreeMap::new();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(BTreeMap::new());
    let threads = read_concurrency(worktrees.len());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(path) = worktrees.get(i) else { return };
                if let Ok(state) = dirty_state(path) {
                    if let Ok(mut map) = out.lock() {
                        map.insert(path.clone(), state);
                    }
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

/// One local branch as `for-each-ref` reports it.
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
