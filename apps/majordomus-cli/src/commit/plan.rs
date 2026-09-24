//! The plan: what is in the working tree, how it divides into commits, and the fingerprint
//! that says the plan is still about this tree.
//!
//! # Why a plan is a value with a fingerprint on it
//!
//! A plan is derived from a working tree at a moment. In a repository worked by one person
//! at a time that is a detail; in this one it is the correctness question. Several workers
//! share a checkout, more share a repository, and a plan computed at one HEAD and executed
//! at another commits files somebody else staged, under a message about work that is no
//! longer what changed.
//!
//! [`PlanFingerprint`] is what makes that detectable rather than merely unlikely: the
//! repository, the worktree, HEAD, and the exact set of paths and states the plan was
//! derived from. A plan carries it, and anything that acts on a plan compares it with the
//! tree in front of it first. The comparison is cheap, it is not a lock, and it does not
//! prevent the race — it refuses to be the one that loses it silently.
//!
//! # What grouping is and is not
//!
//! Grouping is a *recommendation with its reasoning attached*, never an automatic split.
//! The evidence is what the repository can see: which directories changed, which scope the
//! history gives them, and which files are derived from which. Where that evidence supports
//! one commit it says one; where it supports several it says several and says why; where it
//! supports nothing it says so, which is the honest answer for a tree of unrelated edits
//! that only a person can divide.

//! ```
//! use majordomus_cli::commit::{ChangeStage, PlanFingerprint, WorkingTreeState};
//! use majordomus_cli::commit::plan::PathChange;
//!
//! let change = |path: &str, stage| PathChange {
//!     path: path.into(), stage, status: "M.".into(), partial: false,
//! };
//! let tree = WorkingTreeState {
//!     head: Some("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".into()),
//!     changes: vec![change("src/a.rs", ChangeStage::Staged)],
//!     ..WorkingTreeState::default()
//! };
//!
//! // The same tree fingerprints the same way every time.
//! let made = PlanFingerprint::of("/repo/.git", "/repo", &tree);
//! assert!(made.differs_from(&PlanFingerprint::of("/repo/.git", "/repo", &tree)).is_none());
//!
//! // Another worktree of the same repository is a different subject, and is named as one.
//! let elsewhere = PlanFingerprint::of("/repo/.git", "/repo-wt/feature/x", &tree);
//! let why = made.differs_from(&elsewhere).expect("another worktree");
//! assert!(why.contains("another worktree"), "{why}");
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::scopes::{ScopeSuggestion, ScopeVocabulary};
use super::{CommitHeader, CommitMessage};
use crate::model::Diagnostic;
use crate::release::ChangeKind;

/// Where one path stands between the index and the working tree.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// ```
/// use majordomus_cli::commit::ChangeStage;
/// // ordered so that what would be in the next commit sorts first
/// assert!(ChangeStage::Staged < ChangeStage::Unstaged);
/// assert!(ChangeStage::Unstaged < ChangeStage::Untracked);
/// ```
pub enum ChangeStage {
    /// Different in the index from HEAD: it would be in the next commit.
    Staged,
    /// Different in the working tree from the index: it would not.
    Unstaged,
    /// Not tracked at all.
    Untracked,
}

/// One path, and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::ChangeStage;
/// use majordomus_cli::commit::plan::PathChange;
/// // staged and modified again since: the commit would carry the staged half only
/// let c = PathChange {
///     path: "src/a.rs".into(), stage: ChangeStage::Staged, status: "MM".into(), partial: true,
/// };
/// assert!(c.partial);
/// ```
pub struct PathChange {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// Whether it is staged, unstaged or untracked. A path modified both in the index and
    /// in the working tree appears once, as [`ChangeStage::Staged`], with `partial` set.
    pub stage: ChangeStage,
    /// The two-letter status git reported, verbatim.
    pub status: String,
    /// Set when the path is staged *and* modified again since: committing it commits the
    /// staged half only, which is a thing a person should be told rather than discover.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

/// What git says about the working tree right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
/// ```
/// use majordomus_cli::commit::WorkingTreeState;
/// let clean = WorkingTreeState::default();
/// assert!(clean.is_clean());
/// assert!(clean.staged().is_empty());
/// ```
pub struct WorkingTreeState {
    /// The branch, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// HEAD's full name, or `None` in a repository with no commits.
    pub head: Option<String>,
    /// The upstream ref, when the branch has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits this branch has that its upstream does not.
    pub ahead: usize,
    /// Commits the upstream has that this branch does not.
    pub behind: usize,
    /// A merge, rebase, cherry-pick or bisect in progress, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_progress: Option<String>,
    /// Every change, ordered by stage then path.
    pub changes: Vec<PathChange>,
}

impl WorkingTreeState {
    /// The paths that would be in a commit made right now.
    /// ```
    /// use majordomus_cli::commit::{ChangeStage, WorkingTreeState};
    /// use majordomus_cli::commit::plan::PathChange;
    /// let c = |p: &str, s| PathChange { path: p.into(), stage: s, status: "M.".into(), partial: false };
    /// let tree = WorkingTreeState {
    ///     changes: vec![c("a.rs", ChangeStage::Staged), c("b.rs", ChangeStage::Unstaged)],
    ///     ..WorkingTreeState::default()
    /// };
    /// assert_eq!(tree.staged(), vec!["a.rs".to_string()]);
    /// assert!(!tree.is_clean());
    /// ```
    pub fn staged(&self) -> Vec<String> {
        self.changes
            .iter()
            .filter(|c| c.stage == ChangeStage::Staged)
            .map(|c| c.path.clone())
            .collect()
    }

