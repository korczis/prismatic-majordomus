//! Version control for the environment snapshot: branch, head, upstream, how far ahead
//! and behind, and what is dirty — from **one** `git status --porcelain=v2 --branch`.
//!
//! One call, because this runs on every `cd`. The obvious implementation asks git seven
//! times (`rev-parse`, `symbolic-ref`, `rev-list --count`, `status`, ...) and pays seven
//! process spawns for facts a single porcelain-v2 report already carries. [`crate::git`]
//! keeps its own narrow calls for the index build, which asks different questions at a
//! different moment; this is the one the hot path uses.
//!
//! Nothing here writes: `status` is read-only, `--no-optional-locks` keeps it from taking
//! the index lock to refresh stat information, so a snapshot taken while a rebase is in
//! flight cannot interfere with it.
//!
//! The lifecycle is one call and one value, and the value covers the case that has no
//! answer: a directory git cannot report on is a state of the snapshot rather than a
//! failure of it, because a shell prompt outside a work tree still has to draw.
//!
//! ```
//! use majordomus_cli::environment::vcs::{self, VcsState};
//! let dir = tempfile::tempdir().expect("a temporary directory");
//! let state = vcs::inspect(dir.path());
//! assert!(matches!(state, VcsState::Unavailable { .. }), "not a work tree, and not an error");
//! assert!(state.tree().is_none(), "and nothing is invented in place of one");
//! ```

use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The command every fact in this module comes from. Named so that a snapshot's
/// provenance can quote what produced it rather than saying "git".
pub const SOURCE: &str = "git status --porcelain=v2 --branch";

/// What version control says about the checkout, or why it could not be asked.
///
/// Two variants and no third: either git answered, or it did not and the reason is
/// carried. There is deliberately no "clean repository with nothing in it" variant that a
/// missing `git` could be mistaken for — a reader that cannot tell "no changes" from "no
/// answer" will report the first when it means the second.
///
/// ```
/// use majordomus_cli::environment::vcs::{self, VcsState};
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// match vcs::inspect(dir.path()) {
///     VcsState::Unavailable { reason } => assert!(!reason.is_empty(), "it says what happened"),
///     VcsState::Git(tree) => panic!("a temporary directory is not a work tree: {tree:?}"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VcsState {
    /// `git` answered.
    Git(GitWorkingTree),
    /// `git` could not be asked, or this is not a work tree.
    Unavailable {
        /// What went wrong.
        reason: String,
    },
}

impl VcsState {
    /// The work tree, when git answered.
    ///
    /// The one way to read the branch and the counts, so that every caller is made to
    /// handle the case where there are none. A surface that wants to say "clean" has to
    /// pass through this `Option` first.
    ///
    /// ```
    /// use majordomus_cli::environment::vcs;
    /// let dir = tempfile::tempdir().expect("a temporary directory");
    /// assert!(
    ///     vcs::inspect(dir.path()).tree().is_none(),
    ///     "a directory git cannot report on has no work tree to read"
    /// );
    /// ```
    pub fn tree(&self) -> Option<&GitWorkingTree> {
        match self {
            VcsState::Git(t) => Some(t),
            VcsState::Unavailable { .. } => None,
        }
    }
}

/// The state of one work tree, as porcelain v2 reports it.
///
/// The counts are kept apart because they mean different things to a person: a staged
/// change is work that is ready, a modified file is work in progress, and an untracked
/// file is often not work at all. `ahead` and `behind` are `Option` for the same reason —
/// a branch with no upstream is not a branch that is level with one.
///
/// ```
/// use majordomus_cli::environment::vcs;
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// let init = std::process::Command::new("git")
///     .args(["init", "--quiet"])
///     .arg(dir.path())
///     .status()
///     .expect("git runs");
/// assert!(init.success());
/// std::fs::write(dir.path().join("note.txt"), "1").expect("a file nobody has added");
///
/// let tree: vcs::GitWorkingTree =
///     vcs::inspect(dir.path()).tree().cloned().expect("git answered for a work tree");
/// assert_eq!(tree.untracked, 1);
/// assert_eq!(tree.dirty_files(), 0, "an untracked file is not a tracked file that changed");
/// assert!(!tree.clean, "though it is still something a person has not dealt with");
/// assert_eq!(tree.ahead, None, "a branch with no upstream is not zero commits ahead of one");
/// assert_eq!(tree.changed_paths, vec!["note.txt".to_string()]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitWorkingTree {
    /// The commit HEAD names, or `None` in a repository with no commits yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The branch, or `None` when HEAD is detached or unborn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Whether HEAD names a commit directly rather than a branch.
    pub detached: bool,
    /// The upstream the branch tracks, when it tracks one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits this branch has that its upstream does not; `None` without an upstream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u32>,
    /// Commits the upstream has that this branch does not; `None` without an upstream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u32>,
    /// Tracked files with staged changes.
    pub staged: u32,
    /// Tracked files changed in the work tree and not staged.
    pub modified: u32,
    /// Files git does not track and is not ignoring.
    pub untracked: u32,
    /// Files with an unresolved merge.
    pub conflicted: u32,
    /// Whether nothing at all is staged, modified, untracked or conflicted.
    pub clean: bool,
    /// Every path that is not clean, repository-relative, in git's order. Used to decide
    /// whether the cached tier is still valid, and shown to nobody.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_paths: Vec<String>,
}

