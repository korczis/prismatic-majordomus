//! What git says about the state of a work tree and of every branch: uncommitted work from
//! `status --porcelain`, and every local branch with its upstream, its distance from it and
//! the work tree holding it from one `for-each-ref`. Nothing here fetches.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

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

/// The dirty state of many work trees, measured at once.
///
/// Each measurement is a `git status` subprocess of a few tens of milliseconds and none of
/// them depends on another, so they are taken on a small pool of threads rather than one
/// after the next. The measurement that motivated this: on a repository with 128
/// registered work trees the sequence cost 6.2–9.9 s (n=3, shared machine), which was
/// most of what the whole topology cost.
///
/// A `None` in the input means "do not measure this one"; a `None` in the output means it
/// was not measured, or git refused to say. The answers come back in the order the work
/// trees were given, whatever order they finished in, so what this feeds is byte-identical
/// to reading them one at a time.
///
/// ```
/// use majordomus_cli::worktree::state::dirty_states;
/// let repo = tempfile::tempdir().unwrap();
/// let init = std::process::Command::new("git")
///     .args(["init", "-q"])
///     .current_dir(repo.path())
///     .status()
///     .unwrap();
/// assert!(init.success());
/// std::fs::write(repo.path().join("new.txt"), "work").unwrap();
///
/// let states = dirty_states(&[Some(repo.path()), None, Some(repo.path())]);
/// assert_eq!(states.len(), 3);
/// assert!(states[1].is_none(), "a None in is a None out");
/// let first = states[0].as_ref().expect("a git work tree is measured");
/// assert_eq!(first.untracked, 1);
/// assert!(!first.clean);
/// assert_eq!(states[2], states[0], "answers come back in input order");
/// ```
pub fn dirty_states(worktrees: &[Option<&Path>]) -> Vec<Option<DirtyState>> {
    let mut out: Vec<Option<DirtyState>> = vec![None; worktrees.len()];
    let wanted: Vec<usize> = worktrees
        .iter()
        .enumerate()
        .filter_map(|(i, w)| w.map(|_| i))
        .collect();
    // One measurement is not worth a thread, and none is not worth a channel.
    if wanted.len() < 2 {
        for i in wanted {
            out[i] = worktrees[i].and_then(|p| dirty_state(p).ok());
        }
        return out;
    }
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..probe_threads(wanted.len()) {
            let tx = tx.clone();
            let next = &next;
            let wanted = &wanted;
            scope.spawn(move || loop {
                let Some(&i) = wanted.get(next.fetch_add(1, Ordering::Relaxed)) else {
                    return;
                };
                let measured = worktrees[i].and_then(|p| dirty_state(p).ok());
                if tx.send((i, measured)).is_err() {
                    return;
                }
            });
        }
        // The receiver below ends when the last sender is gone, so this one must go first.
        drop(tx);
        for (i, measured) in rx {
            out[i] = measured;
        }
    });
    out
}

/// How many work trees to measure at once. Each measurement waits on a git subprocess
/// rather than on this process's CPU, but the pool is still the machine's parallelism and
/// no more: a development machine runs other people's sessions, and a pool that
/// oversubscribes it makes their commands slower to make this one faster.
fn probe_threads(work: usize) -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16)
        .min(work)
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

/// The git directory of one work tree, read the way git records it rather than by asking
/// git for it: `.git` is that directory in the primary checkout and a file holding
/// `gitdir: <path>` in a linked one. `git rev-parse --absolute-git-dir` answers the same
/// question, but it is a subprocess, and a subprocess is what this costs — 128 of the
/// cheapest possible `git rev-parse` took 1.5–2.2 s on this machine (n=3) — paid once per
/// work tree for a question one `stat` and one small read answer. Git is asked only when
/// `.git` is not there to read, so a checkout arranged some other way still gets an answer.
fn git_dir_of(worktree: &Path) -> Option<PathBuf> {
    let dot = worktree.join(".git");
    match std::fs::metadata(&dot) {
        Ok(m) if m.is_dir() => return Some(dot),
        Ok(_) => {
            if let Some(p) = std::fs::read_to_string(&dot)
                .ok()
                .as_deref()
                .and_then(gitdir_link)
            {
                return Some(if p.is_absolute() { p } else { worktree.join(p) });
            }
        }
        Err(_) => {}
    }
    let out = git::try_run(worktree, &["rev-parse", "--absolute-git-dir"]).ok()?;
    if out.status != Some(0) {
        return None;
    }
    Some(PathBuf::from(out.text().ok()?))
}