    /// Whether there is nothing to commit.
    pub fn is_clean(&self) -> bool {
        self.changes.is_empty()
    }
}

/// What a plan was derived from.
///
/// Compared, never trusted: a plan whose fingerprint differs from the tree in front of it
/// describes a tree that no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::{PlanFingerprint, WorkingTreeState};
/// let f = PlanFingerprint::of("/repo/.git", "/repo", &WorkingTreeState::default());
/// assert_eq!(f.head, "unborn", "a repository with no commits says so");
/// ```
pub struct PlanFingerprint {
    /// The repository: git's common directory, which every worktree of one repository
    /// shares and no two repositories do.
    pub repository: String,
    /// The worktree: this checkout's own path. Two worktrees of one repository have the
    /// same repository and different worktrees, which is what keeps their plans apart.
    pub worktree: String,
    /// HEAD when the plan was derived, or `unborn`.
    pub head: String,
    /// SHA-256 over every change, its stage and its status, in order. Restaging one file
    /// changes it; touching a file the plan does not contain does not.
    pub changes: String,
}

impl PlanFingerprint {
    /// The fingerprint of a tree as it stands.
    /// ```
    /// use majordomus_cli::commit::{PlanFingerprint, WorkingTreeState};
    /// let a = PlanFingerprint::of("/r/.git", "/r", &WorkingTreeState::default());
    /// let b = PlanFingerprint::of("/r/.git", "/r", &WorkingTreeState::default());
    /// assert_eq!(a, b, "deterministic over one tree");
    /// ```
    pub fn of(repository: &str, worktree: &str, tree: &WorkingTreeState) -> PlanFingerprint {
        let mut hasher = Sha256::new();
        for c in &tree.changes {
            hasher.update(c.path.as_bytes());
            hasher.update([0]);
            hasher.update(format!("{:?}", c.stage).as_bytes());
            hasher.update([0]);
            hasher.update(c.status.as_bytes());
            hasher.update([0]);
        }
        PlanFingerprint {
            repository: repository.to_string(),
            worktree: worktree.to_string(),
            head: tree.head.clone().unwrap_or_else(|| "unborn".into()),
            changes: format!("{:x}", hasher.finalize()),
        }
    }