impl GitWorkingTree {
    /// How many tracked files differ from HEAD, staged or not.
    pub fn dirty_files(&self) -> u32 {
        self.staged + self.modified + self.conflicted
    }
}

/// Ask git about `root` with one call. Never fails: a missing `git`, a directory that is
/// not a work tree, or a report this cannot parse all become
/// [`VcsState::Unavailable`] with the reason.
///
/// `budget` bounds nothing here — `git status` has no timeout of its own and a repository
/// large enough to exceed a shell prompt's patience is a fact worth feeling — but the
/// call is the single most expensive thing in a fast resolution, which is why there is
/// exactly one of it.
///
/// ```
/// use majordomus_cli::environment::vcs;
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// let init = std::process::Command::new("git")
///     .args(["init", "--quiet"])
///     .arg(dir.path())
///     .status()
///     .expect("git runs");
/// assert!(init.success());
///
/// let tree = vcs::inspect(dir.path()).tree().cloned().expect("a fresh repository is a work tree");
/// assert!(tree.clean, "nothing has been done in it yet");
/// assert_eq!(tree.upstream, None, "and it tracks nothing");
/// assert_eq!(tree.dirty_files(), 0);
/// ```
pub fn inspect(root: &Path) -> VcsState {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        // --no-optional-locks: a status may otherwise take the index lock to write back
        // refreshed stat information, and a snapshot must not contend with the work.
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
            "-z",
        ])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(e) => {
            return VcsState::Unavailable {
                reason: format!("cannot run git: {e}"),
            }
        }
    };
    if !out.status.success() {
        return VcsState::Unavailable {
            reason: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        };
    }
    match parse(&String::from_utf8_lossy(&out.stdout)) {
        Some(tree) => VcsState::Git(tree),
        None => VcsState::Unavailable {
            reason: "git status --porcelain=v2 produced no branch header".into(),
        },
    }
}

/// Parse a `git status --porcelain=v2 --branch -z` report.
///
/// Records are NUL-separated. A rename record (`2 `) carries two NUL-separated paths, so
/// the parser consumes one extra record after it; a parser that does not is silently one
/// record out of step for the rest of the report, which is the classic way this format is
/// got wrong.
///
/// ```
/// use majordomus_cli::environment::vcs::parse;
/// // One record per entry, joined by the NUL the report separates them with.
/// let report = [
///     "# branch.oid abc123",
///     "# branch.head master",
///     "# branch.upstream origin/master",
///     "# branch.ab +2 -1",
///     "1 .M N... 100644 100644 100644 aaa bbb src/a.rs",
///     "1 M. N... 100644 100644 100644 ccc ddd src/b.rs",
///     "? notes.txt",
/// ].join("\0");
/// let tree = parse(&report).expect("a branch header");
/// assert_eq!(tree.branch.as_deref(), Some("master"));
/// assert_eq!(tree.upstream.as_deref(), Some("origin/master"));
/// assert_eq!((tree.ahead, tree.behind), (Some(2), Some(1)));
/// assert_eq!((tree.staged, tree.modified, tree.untracked), (1, 1, 1));
/// assert!(!tree.clean);
/// ```
pub fn parse(report: &str) -> Option<GitWorkingTree> {
    let mut head = None;
    let mut branch = None;
    let mut detached = false;
    let mut upstream = None;
    let mut ahead = None;
    let mut behind = None;
    let (mut staged, mut modified, mut untracked, mut conflicted) = (0u32, 0u32, 0u32, 0u32);
    let mut changed_paths: Vec<String> = Vec::new();
    let mut saw_header = false;

    let mut records = report.split('\0').filter(|r| !r.is_empty());
    while let Some(record) = records.next() {
        match record.as_bytes().first() {
            Some(b'#') => {
                saw_header = true;
                let rest = record.strip_prefix("# ").unwrap_or(record);
                let (key, value) = match rest.split_once(' ') {
                    Some(pair) => pair,
                    None => continue,
                };
                match key {
                    "branch.oid" if value != "(initial)" => head = Some(value.to_string()),
                    "branch.head" if value == "(detached)" => detached = true,
                    "branch.head" => branch = Some(value.to_string()),
                    "branch.upstream" => upstream = Some(value.to_string()),
                    "branch.ab" => {
                        let mut parts = value.split_whitespace();
                        ahead = parts.next().and_then(signed);
                        behind = parts.next().and_then(signed);
                    }
                    _ => {}
                }
            }
            // "1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>"
            Some(b'1') => {
                if let Some((x, y, path)) = ordinary(record, 8) {
                    if x != '.' {
                        staged += 1;
                    }
                    if y != '.' {
                        modified += 1;
                    }
                    changed_paths.push(path);
                }
            }
            // "2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>" then the original
            // path as its own NUL-separated record.
            Some(b'2') => {
                if let Some((x, y, path)) = ordinary(record, 9) {
                    if x != '.' {
                        staged += 1;
                    }
                    if y != '.' {
                        modified += 1;
                    }
                    changed_paths.push(path);
                }
                // consume the original path of the rename, whatever it is
                let _ = records.next();
            }
            Some(b'u') => {
                conflicted += 1;
                if let Some((_, _, path)) = ordinary(record, 10) {
                    changed_paths.push(path);
                }
            }
            Some(b'?') => {
                untracked += 1;
                if let Some(path) = record.strip_prefix("? ") {
                    changed_paths.push(path.to_string());
                }
            }
            // '!' is an ignored file, which --untracked-files=normal does not report and
            // which would not be news if it did.
            _ => {}
        }
    }
    if !saw_header {
        return None;
    }
    changed_paths.sort();
    changed_paths.dedup();
    Some(GitWorkingTree {
        head,
        branch,
        detached,
        upstream,
        ahead,
        behind,
        staged,
        modified,
        untracked,
        conflicted,
        clean: staged == 0 && modified == 0 && untracked == 0 && conflicted == 0,
        changed_paths,
    })
}