/// The path a linked work tree's `.git` file names, when it names one. Git writes exactly
/// one line, `gitdir: <path>`, and the path is normally absolute.
///
/// ```
/// use majordomus_cli::worktree::state::gitdir_link;
/// use std::path::PathBuf;
/// assert_eq!(gitdir_link("gitdir: /a/.git/worktrees/x\n"), Some(PathBuf::from("/a/.git/worktrees/x")));
/// assert_eq!(gitdir_link("gitdir: ../elsewhere"), Some(PathBuf::from("../elsewhere")));
/// assert_eq!(gitdir_link("gitdir:\n"), None);
/// assert_eq!(gitdir_link("something else\n"), None);
/// assert_eq!(gitdir_link(""), None);
/// ```
pub fn gitdir_link(text: &str) -> Option<PathBuf> {
    let rest = text.lines().next()?.strip_prefix("gitdir:")?.trim();
    if rest.is_empty() {
        None
    } else {
        Some(PathBuf::from(rest))
    }
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
    let out = git::run(primary, &["for-each-ref", "refs/heads", BRANCH_FORMAT])?;
    Ok(parse_branches(&String::from_utf8_lossy(&out.stdout)))
}

/// One local branch, read now: the same fields as [`branches`], for a caller that must not act
/// on a reading taken earlier. `None` when the branch no longer exists.
///
/// `for-each-ref refs/heads/<name>` also matches the branches below `<name>/`, so the result is
/// filtered to the exact name.
///
/// ```
/// use majordomus_cli::worktree::state::branch;
/// let repo = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     let ok = std::process::Command::new("git")
///         .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
///         .args(args)
///         .current_dir(repo.path())
///         .status()
///         .unwrap();
///     assert!(ok.success(), "git {args:?}");
/// };
/// git(&["init", "-q", "-b", "trunk"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "root"]);
/// git(&["branch", "feat/sub"]);
///
/// // `refs/heads/feat` matches feat/sub as well; the exact-name filter drops it.
/// assert!(branch(repo.path(), "feat").unwrap().is_none());
/// let feat = branch(repo.path(), "feat/sub").unwrap().expect("feat/sub exists");
/// assert_eq!(feat.name, "feat/sub");
/// assert!(feat.upstream.is_none());
/// assert_eq!(feat.head.len(), 40);
/// assert!(branch(repo.path(), "trunk").unwrap().unwrap().worktree.is_some());
/// assert!(branch(repo.path(), "gone").unwrap().is_none());
/// ```
pub fn branch(primary: &Path, name: &str) -> Result<Option<BranchRef>> {
    let pattern = format!("refs/heads/{name}");
    let out = git::run(primary, &["for-each-ref", pattern.as_str(), BRANCH_FORMAT])?;
    Ok(parse_branches(&String::from_utf8_lossy(&out.stdout))
        .into_iter()
        .find(|b| b.name == name))
}

/// The `for-each-ref` format [`parse_branches`] reads.
const BRANCH_FORMAT: &str = "--format=%(refname:short)%00%(objectname)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(worktreepath)%00";

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

/// Every local branch reachable from `trunk` or from the remote-tracking branch `trunk`
/// follows. A local trunk that has fallen behind its remote — a primary checkout nobody has
/// pulled, because a session holds uncommitted work in it — would otherwise call every branch
/// that landed since unmerged, and cleanup would offer nothing; a branch the remote trunk
/// contains is published and integrated, which is the safety cleanup asks for.
///
/// ```
/// use majordomus_cli::worktree::state::merged_into_trunk;
/// use std::process::Command;
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     let ok = Command::new("git")
///         .arg("-C")
///         .arg(dir.path())
///         .args(args)
///         .env("GIT_AUTHOR_NAME", "t")
///         .env("GIT_AUTHOR_EMAIL", "t@example.com")
///         .env("GIT_COMMITTER_NAME", "t")
///         .env("GIT_COMMITTER_EMAIL", "t@example.com")
///         .status()
///         .unwrap()
///         .success();
///     assert!(ok, "git {args:?}");
/// };
/// git(&["init", "-q", "-b", "main"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "base"]);
/// git(&["branch", "landed"]);
/// // no upstream here: the local trunk alone decides, and it contains `landed`
/// let merged = merged_into_trunk(dir.path(), "main").unwrap();
/// assert!(merged.contains("landed"));
/// ```
pub fn merged_into_trunk(primary: &Path, trunk: &str) -> Result<BTreeSet<String>> {
    let mut merged = merged_into(primary, trunk)?;
    let upstream = format!("{trunk}@{{upstream}}");
    if let Ok(out) = git::run(
        primary,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            &upstream,
        ],
    ) {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            merged.extend(merged_into(primary, &name)?);
        }
    }
    Ok(merged)
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
    crate::order::canonical(&mut ids);
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