    /// Why this fingerprint does not describe `now`, or `None` when it does.
    ///
    /// A sentence rather than a boolean, because "the plan is stale" is not an answer
    /// anybody can act on and "HEAD moved from a1b2c3d to e4f5a6b" is.
    /// ```
    /// use majordomus_cli::commit::{PlanFingerprint, WorkingTreeState};
    /// let mut then = WorkingTreeState::default();
    /// then.head = Some("a1b2c3d4e5f6".into());
    /// let mut now = then.clone();
    /// now.head = Some("ffffffffffff".into());
    /// let why = PlanFingerprint::of("/r/.git", "/r", &then)
    ///     .differs_from(&PlanFingerprint::of("/r/.git", "/r", &now))
    ///     .expect("HEAD moved");
    /// // a sentence, because "the plan is stale" is not something anybody can act on
    /// assert!(why.contains("HEAD moved"), "{why}");
    /// ```
    pub fn differs_from(&self, now: &PlanFingerprint) -> Option<String> {
        if self.repository != now.repository {
            return Some(format!(
                "the plan is of another repository ({} rather than {})",
                self.repository, now.repository
            ));
        }
        if self.worktree != now.worktree {
            return Some(format!(
                "the plan is of another worktree ({} rather than {})",
                self.worktree, now.worktree
            ));
        }
        if self.head != now.head {
            return Some(format!(
                "HEAD moved from {} to {} since the plan was made",
                short(&self.head),
                short(&now.head)
            ));
        }
        if self.changes != now.changes {
            return Some(
                "the working tree changed since the plan was made: what is staged, or what is modified, is not what it was".into(),
            );
        }
        None
    }
}

fn short(name: &str) -> &str {
    &name[..9.min(name.len())]
}

/// One commit a plan proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One commit a plan proposes: its message, its paths, and why those paths are one commit.
///
/// ```
/// use majordomus_cli::commit::{plan::group, CommitGroup, ScopeVocabulary};
/// let mut v = ScopeVocabulary::default();
/// v.learn("commit", "src/commit", 40);
/// let g: Vec<CommitGroup> = group(&["src/commit/a.rs".into()], &v, &[]);
/// // the subject is empty on purpose: what a change did is the one thing no evidence in
/// // the tree can state
/// assert_eq!(g[0].message.header.subject, "");
/// assert_eq!(g[0].message.header.scope.as_deref(), Some("commit"));
/// assert!(g[0].rationale.contains("40 prior commit"), "{}", g[0].rationale);
/// ```
pub struct CommitGroup {
    /// The message as proposed. The summary is the one field a person is expected to
    /// replace: nothing here can know *why* a change was made, and a planner that wrote a
    /// confident sentence about it would be writing fiction into the history.
    pub message: CommitMessage,
    /// The paths, in order.
    pub paths: Vec<String>,
    /// Why these paths are one commit, in evidence a reader can check.
    pub rationale: String,
    /// The scope suggestion and what it was derived from, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_from: Option<ScopeSuggestion>,
}

/// A plan over a working tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::{CommitPlan, PlanFingerprint, WorkingTreeState};
/// let plan = CommitPlan {
///     fingerprint: PlanFingerprint::of("/r/.git", "/r", &WorkingTreeState::default()),
///     tree: WorkingTreeState::default(),
///     groups: Vec::new(),
///     diagnostics: Vec::new(),
/// };
/// assert!(plan.groups.is_empty(), "a clean tree proposes nothing");
/// ```
pub struct CommitPlan {
    /// What the plan was derived from, and what makes it stale.
    pub fingerprint: PlanFingerprint,
    /// The tree as it stood.
    pub tree: WorkingTreeState,
    /// The commits proposed, in the order they should be made.
    pub groups: Vec<CommitGroup>,
    /// What a person should know before acting on it.
    pub diagnostics: Vec<Diagnostic>,
}

/// Read `git status --porcelain=v2 --branch`, which reports the branch, the upstream and
/// the divergence in the same call as the paths.
///
/// Returns `None` when git could not be asked or the directory is not a work tree; the
/// caller reports that, because "there is no working tree" is a different answer from "the
/// working tree is clean" and a plan that confused the two would propose committing nothing
/// in a directory that is not a repository.
/// ```
/// use majordomus_cli::commit::plan::working_tree;
/// // "there is no working tree" and "the working tree is clean" are different answers
/// let plain = tempfile::tempdir().expect("a temporary directory");
/// assert!(working_tree(plain.path()).is_none());
/// ```
pub fn working_tree(root: &Path) -> Option<WorkingTreeState> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
            "--no-renames",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut tree = WorkingTreeState::default();
    let mut both: Vec<String> = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(rest) = line.strip_prefix("# branch.") {
            let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
            match key {
                "oid" if value != "(initial)" => tree.head = Some(value.to_string()),
                "head" if value != "(detached)" => tree.branch = Some(value.to_string()),
                "upstream" => tree.upstream = Some(value.to_string()),
                "ab" => {
                    let mut parts = value.split_whitespace();
                    tree.ahead = parts
                        .next()
                        .and_then(|s| s.trim_start_matches('+').parse().ok())
                        .unwrap_or(0);
                    tree.behind = parts
                        .next()
                        .and_then(|s| s.trim_start_matches('-').parse().ok())
                        .unwrap_or(0);
                }
                _ => {}
            }
            continue;
        }
        // `1 XY sub mH mI mW hH hI path` for an ordinary change, `? path` for untracked,
        // `u ...` for unmerged. The status letters are the two after the record kind.
        let mut fields = line.splitn(2, ' ');
        let kind = fields.next().unwrap_or_default();
        let rest = fields.next().unwrap_or_default();
        match kind {
            "?" => tree.changes.push(PathChange {
                path: rest.to_string(),
                stage: ChangeStage::Untracked,
                status: "??".into(),
                partial: false,
            }),
            "1" | "2" | "u" => {
                let mut parts = rest.split(' ');
                let xy = parts.next().unwrap_or("  ");
                let path = rest.split(' ').next_back().unwrap_or_default().to_string();
                if path.is_empty() {
                    continue;
                }
                let index = xy.chars().next().unwrap_or('.');
                let worktree = xy.chars().nth(1).unwrap_or('.');
                let staged = index != '.';
                if staged && worktree != '.' {
                    both.push(path.clone());
                }
                tree.changes.push(PathChange {
                    path,
                    stage: if staged {
                        ChangeStage::Staged
                    } else {
                        ChangeStage::Unstaged
                    },
                    status: xy.to_string(),
                    partial: staged && worktree != '.',
                });
            }
            _ => {}
        }
    }
    tree.in_progress = in_progress(root);
    crate::order::canonical(&mut tree.changes);
    tree.changes
        .dedup_by(|a, b| a.path == b.path && a.stage == b.stage);
    Some(tree)
}