/// The two status characters and the path of an entry whose path starts at `fields`.
fn ordinary(record: &str, fields: usize) -> Option<(char, char, String)> {
    let mut chars = record.split(' ').nth(1)?.chars();
    let x = chars.next()?;
    let y = chars.next()?;
    let path = record.splitn(fields + 1, ' ').nth(fields)?;
    Some((x, y, path.to_string()))
}

/// `+2` or `-1` as a count.
fn signed(text: &str) -> Option<u32> {
    text.trim_start_matches(['+', '-']).parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_checkout_is_clean() {
        let tree = parse("# branch.oid abc\0# branch.head master\0").expect("a header");
        assert!(tree.clean);
        assert_eq!(tree.dirty_files(), 0);
        assert_eq!(tree.branch.as_deref(), Some("master"));
        assert_eq!(tree.ahead, None, "no upstream is not zero ahead");
    }

    #[test]
    fn a_detached_head_has_no_branch_and_says_so() {
        let tree = parse("# branch.oid abc\0# branch.head (detached)\0").expect("a header");
        assert!(tree.detached);
        assert_eq!(tree.branch, None);
    }

    #[test]
    fn an_unborn_repository_has_no_head() {
        let tree = parse("# branch.oid (initial)\0# branch.head master\0").expect("a header");
        assert_eq!(tree.head, None);
        assert_eq!(tree.branch.as_deref(), Some("master"));
    }

    /// A rename record is followed by a second record holding the original path. A parser
    /// that does not consume it reads that path as the next entry — it starts with neither
    /// `1` nor `?`, so it is silently dropped, and every count after a rename is wrong by
    /// however many renames preceded it. This is the assertion that would catch it.
    #[test]
    fn a_rename_consumes_its_original_path_record() {
        let report = "# branch.oid abc\0# branch.head master\0\
                      2 R. N... 100644 100644 100644 aaa bbb R100 new.rs\0old.rs\0\
                      ? untracked.txt\0";
        let tree = parse(report).expect("a header");
        assert_eq!(tree.staged, 1);
        assert_eq!(
            tree.untracked, 1,
            "the untracked entry after a rename is seen"
        );
        assert_eq!(tree.changed_paths, vec!["new.rs", "untracked.txt"]);
    }

    #[test]
    fn a_path_with_spaces_survives_the_field_split() {
        let report = "# branch.oid abc\0# branch.head master\0\
                      1 .M N... 100644 100644 100644 aaa bbb docs/a file.md\0";
        let tree = parse(report).expect("a header");
        assert_eq!(tree.changed_paths, vec!["docs/a file.md"]);
        assert_eq!(tree.modified, 1);
    }

    #[test]
    fn a_conflict_is_neither_staged_nor_modified() {
        let report = "# branch.oid abc\0# branch.head master\0\
                      u UU N... 100644 100644 100644 100644 aaa bbb ccc src/x.rs\0";
        let tree = parse(report).expect("a header");
        assert_eq!(tree.conflicted, 1);
        assert_eq!(tree.staged, 0);
        assert!(!tree.clean);
    }

    #[test]
    fn a_report_without_a_branch_header_is_not_a_report() {
        assert!(parse("").is_none());
        assert!(parse("? x\0").is_none());
    }

    #[test]
    fn inspecting_a_directory_that_is_not_a_work_tree_says_why() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        match inspect(dir.path()) {
            VcsState::Unavailable { reason } => assert!(!reason.is_empty()),
            VcsState::Git(_) => panic!("a bare temporary directory is not a work tree"),
        }
    }
}