/// A merge, rebase, cherry-pick, revert or bisect in progress, by the file git leaves
/// behind for it. Asked of the *git directory*, which in a linked worktree is not `.git/`
/// in the checkout but a directory under the repository's common one — `git rev-parse` is
/// what knows which, so nothing here assumes.
fn in_progress(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let dir = Path::new(String::from_utf8_lossy(&out.stdout).trim()).to_path_buf();
    for (file, what) in [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ] {
        if dir.join(file).exists() {
            return Some(what.to_string());
        }
    }
    None
}

/// The kind a set of paths suggests, from what the paths *are*.
///
/// Only the cases where the paths decide it on their own: a change that is entirely tests
/// is a test change, one that is entirely documentation is a documentation change, one that
/// is entirely pipeline is a pipeline change. Everything else is left unset, because the
/// difference between a feature, a fix and a refactor is in the diff's meaning and not in
/// its file names, and a planner that guessed would be putting the wrong word in front of
/// every commit in the history.
/// ```
/// use majordomus_cli::commit::plan::kind_of;
/// use majordomus_cli::release::ChangeKind;
/// assert_eq!(kind_of(&["docs/COMMIT.md".into()]), Some(ChangeKind::Docs));
/// // a source change could be a feature, a fix or a refactor; the file names do not say
/// assert_eq!(kind_of(&["src/a.rs".into()]), None);
/// ```
pub fn kind_of(paths: &[String]) -> Option<ChangeKind> {
    if paths.is_empty() {
        return None;
    }
    let all = |f: fn(&str) -> bool| paths.iter().all(|p| f(p));
    if all(|p| p.starts_with("docs/") || p.ends_with(".md")) {
        return Some(ChangeKind::Docs);
    }
    if all(|p| p.starts_with("test/") || p.starts_with("tests/") || p.contains("/tests/")) {
        return Some(ChangeKind::Test);
    }
    if all(|p| p.starts_with(".github/") || p.starts_with("scripts/ci/")) {
        return Some(ChangeKind::Ci);
    }
    None
}

/// Which of `paths` this repository declares derived, asked of git rather than parsed.
///
/// `.gitattributes` marks every generated file `merge=derived`, and `git check-attr` is the
/// reader that already understands its precedence, its globs and its negations. A second
/// parser here would be a second opinion about which files are generated, which is the class
/// of defect this repository spends its time removing — and one that has already cost it
/// once, when a hand-written glob claimed forty-five files of which the generator writes
/// nine.
/// ```
/// use majordomus_cli::commit::plan::derived_among;
/// let plain = tempfile::tempdir().expect("a temporary directory");
/// // nothing to ask about is nothing derived, and git is never run
/// assert!(derived_among(plain.path(), &[]).is_empty());
/// ```
pub fn derived_among(root: &Path, paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-attr", "merge", "--"])
        .args(paths)
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    // `<path>: merge: derived`, one line each. A path may contain a colon, so the match is
    // against the fixed suffix rather than a split from the left.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_suffix(": merge: derived"))
        .map(str::to_string)
        .collect()
}

/// Group a set of paths into the commits the evidence supports.
///
/// The evidence is the scope vocabulary: paths the history scopes the same way are one
/// commit. Paths the vocabulary knows nothing about are one further group rather than one
/// group each, because the alternative — a commit per unrecognised file — is never what
/// somebody wanted and is the failure that makes a planner untrustworthy on its first run
/// in a new repository.
///
/// `derived` is the subset of `paths` this repository generates. Its presence collapses the
/// plan to one commit, and the reason is `project.derived-files-regenerated`: a generated
/// file is a projection of the whole tree, so only the last of several commits could carry a
/// current one and every earlier commit would be stale by construction — which the
/// `derived-current` hook then refuses, one commit at a time, after the split has already
/// been made. The planner found this on its first run against its own change set, where it
/// proposed putting twenty-three generated files in a commit of their own.
/// ```
/// use majordomus_cli::commit::{plan::group, ScopeVocabulary};
/// let mut v = ScopeVocabulary::default();
/// v.learn("alpha", "src/alpha", 40);
/// v.learn("derive", "docs/generated", 180);
/// let paths: Vec<String> = vec!["src/alpha/a.rs".into(), "docs/generated/x.json".into()];
/// // without the derived-file rule the scopes alone would make two commits
/// assert_eq!(group(&paths, &v, &[]).len(), 2);
/// // with one of them generated, it is one, and the rationale names the rule that decided
/// let one = group(&paths, &v, &["docs/generated/x.json".into()]);
/// assert_eq!(one.len(), 1);
/// assert!(one[0].rationale.contains("derived-files-regenerated"), "{}", one[0].rationale);
/// ```
pub fn group(
    paths: &[String],
    vocabulary: &ScopeVocabulary,
    derived: &[String],
) -> Vec<CommitGroup> {
    let natural = group_by_scope(paths, vocabulary);
    if !derived.is_empty() && natural.len() > 1 {
        // The scope of the one commit is the sources' scope, not the generated tree's: what
        // the change is about is what was written, and the projection follows it.
        let sources: Vec<String> = paths
            .iter()
            .filter(|p| !derived.contains(p))
            .cloned()
            .collect();
        let scope_from = vocabulary.suggest(&sources).filter(|s| !s.ambiguous);
        let kind = kind_of(&sources);
        let mut all = paths.to_vec();
        crate::order::canonical_strings(&mut all);
        all.dedup();
        let would_have = natural.len();
        return vec![CommitGroup {
            message: CommitMessage {
                header: CommitHeader {
                    word: kind.and_then(|k| k.word()).unwrap_or_default().to_string(),
                    kind: kind.unwrap_or(ChangeKind::Other),
                    scope: scope_from.as_ref().map(|s| s.scope.clone()),
                    breaking: false,
                    subject: String::new(),
                },
                body: String::new(),
                trailers: Vec::new(),
            },
            paths: all,
            rationale: format!(
                "one commit, not {would_have}: {} of these {} file(s) are generated, and a generated file is a projection of the whole tree — split, every commit but the last would carry derived data older than its own sources (project.derived-files-regenerated)",
                derived.len(),
                paths.len()
            ),
            scope_from,
        }];
    }
    natural
}

/// The grouping the scope vocabulary alone supports, before the derived-file rule applies.
fn group_by_scope(paths: &[String], vocabulary: &ScopeVocabulary) -> Vec<CommitGroup> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut by_scope: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    // Why a path got no scope, so that the group can say which: a repository that has never
    // committed anything about this directory, and a directory a dozen scopes have equal
    // claim to, are different situations and lead a reader to different next steps.
    let mut ambiguous = 0usize;
    let mut unknown = 0usize;
    for path in paths {
        let suggestion = vocabulary.suggest(std::slice::from_ref(path));
        match &suggestion {
            Some(s) if s.ambiguous => ambiguous += 1,
            None => unknown += 1,
            _ => {}
        }
        let scope = suggestion.filter(|s| !s.ambiguous).map(|s| s.scope);
        by_scope.entry(scope).or_default().push(path.clone());
    }
    // Paths within a group are ordered by name, so that a plan is a function of the tree
    // and not of the order git happened to report it in.
    for group in by_scope.values_mut() {
        crate::order::canonical_strings(group);
        group.dedup();
    }
    // Deterministic: named scopes first, in vocabulary order, then the unrecognised group.
    let order = vocabulary.words();
    let mut keys: Vec<ScopeKey> = by_scope
        .keys()
        .map(|k| ScopeKey {
            position: k
                .as_ref()
                .and_then(|s| order.iter().position(|w| w == s))
                .map_or(i64::MAX, |p| p as i64),
            scope: k.clone(),
        })
        .collect();
    crate::order::canonical(&mut keys);
    let keys: Vec<Option<String>> = keys.into_iter().map(|k| k.scope).collect();
    keys.into_iter()
        .map(|scope| {
            let paths = by_scope.remove(&scope).unwrap_or_default();
            let scope_from = scope
                .as_ref()
                .and_then(|_| vocabulary.suggest(&paths));
            let kind = kind_of(&paths);
            let rationale = match (&scope, &scope_from) {
                (Some(s), Some(from)) => format!(
                    "{} file(s) under {}, which {} prior commit(s) scoped `{s}`",
                    paths.len(),
                    from.directory,
                    from.commits
                ),
                _ if ambiguous > 0 && unknown > 0 => format!(
                    "{} file(s) with no scope of their own: {unknown} in directories the history has never scoped, {ambiguous} in directories several scopes have equal claim to. One group rather than one commit each",
                    paths.len()
                ),
                _ if ambiguous > 0 => format!(
                    "{} file(s) in directories several scopes have equal claim to; the history does not decide between them, so they are one group and the scope is yours to name",
                    paths.len()
                ),
                _ => format!(
                    "{} file(s) in directories this history has never scoped; they are one group rather than one commit each",
                    paths.len()
                ),
            };
            CommitGroup {
                message: CommitMessage {
                    header: CommitHeader {
                        word: kind.and_then(|k| k.word()).unwrap_or_default().to_string(),
                        kind: kind.unwrap_or(ChangeKind::Other),
                        scope: scope.clone(),
                        breaking: false,
                        // Deliberately empty: what the change *did* is the one thing no
                        // evidence in the tree can state, and the plan says so rather than
                        // inventing a sentence.
                        subject: String::new(),
                    },
                    body: String::new(),
                    trailers: Vec::new(),
                },
                paths,
                rationale,
                scope_from,
            }
        })
        .collect()
}

/// A plan's changes in canonical order: by path, then by the status git reported, so the
/// halves of one path stay adjacent and `dedup_by` sees them together.
impl crate::order::Ordered for PathChange {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.path, &self.status)
    }
}

/// A group's scope with its place in the vocabulary: named scopes keep the vocabulary's
/// order, and the unrecognised group comes last.
struct ScopeKey {
    scope: Option<String>,
    position: i64,
}

impl crate::order::Ordered for ScopeKey {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        match &self.scope {
            Some(s) => crate::order::OrderKey::grouped("named", s, s).ranked(self.position),
            None => crate::order::OrderKey::plain("", ""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::scopes::ScopeUse;

    fn tree(changes: &[(&str, ChangeStage, &str)]) -> WorkingTreeState {
        WorkingTreeState {
            head: Some("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".into()),
            changes: changes
                .iter()
                .map(|(p, s, st)| PathChange {
                    path: p.to_string(),
                    stage: *s,
                    status: st.to_string(),
                    partial: false,
                })
                .collect(),
            ..WorkingTreeState::default()
        }
    }

    #[test]
    fn a_fingerprint_is_stable_for_one_tree_and_moves_with_it() {
        let a = tree(&[("src/a.rs", ChangeStage::Staged, "M.")]);
        let f = PlanFingerprint::of("/repo/.git", "/wt", &a);
        assert_eq!(
            f,
            PlanFingerprint::of("/repo/.git", "/wt", &a),
            "deterministic"
        );
        assert!(f
            .differs_from(&PlanFingerprint::of("/repo/.git", "/wt", &a))
            .is_none());
    }

    #[test]
    fn a_moved_head_is_named_rather_than_merely_reported() {
        let a = tree(&[("src/a.rs", ChangeStage::Staged, "M.")]);
        let mut b = a.clone();
        b.head = Some("ffffffffffffffffffffffffffffffffffffffff".into());
        let why = PlanFingerprint::of("/repo/.git", "/wt", &a)
            .differs_from(&PlanFingerprint::of("/repo/.git", "/wt", &b))
            .expect("it moved");
        assert!(why.contains("HEAD moved"), "{why}");
        assert!(why.contains("a1b2c3d4e"), "the sha it was at: {why}");
    }

    #[test]
    fn restaging_makes_a_plan_stale() {
        let before = tree(&[("src/a.rs", ChangeStage::Unstaged, ".M")]);
        let after = tree(&[("src/a.rs", ChangeStage::Staged, "M.")]);
        let why = PlanFingerprint::of("/r", "/w", &before)
            .differs_from(&PlanFingerprint::of("/r", "/w", &after))
            .expect("what is staged changed");
        assert!(why.contains("working tree changed"), "{why}");
    }

    #[test]
    fn two_worktrees_of_one_repository_do_not_share_a_plan() {
        // The failure this prevents: a plan derived in one checkout executing in another,
        // where the same branch name and the same HEAD would otherwise look identical.
        let t = tree(&[("src/a.rs", ChangeStage::Staged, "M.")]);
        let a = PlanFingerprint::of("/repo/.git", "/repo", &t);
        let b = PlanFingerprint::of("/repo/.git", "/repo-wt/feature/x", &t);
        assert_eq!(a.repository, b.repository, "one repository");
        let why = a.differs_from(&b).expect("another worktree");
        assert!(why.contains("another worktree"), "{why}");
    }

    #[test]
    fn a_plan_of_another_repository_is_refused_before_anything_else() {
        let t = tree(&[]);
        let why = PlanFingerprint::of("/a/.git", "/a", &t)
            .differs_from(&PlanFingerprint::of("/b/.git", "/b", &t))
            .expect("another repository");
        assert!(why.contains("another repository"), "{why}");
    }

    fn vocabulary(rows: &[(&str, &str, usize)]) -> ScopeVocabulary {
        let mut v = ScopeVocabulary::default();
        for (scope, prefix, n) in rows {
            v.learn(scope, prefix, *n);
            if !v.knows(scope) {
                v.scopes.push(ScopeUse {
                    scope: scope.to_string(),
                    commits: *n,
                    directories: vec![prefix.to_string()],
                });
            }
        }
        v
    }

    #[test]
    fn paths_the_history_scopes_alike_are_one_commit() {
        let v = vocabulary(&[("commit", "apps/cli/src", 40), ("site", "site/content", 30)]);
        let groups = group_by_scope(
            &[
                "apps/cli/src/commit/plan.rs".into(),
                "apps/cli/src/commit/verdict.rs".into(),
                "site/content/a.md".into(),
            ],
            &v,
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].message.header.scope.as_deref(), Some("commit"));
        assert_eq!(groups[0].paths.len(), 2);
        assert_eq!(groups[1].message.header.scope.as_deref(), Some("site"));
        // and the reason is evidence a reader can check, not an assertion
        assert!(
            groups[0].rationale.contains("40 prior commit"),
            "{}",
            groups[0].rationale
        );
    }

    #[test]
    fn unrecognised_paths_are_one_group_and_not_one_commit_each() {
        let v = vocabulary(&[("commit", "apps/cli/src", 40)]);
        let groups = group(&["brand/new/a.rs".into(), "brand/new/b.rs".into()], &v, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].paths.len(), 2);
        assert!(groups[0].message.header.scope.is_none());
        assert!(
            groups[0].rationale.contains("never scoped"),
            "{}",
            groups[0].rationale
        );
    }

    #[test]
    fn a_group_proposes_no_subject_because_nothing_in_the_tree_knows_why() {
        let v = vocabulary(&[("commit", "apps/cli/src", 40)]);
        let groups = group_by_scope(&["apps/cli/src/commit/plan.rs".into()], &v);
        assert_eq!(groups[0].message.header.subject, "");
    }

    #[test]
    fn a_kind_is_taken_from_the_paths_only_when_the_paths_decide_it() {
        assert_eq!(kind_of(&["docs/COMMIT.md".into()]), Some(ChangeKind::Docs));
        assert_eq!(kind_of(&["test/cases/1.sh".into()]), Some(ChangeKind::Test));
        assert_eq!(
            kind_of(&[".github/workflows/ci.yml".into()]),
            Some(ChangeKind::Ci)
        );
        // a source change could be a feature, a fix or a refactor; the file names do not say
        assert_eq!(kind_of(&["src/a.rs".into()]), None);
        // and a mixed set decides nothing
        assert_eq!(kind_of(&["docs/a.md".into(), "src/a.rs".into()]), None);
        assert_eq!(kind_of(&[]), None);
    }

    #[test]
    fn grouping_is_deterministic_whatever_order_the_paths_arrive_in() {
        let v = vocabulary(&[("commit", "apps/cli/src", 40), ("site", "site/content", 30)]);
        let forward: Vec<String> = vec![
            "apps/cli/src/a.rs".into(),
            "site/content/b.md".into(),
            "apps/cli/src/c.rs".into(),
        ];
        let mut reversed = forward.clone();
        reversed.reverse();
        let a = group(&forward, &v, &[]);
        let b = group(&reversed, &v, &[]);
        assert_eq!(
            a.iter().map(|g| g.paths.clone()).collect::<Vec<_>>(),
            b.iter().map(|g| g.paths.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_directory_that_is_not_a_work_tree_is_not_a_clean_tree() {
        // The distinction that matters: "nothing to commit" and "this is not a repository"
        // are different answers, and a planner that returned the first for the second would
        // propose committing nothing rather than say where it is.
        let plain = tempfile::tempdir().expect("a temporary directory");
        assert!(working_tree(plain.path()).is_none());
    }
}

/// Every commit in `range`, as a subject and a body, newest first.
///
/// The same separators the changelog reads the log with, for the same reason: a commit body
/// may contain any arrangement of newlines, and a field separator git will not produce is
/// the only way to tell where one ends.
/// ```
/// use majordomus_cli::commit::plan::messages_in;
/// // a range git cannot resolve is None, not an empty history
/// let plain = tempfile::tempdir().expect("a temporary directory");
/// assert!(messages_in(plain.path(), "HEAD").is_none());
/// ```
pub fn messages_in(root: &Path, range: &str) -> Option<Vec<(String, String)>> {
    const FIELD: &str = "\u{1}";
    const RECORD: &str = "\u{2}";
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["log", &format!("--format=%H{FIELD}%B{RECORD}"), range])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .split(RECORD)
            .filter_map(|record| {
                let mut parts = record.trim_start_matches('\n').splitn(2, FIELD);
                let commit = parts.next()?.trim();
                if commit.is_empty() {
                    return None;
                }
                Some((
                    commit.to_string(),
                    parts.next().unwrap_or_default().trim_end().to_string(),
                ))
            })
            .collect(),
    )
}

#[cfg(test)]
mod derived_tests {
    use super::*;
    use crate::commit::scopes::ScopeUse;

    fn vocabulary(rows: &[(&str, &str, usize)]) -> ScopeVocabulary {
        let mut v = ScopeVocabulary::default();
        for (scope, prefix, n) in rows {
            v.learn(scope, prefix, *n);
            if !v.knows(scope) {
                v.scopes.push(ScopeUse {
                    scope: scope.to_string(),
                    commits: *n,
                    directories: vec![prefix.to_string()],
                });
            }
        }
        v
    }

    /// The defect this repository's own change set found on the planner's first real run: it
    /// proposed a commit of twenty-three generated files, which `project.derived-files-
    /// regenerated` forbids and the `derived-current` hook would have refused one commit at
    /// a time, after the split.
    #[test]
    fn a_change_set_carrying_generated_files_is_one_commit_and_says_why() {
        let v = vocabulary(&[
            ("alpha", "apps/cli/src", 40),
            ("derive", "docs/generated", 180),
        ]);
        let paths: Vec<String> = vec![
            "apps/cli/src/alpha/a.rs".into(),
            "docs/generated/registry.json".into(),
            "docs/generated/cli.md".into(),
        ];
        let derived: Vec<String> = vec![
            "docs/generated/registry.json".into(),
            "docs/generated/cli.md".into(),
        ];
        // Without the rule the scopes alone would make two commits.
        assert_eq!(group_by_scope(&paths, &v).len(), 2);
        let groups = group(&paths, &v, &derived);
        assert_eq!(
            groups.len(),
            1,
            "generated files may not be split from their source"
        );
        assert_eq!(groups[0].paths.len(), 3);
        assert!(
            groups[0].rationale.contains("one commit, not 2"),
            "{}",
            groups[0].rationale
        );
        assert!(
            groups[0].rationale.contains("derived-files-regenerated"),
            "the rationale names the rule that decided: {}",
            groups[0].rationale
        );
        // and the scope is the source's, not the generated tree's
        assert_eq!(groups[0].message.header.scope.as_deref(), Some("alpha"));
    }

    #[test]
    fn a_change_set_that_would_be_one_commit_anyway_is_not_relabelled() {
        let v = vocabulary(&[("alpha", "apps/cli/src", 40)]);
        let paths: Vec<String> = vec![
            "apps/cli/src/alpha/a.rs".into(),
            "apps/cli/src/alpha/b.rs".into(),
        ];
        let groups = group(&paths, &v, &["apps/cli/src/alpha/b.rs".into()]);
        assert_eq!(groups.len(), 1);
        assert!(
            !groups[0].rationale.contains("one commit, not"),
            "nothing was collapsed, so nothing should claim it was: {}",
            groups[0].rationale
        );
    }

    #[test]
    fn a_change_set_with_no_generated_file_splits_as_the_history_says() {
        let v = vocabulary(&[("alpha", "apps/cli/src", 40), ("site", "site/content", 30)]);
        let paths: Vec<String> = vec!["apps/cli/src/alpha/a.rs".into(), "site/content/b.md".into()];
        assert_eq!(group(&paths, &v, &[]).len(), 2);
    }
}
