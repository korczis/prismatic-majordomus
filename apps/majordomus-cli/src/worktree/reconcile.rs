//! What becomes of every branch and every worktree: one state each, one next step each, and
//! the proven ones carried out.
//!
//! # The problem this exists for
//!
//! `worktree create` is one command and nothing was its inverse. On 2026-10-10 this
//! repository held 274 registered worktrees and 297 local branches, and three readings said
//! why none of them went away:
//!
//! * `worktree cleanup` nominates a branch only when the trunk reaches it *and* its worktree
//!   is clean. Sixty-four of the eighty-one merged branches held a dirty worktree, so they
//!   were never nominated, and nothing said what the dirt was waiting for.
//! * Its reclaim refuses a branch whose upstream is gone — which is the state of every
//!   branch a forge deleted after merging it. The usual landed branch was the refused one.
//! * A branch the trunk does not reach had no state at all. Twenty-three merged to a tree
//!   identical to the trunk's, 125 conflicted with it, and both read as "unmerged", the same
//!   word a branch somebody is working on right now reads as.
//!
//! So the count grew from every normal action and shrank from none, and what was safe to
//! remove could only be found by a person measuring each one by hand.
//!
//! # What this decides
//!
//! Every non-trunk branch and every detached worktree is one [`ReconcileEntry`] with one
//! [`WorkState`] and one [`ReconcileStep`], decided by [`decide`] from [`Readings`] taken
//! from git and from the kernel — never from an age, a name or a timestamp. The decision is
//! a pure function, so the listing, the re-measurement before an act and the tests all run
//! the same code.
//!
//! # What this does, and what it never does
//!
//! [`WorktreeService::reconcile`] carries out only the steps whose proof is complete: it
//! removes the clean worktree of a branch the trunk already contains, and deletes a branch
//! whose every commit a remote holds and whose merge would change nothing. Each subject is
//! measured again immediately before it goes, a worktree is removed without `--force`, and
//! a branch is deleted by `git update-ref -d` naming the commit that was judged, so a branch
//! that moved in between is refused rather than lost. It never removes uncommitted work, a
//! worktree a process is working in, a branch holding a commit no remote has, the worktree
//! the call came from, or a session's scratch checkout unless asked to.
//!
//! ```
//! use majordomus_cli::worktree::reconcile::{decide, Readings, ReconcileStep, TrunkRelation, WorkState};
//!
//! // a branch the trunk contains, published once, its worktree clean and idle
//! let landed = Readings {
//!     checked_out: true,
//!     occupied: Some(false),
//!     clean: Some(true),
//!     relation: Some(TrunkRelation::Landed),
//!     unpublished: Some(false),
//!     ever_published: true,
//!     ..Readings::default()
//! };
//! let decision = decide(&landed);
//! assert_eq!(decision.state, WorkState::Merged);
//! assert_eq!(decision.step, ReconcileStep::RemoveWorktreeAndDeleteBranch);
//!
//! // the same branch with one file nobody committed is kept, whatever else is true
//! let dirty = Readings { clean: Some(false), ..landed };
//! assert_eq!(decide(&dirty).step, ReconcileStep::Commit);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::Result;
use super::git;
use super::model::Standing;
use super::service::{display, Detail, WorktreeService};
use super::state;

/// The `schema` every reconciliation document carries.
pub const RECONCILE_SCHEMA: &str = "majordomus/worktree-reconciliation/v1";

// ---------------------------------------------------------------- vocabulary

/// How a commit stands against the trunk, read from git and from nothing else.
///
/// ```
/// use majordomus_cli::worktree::reconcile::TrunkRelation;
///
/// // the word a document carries; renaming a variant must not rename it
/// assert_eq!(TrunkRelation::Equivalent.as_str(), "equivalent");
/// assert_eq!(serde_json::to_string(&TrunkRelation::Landed).unwrap(), "\"landed\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrunkRelation {
    /// The trunk reaches the commit: it is in the repository's own history.
    Landed,
    /// The trunk does not reach it, and merging it would produce the trunk's own tree: every
    /// change it carries arrived by another commit.
    Equivalent,
    /// Merging it changes the trunk and conflicts on nothing.
    Mergeable,
    /// Merging it conflicts with the trunk.
    Conflicting,
}

impl TrunkRelation {
    /// The word this relation is reported under — the same word its serialisation carries.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::TrunkRelation;
    ///
    /// for r in [TrunkRelation::Landed, TrunkRelation::Equivalent, TrunkRelation::Mergeable, TrunkRelation::Conflicting] {
    ///     assert_eq!(serde_json::to_string(&r).unwrap(), format!("\"{}\"", r.as_str()));
    /// }
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            TrunkRelation::Landed => "landed",
            TrunkRelation::Equivalent => "equivalent",
            TrunkRelation::Mergeable => "mergeable",
            TrunkRelation::Conflicting => "conflicting",
        }
    }
}

/// The one state a branch or a detached worktree is in.
///
/// Declared in the order [`decide`] asks its questions, which is the order of what must not
/// be lost: a process, then files, then commits, then a name.
///
/// ```
/// use majordomus_cli::worktree::reconcile::WorkState;
///
/// assert_eq!(WorkState::Unpublished.as_str(), "unpublished");
/// assert_eq!(serde_json::to_string(&WorkState::Equivalent).unwrap(), "\"equivalent\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    /// A process has its working directory inside the worktree: somebody is here.
    Active,
    /// The worktree holds files no commit carries.
    Dirty,
    /// A reading could not be taken, so nothing is decided: not knowing is not permission.
    Unreadable,
    /// The trunk reaches the branch and it was never published: a branch somebody cut and
    /// has not committed to yet, which reads exactly like one that landed.
    Unstarted,
    /// The trunk reaches it and it was published: its work is in the repository's history.
    Merged,
    /// Its commits are not the trunk's, and merging them changes nothing.
    Equivalent,
    /// It holds a commit no remote-tracking ref reaches.
    Unpublished,
    /// A detached worktree whose commit no ref holds: the worktree is its only name.
    Orphaned,
    /// Merging it conflicts with the trunk.
    Conflicted,
    /// It merges cleanly and the trunk has moved since it left.
    Stale,
    /// It merges cleanly and the trunk has not moved: nothing stands before integration.
    Ready,
}

impl WorkState {
    /// The word this state is reported under — the same word its serialisation carries.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::WorkState;
    ///
    /// for s in [WorkState::Active, WorkState::Dirty, WorkState::Unreadable, WorkState::Unstarted,
    ///           WorkState::Merged, WorkState::Equivalent, WorkState::Unpublished, WorkState::Orphaned,
    ///           WorkState::Conflicted, WorkState::Stale, WorkState::Ready] {
    ///     assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.as_str()));
    /// }
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkState::Active => "active",
            WorkState::Dirty => "dirty",
            WorkState::Unreadable => "unreadable",
            WorkState::Unstarted => "unstarted",
            WorkState::Merged => "merged",
            WorkState::Equivalent => "equivalent",
            WorkState::Unpublished => "unpublished",
            WorkState::Orphaned => "orphaned",
            WorkState::Conflicted => "conflicted",
            WorkState::Stale => "stale",
            WorkState::Ready => "ready",
        }
    }
}

/// The one step a state permits.
///
/// ```
/// use majordomus_cli::worktree::reconcile::ReconcileStep;
///
/// // only three steps remove anything, and they are the only ones `reconcile --apply` takes
/// assert!(ReconcileStep::RemoveWorktree.removes());
/// assert!(ReconcileStep::DeleteBranch.removes());
/// assert!(!ReconcileStep::Publish.removes() && !ReconcileStep::Keep.removes());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileStep {
    /// Nothing to do, or nothing that may be done.
    Keep,
    /// Commit what the worktree holds: nothing else can carry it.
    Commit,
    /// Push the branch: its commits are on this disk only.
    Publish,
    /// Give the commit a branch: a detached worktree is its only name.
    Name,
    /// Remove the worktree and keep the branch.
    RemoveWorktree,
    /// Delete the branch, which has no worktree.
    DeleteBranch,
    /// Remove the worktree, then delete the branch.
    RemoveWorktreeAndDeleteBranch,
    /// Bring the trunk into the branch before it is integrated.
    Refresh,
    /// Integrate it: nothing stands in the way.
    Integrate,
    /// Resolve its conflict with the trunk, or decide it is superseded.
    Resolve,
}

impl ReconcileStep {
    /// The word this step is reported under — the same word its serialisation carries.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::ReconcileStep;
    ///
    /// assert_eq!(ReconcileStep::RemoveWorktreeAndDeleteBranch.as_str(), "remove_worktree_and_delete_branch");
    /// let json = serde_json::to_string(&ReconcileStep::Refresh).unwrap();
    /// assert_eq!(json, format!("\"{}\"", ReconcileStep::Refresh.as_str()));
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            ReconcileStep::Keep => "keep",
            ReconcileStep::Commit => "commit",
            ReconcileStep::Publish => "publish",
            ReconcileStep::Name => "name",
            ReconcileStep::RemoveWorktree => "remove_worktree",
            ReconcileStep::DeleteBranch => "delete_branch",
            ReconcileStep::RemoveWorktreeAndDeleteBranch => "remove_worktree_and_delete_branch",
            ReconcileStep::Refresh => "refresh",
            ReconcileStep::Integrate => "integrate",
            ReconcileStep::Resolve => "resolve",
        }
    }

    /// This step removes a worktree, a branch or both.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::ReconcileStep;
    ///
    /// assert!(ReconcileStep::RemoveWorktreeAndDeleteBranch.removes());
    /// assert!(!ReconcileStep::Refresh.removes());
    /// ```
    pub fn removes(&self) -> bool {
        self.removes_worktree() || self.deletes_branch()
    }

    /// This step removes a worktree, with or without its branch: the half of an act that
    /// goes through `git worktree remove` and is refused for uncommitted work.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::ReconcileStep;
    ///
    /// assert!(ReconcileStep::RemoveWorktree.removes_worktree());
    /// assert!(!ReconcileStep::DeleteBranch.removes_worktree());
    /// ```
    pub fn removes_worktree(&self) -> bool {
        matches!(
            self,
            ReconcileStep::RemoveWorktree | ReconcileStep::RemoveWorktreeAndDeleteBranch
        )
    }

    /// This step deletes a branch, with or without a worktree first: the half of an act
    /// that is a compare-and-delete on the commit that was judged.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::ReconcileStep;
    ///
    /// assert!(ReconcileStep::DeleteBranch.deletes_branch());
    /// assert!(!ReconcileStep::RemoveWorktree.deletes_branch());
    /// ```
    pub fn deletes_branch(&self) -> bool {
        matches!(
            self,
            ReconcileStep::DeleteBranch | ReconcileStep::RemoveWorktreeAndDeleteBranch
        )
    }
}

// ---------------------------------------------------------------- the decision

/// Everything [`decide`] is allowed to know about one subject.
///
/// An `Option` that is `None` is a reading that could not be taken. It is never treated as
/// the harmless answer: a worktree whose occupancy is unknown is not unoccupied.
///
/// ```
/// use majordomus_cli::worktree::reconcile::{decide, Readings, WorkState};
///
/// // nothing read at all decides nothing
/// assert_eq!(decide(&Readings::default()).state, WorkState::Unreadable);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Readings {
    /// The subject is a detached worktree rather than a branch.
    pub detached: bool,
    /// A worktree holds it.
    pub checked_out: bool,
    /// A process has its working directory inside the worktree.
    pub occupied: Option<bool>,
    /// The worktree holds no uncommitted work.
    pub clean: Option<bool>,
    /// How its commit stands against the trunk.
    pub relation: Option<TrunkRelation>,
    /// Its commit is on no remote-tracking ref.
    pub unpublished: Option<bool>,
    /// The branch has, or had, a remote counterpart of its own name: such an upstream is
    /// configured, gone or not, or a remote-tracking ref carries its name. The trunk's
    /// remote as an upstream does not count: git gives it to every branch cut from it.
    pub ever_published: bool,
    /// Some ref holds a detached worktree's commit.
    pub held_by_a_ref: bool,
    /// How many trunk commits it lacks.
    pub behind: Option<u64>,
}

/// What [`decide`] concluded, with the readings that decided it in words.
///
/// ```
/// use majordomus_cli::worktree::reconcile::{decide, Decision, Readings, ReconcileStep, WorkState};
///
/// // a decision always says why: the reasons are what `reconcile <branch>` prints
/// let unread: Decision = decide(&Readings::default());
/// assert_eq!((unread.state, unread.step), (WorkState::Unreadable, ReconcileStep::Keep));
/// assert!(!unread.reasons.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The state.
    pub state: WorkState,
    /// The step that state permits.
    pub step: ReconcileStep,
    /// Why, one reading per line.
    pub reasons: Vec<String>,
}

fn decision(state: WorkState, step: ReconcileStep, reasons: &[&str]) -> Decision {
    Decision {
        state,
        step,
        reasons: reasons.iter().map(|r| (*r).to_string()).collect(),
    }
}

/// Decide one subject from its readings. Pure: no git, no filesystem, no clock.
///
/// The questions are asked in the order of what cannot be recovered: a process working in
/// the worktree, files no commit carries, a reading that failed, and only then where the
/// commits stand.
///
/// ```
/// use majordomus_cli::worktree::reconcile::{decide, Readings, ReconcileStep, TrunkRelation, WorkState};
///
/// let base = Readings { occupied: Some(false), clean: Some(true), unpublished: Some(false), ..Readings::default() };
///
/// // a branch cut from the trunk a minute ago reads as landed; it was never published, so it stays
/// let fresh = Readings { checked_out: true, relation: Some(TrunkRelation::Landed), ..base.clone() };
/// assert_eq!(decide(&fresh).state, WorkState::Unstarted);
/// assert_eq!(decide(&fresh).step, ReconcileStep::Keep);
///
/// // a branch whose changes all arrived another way, with every commit on a remote
/// let duplicate = Readings { relation: Some(TrunkRelation::Equivalent), ever_published: true, ..base.clone() };
/// assert_eq!(decide(&duplicate).step, ReconcileStep::DeleteBranch);
///
/// // a process inside outranks everything
/// let busy = Readings { checked_out: true, occupied: Some(true), ..duplicate };
/// assert_eq!(decide(&busy).state, WorkState::Active);
/// ```
pub fn decide(r: &Readings) -> Decision {
    if r.checked_out {
        match (r.occupied, r.clean) {
            (Some(true), _) => {
                return decision(
                    WorkState::Active,
                    ReconcileStep::Keep,
                    &["a process has its working directory inside the worktree"],
                )
            }
            (_, Some(false)) => {
                return decision(
                    WorkState::Dirty,
                    ReconcileStep::Commit,
                    &["the worktree holds files no commit carries"],
                )
            }
            (None, _) => {
                return decision(
                    WorkState::Unreadable,
                    ReconcileStep::Keep,
                    &["cannot tell whether a process is working inside the worktree"],
                )
            }
            (_, None) => {
                return decision(
                    WorkState::Unreadable,
                    ReconcileStep::Keep,
                    &["the worktree's uncommitted work could not be read"],
                )
            }
            (Some(false), Some(true)) => {}
        }
    }
    let Some(relation) = r.relation else {
        return decision(
            WorkState::Unreadable,
            ReconcileStep::Keep,
            &["git could not say how the commit stands against the trunk"],
        );
    };
    match relation {
        TrunkRelation::Landed => landed(r),
        TrunkRelation::Equivalent => equivalent(r),
        TrunkRelation::Mergeable | TrunkRelation::Conflicting => unintegrated(r, relation),
    }
}

/// The trunk reaches the commit.
fn landed(r: &Readings) -> Decision {
    if r.detached {
        return decision(
            WorkState::Merged,
            ReconcileStep::RemoveWorktree,
            &[
                "the trunk reaches the commit",
                "the worktree is clean and nothing works inside it",
            ],
        );
    }
    if !r.ever_published {
        return decision(
            WorkState::Unstarted,
            ReconcileStep::Keep,
            &[
                "the trunk reaches the branch",
                "no remote branch ever carried its name: a branch cut and not yet committed to reads the same",
            ],
        );
    }
    if r.checked_out {
        decision(
            WorkState::Merged,
            ReconcileStep::RemoveWorktreeAndDeleteBranch,
            &[
                "the trunk reaches the branch, and it was published",
                "the worktree is clean and nothing works inside it",
            ],
        )
    } else {
        decision(
            WorkState::Merged,
            ReconcileStep::DeleteBranch,
            &[
                "the trunk reaches the branch, and it was published",
                "no worktree holds it",
            ],
        )
    }
}

/// Merging the commit would produce the trunk's own tree.
fn equivalent(r: &Readings) -> Decision {
    let same = "merging it produces the trunk's own tree: every change it carries is already there";
    match r.unpublished {
        None => decision(
            WorkState::Unreadable,
            ReconcileStep::Keep,
            &[same, "cannot tell whether a remote holds its commits"],
        ),
        Some(true) if r.detached && !r.held_by_a_ref => orphaned(),
        Some(true) if r.checked_out => decision(
            WorkState::Equivalent,
            ReconcileStep::RemoveWorktree,
            &[
                same,
                "its commits are on this disk only, so the branch is kept",
                "the worktree is clean and nothing works inside it",
            ],
        ),
        Some(true) => decision(
            WorkState::Equivalent,
            ReconcileStep::Keep,
            &[
                same,
                "its commits are on this disk only, so the branch is kept",
            ],
        ),
        Some(false) if r.detached => decision(
            WorkState::Equivalent,
            ReconcileStep::RemoveWorktree,
            &[same, "a remote-tracking ref reaches the commit"],
        ),
        Some(false) if r.checked_out => decision(
            WorkState::Equivalent,
            ReconcileStep::RemoveWorktreeAndDeleteBranch,
            &[
                same,
                "a remote-tracking ref reaches every commit of it",
                "the worktree is clean and nothing works inside it",
            ],
        ),
        Some(false) => decision(
            WorkState::Equivalent,
            ReconcileStep::DeleteBranch,
            &[
                same,
                "a remote-tracking ref reaches every commit of it",
                "no worktree holds it",
            ],
        ),
    }
}

/// A detached worktree is the only name its commit has.
fn orphaned() -> Decision {
    decision(
        WorkState::Orphaned,
        ReconcileStep::Name,
        &[
            "no ref holds the commit this detached worktree is at",
            "removing the worktree would leave it reachable from nothing",
        ],
    )
}

/// The commit carries changes the trunk lacks.
fn unintegrated(r: &Readings, relation: TrunkRelation) -> Decision {
    match r.unpublished {
        None => {
            return decision(
                WorkState::Unreadable,
                ReconcileStep::Keep,
                &["cannot tell whether a remote holds its commits"],
            )
        }
        Some(true) if r.detached && !r.held_by_a_ref => return orphaned(),
        Some(true) => {
            return decision(
                WorkState::Unpublished,
                ReconcileStep::Publish,
                &["it holds a commit no remote-tracking ref reaches"],
            )
        }
        Some(false) => {}
    }
    if relation == TrunkRelation::Conflicting {
        return decision(
            WorkState::Conflicted,
            ReconcileStep::Resolve,
            &[
                "merging it conflicts with the trunk on an authored file",
                "a remote-tracking ref reaches every commit of it",
            ],
        );
    }
    match r.behind {
        Some(0) => decision(
            WorkState::Ready,
            ReconcileStep::Integrate,
            &[
                "it merges into the trunk without a conflict",
                "the trunk has not moved since it left",
            ],
        ),
        _ => decision(
            WorkState::Stale,
            ReconcileStep::Refresh,
            &[
                "it merges into the trunk without a conflict",
                "the trunk has moved since it left",
            ],
        ),
    }
}

// ---------------------------------------------------------------- the documents

/// One branch or one detached worktree, decided.
///
/// ```
/// use majordomus_cli::worktree::reconcile::{ReconcileEntry, ReconcileStep, WorkState};
///
/// let entry = ReconcileEntry {
///     branch: Some("feature/x".into()),
///     worktree: None,
///     head: "0123456789abcdef".into(),
///     state: WorkState::Merged,
///     step: ReconcileStep::DeleteBranch,
///     automatic: true,
///     scratch: false,
///     here: false,
///     relation: None,
///     behind: None,
///     reasons: vec!["the trunk reaches the branch, and it was published".into()],
///     command: "majordomus worktree reconcile --apply".into(),
/// };
/// // a branch is named by its name, a detached worktree by its path
/// assert_eq!(entry.subject(), "feature/x");
/// // and a field nothing was read for is left out of the document
/// assert!(!serde_json::to_string(&entry).unwrap().contains("behind"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReconcileEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch. Absent for a detached worktree.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The worktree holding it, when one does.
    pub worktree: Option<String>,
    /// The commit that was judged. An act names it, so a subject that moved is refused.
    pub head: String,
    /// The state.
    pub state: WorkState,
    /// The step that state permits.
    pub step: ReconcileStep,
    /// `reconcile --apply` takes this step on its own: it removes something, the proof is
    /// complete, and the subject is neither a scratch checkout nor where the call came from.
    pub automatic: bool,
    /// A session's scratch checkout or a detached worktree: whoever made it removes it, and
    /// `--apply` leaves it unless `--include-scratch` is given.
    pub scratch: bool,
    /// The call came from inside this worktree. It is never removed from inside, whatever
    /// else is true of it.
    pub here: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// How the commit stands against the trunk, when git could say.
    pub relation: Option<TrunkRelation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// How many trunk commits it lacks, read when it merges cleanly.
    pub behind: Option<u64>,
    /// Why: the readings that decided it, one per line.
    pub reasons: Vec<String>,
    /// The command that takes the step.
    pub command: String,
}

impl ReconcileEntry {
    /// How this entry is named: its branch, or the path of its detached worktree.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::{ReconcileEntry, ReconcileStep, WorkState};
    ///
    /// let detached = ReconcileEntry {
    ///     branch: None,
    ///     worktree: Some("/srv/repo-wt/scratch".into()),
    ///     head: "abc".into(),
    ///     state: WorkState::Orphaned,
    ///     step: ReconcileStep::Name,
    ///     automatic: false,
    ///     scratch: true,
    ///     here: false,
    ///     relation: None,
    ///     behind: None,
    ///     reasons: vec![],
    ///     command: String::new(),
    /// };
    /// assert_eq!(detached.subject(), "/srv/repo-wt/scratch");
    /// ```
    pub fn subject(&self) -> &str {
        self.branch
            .as_deref()
            .or(self.worktree.as_deref())
            .unwrap_or("")
    }
}

/// One collection, one order, declared once: what `--apply` would take first, then by state
/// in the order [`decide`] asks, then by name.
impl crate::order::Ordered for ReconcileEntry {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let group = if self.automatic { "automatic" } else { "held" };
        crate::order::OrderKey::grouped(group, self.subject(), self.subject())
            .ranked(self.state as i64)
    }
}

/// Every branch and detached worktree of the repository, decided, with the one verdict over
/// them.
///
/// ```
/// use majordomus_cli::worktree::reconcile::{Reconciliation, RECONCILE_SCHEMA};
///
/// let none = Reconciliation {
///     schema: RECONCILE_SCHEMA.into(),
///     repository: "/srv/repo".into(),
///     trunk: Some("master".into()),
///     base: Some("origin/master".into()),
///     entries: vec![],
///     tallies: Default::default(),
///     automatic: 0,
///     settled: true,
/// };
/// // a second run after an apply reads exactly this
/// assert_eq!(none.summary(), "nothing to reconcile: 0 subject(s), none removable on proof");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reconciliation {
    /// [`RECONCILE_SCHEMA`].
    pub schema: String,
    /// The primary checkout this was measured from.
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The trunk, when there is one.
    pub trunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The ref every relation was decided against: the remote-tracking branch the trunk
    /// follows when it has one, so a primary checkout nobody pulled hides nothing.
    pub base: Option<String>,
    /// Every subject: what `--apply` would take first, then by state, then by name.
    pub entries: Vec<ReconcileEntry>,
    /// How many subjects are in each state, by its word.
    pub tallies: BTreeMap<String, usize>,
    /// How many steps `--apply` would take.
    pub automatic: usize,
    /// Nothing is removable on proof: a second `--apply` would do nothing.
    pub settled: bool,
}

impl Reconciliation {
    /// The one line a person reads first.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::{Reconciliation, RECONCILE_SCHEMA};
    ///
    /// let mut r = Reconciliation {
    ///     schema: RECONCILE_SCHEMA.into(),
    ///     repository: "/srv/repo".into(),
    ///     trunk: None,
    ///     base: None,
    ///     entries: vec![],
    ///     tallies: Default::default(),
    ///     automatic: 3,
    ///     settled: false,
    /// };
    /// assert!(r.summary().starts_with("3 of 0 subject(s) removable on proof"));
    /// r.automatic = 0;
    /// r.settled = true;
    /// assert!(r.summary().starts_with("nothing to reconcile"));
    /// ```
    pub fn summary(&self) -> String {
        if self.settled {
            return format!(
                "nothing to reconcile: {} subject(s), none removable on proof",
                self.entries.len()
            );
        }
        format!(
            "{} of {} subject(s) removable on proof: majordomus worktree reconcile --apply",
            self.automatic,
            self.entries.len()
        )
    }
}

/// A branch `reconcile --apply` deleted, with the commit it pointed at so that it can be
/// made again.
///
/// ```
/// use majordomus_cli::worktree::reconcile::DeletedBranch;
///
/// let gone = DeletedBranch { branch: "feature/x".into(), head: "0123abc".into() };
/// assert_eq!(gone.restore(), "git branch feature/x 0123abc");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeletedBranch {
    /// The branch.
    pub branch: String,
    /// The commit it pointed at.
    pub head: String,
}

impl DeletedBranch {
    /// The command that makes the branch again.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::DeletedBranch;
    ///
    /// let gone = DeletedBranch { branch: "fix/y".into(), head: "feed".into() };
    /// assert!(gone.restore().ends_with("fix/y feed"));
    /// ```
    pub fn restore(&self) -> String {
        format!("git branch {} {}", self.branch, self.head)
    }
}

/// A subject `reconcile --apply` listed and did not act on, with why.
///
/// ```
/// use majordomus_cli::worktree::reconcile::ReconcileRefusal;
///
/// let r = ReconcileRefusal { subject: "feature/x".into(), reason: "it moved".into() };
/// assert_eq!(serde_json::to_value(&r).unwrap()["reason"], "it moved");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReconcileRefusal {
    /// The branch, or the detached worktree's path.
    pub subject: String,
    /// Why it was left.
    pub reason: String,
}

/// What `reconcile --apply` did: the worktrees it removed, the branches it deleted with the
/// commit each pointed at, and every subject it listed and then left, with the reason. An
/// act that found nothing to do answers in the same shape, with three empty lists.
///
/// ```
/// use majordomus_cli::worktree::reconcile::ReconcileOutcome;
///
/// let nothing = ReconcileOutcome::default();
/// // an apply that found nothing to do says so in the same shape as one that did
/// assert!(nothing.removed.is_empty() && nothing.deleted.is_empty() && nothing.refused.is_empty());
/// assert_eq!(nothing.summary(), "worktree reconcile: 0 worktree(s) removed, 0 branch(es) deleted, 0 refused");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReconcileOutcome {
    /// The worktrees removed, by path.
    pub removed: Vec<String>,
    /// The branches deleted, each with the commit it pointed at.
    pub deleted: Vec<DeletedBranch>,
    /// What was listed and left, with why.
    pub refused: Vec<ReconcileRefusal>,
}

impl ReconcileOutcome {
    /// The one line that closes the report.
    ///
    /// ```
    /// use majordomus_cli::worktree::reconcile::{DeletedBranch, ReconcileOutcome};
    ///
    /// let outcome = ReconcileOutcome {
    ///     removed: vec!["/srv/repo-wt/feature/x".into()],
    ///     deleted: vec![DeletedBranch { branch: "feature/x".into(), head: "abc".into() }],
    ///     refused: vec![],
    /// };
    /// assert!(outcome.summary().contains("1 worktree(s) removed, 1 branch(es) deleted"));
    /// ```
    pub fn summary(&self) -> String {
        format!(
            "worktree reconcile: {} worktree(s) removed, {} branch(es) deleted, {} refused",
            self.removed.len(),
            self.deleted.len(),
            self.refused.len()
        )
    }
}

/// What `reconcile --apply` may reach beyond the default.
///
/// ```
/// use majordomus_cli::worktree::reconcile::ReconcileOptions;
///
/// // by default a scratch checkout is listed and left to whoever made it
/// assert!(!ReconcileOptions::default().include_scratch);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconcileOptions {
    /// Also remove scratch checkouts and detached worktrees whose proof is complete.
    pub include_scratch: bool,
}

// ---------------------------------------------------------------- the measurement

/// What one reconciliation reads once and every subject is judged against.
struct Shared {
    primary: PathBuf,
    /// The ref relations are decided against, and its tree.
    base: Option<(String, String)>,
    /// Commits on a local branch and on no remote-tracking ref.
    unpublished: BTreeSet<String>,
    /// The short names remote-tracking refs carry, remote removed.
    remote_names: BTreeSet<String>,
    /// Every working directory a process on this machine has, or `None` without `lsof`.
    working_directories: Option<Vec<String>>,
}

/// One thing to judge, before any reading of it.
#[derive(Debug, Clone)]
struct Subject {
    branch: Option<String>,
    worktree: Option<PathBuf>,
    head: String,
    /// An upstream of the branch's own name is configured, gone or not.
    own_upstream: bool,
    scratch: bool,
    current: bool,
}

impl WorktreeService {
    /// Every non-trunk branch and every detached worktree, decided.
    ///
    /// Reads git and the kernel and writes nothing: one topology without per-worktree
    /// status, then for each subject its own status, its relation to the trunk and whether
    /// a remote holds it, taken in parallel.
    ///
    /// ```
    /// use std::process::Command;
    /// use majordomus_cli::worktree::reconcile::WorkState;
    /// use majordomus_cli::worktree::WorktreeService;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let repo = dir.path().join("repo");
    /// std::fs::create_dir_all(&repo).unwrap();
    /// let git = |args: &[&str]| {
    ///     let ok = Command::new("git").current_dir(&repo)
    ///         .args(["-c", "user.email=d@example.com", "-c", "user.name=doc", "-c", "commit.gpgsign=false"])
    ///         .args(args).status().unwrap().success();
    ///     assert!(ok, "git {args:?}");
    /// };
    /// git(&["init", "-q", "-b", "master"]);
    /// git(&["commit", "-q", "--allow-empty", "-m", "base"]);
    /// git(&["branch", "feature/cut"]);
    ///
    /// let plan = WorktreeService::open(&repo).unwrap().reconciliation().unwrap();
    /// // a branch cut from the trunk and never published is not offered for removal
    /// assert_eq!(plan.entries.len(), 1);
    /// assert_eq!(plan.entries[0].state, WorkState::Unstarted);
    /// assert!(plan.settled);
    /// ```
    pub fn reconciliation(&self) -> Result<Reconciliation> {
        let topology = self.refreshed()?.topology(Detail::Fast)?;
        let shared = self.shared()?;

        let mut subjects: Vec<Subject> = Vec::new();
        let by_path: BTreeMap<&str, &super::model::WorktreeState> = topology
            .worktrees
            .iter()
            .map(|w| (w.path.as_str(), w))
            .collect();
        for b in topology.branches.iter().filter(|b| !b.trunk) {
            let holder = b.worktree.as_deref().and_then(|p| by_path.get(p));
            subjects.push(Subject {
                branch: Some(b.name.clone()),
                worktree: b
                    .worktree
                    .as_deref()
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir()),
                head: b.head.clone(),
                own_upstream: own_upstream(&b.name, b.upstream.as_ref().map(|u| u.name.as_str())),
                scratch: holder.is_some_and(|h| h.standing == Standing::Ephemeral),
                current: holder.is_some_and(|h| h.current),
            });
        }
        for w in topology
            .worktrees
            .iter()
            .filter(|w| w.detached && w.exists && w.standing != Standing::Primary)
        {
            // a detached worktree git reports no commit for has nothing to judge
            let Some(head) = w.head.clone() else {
                continue;
            };
            subjects.push(Subject {
                branch: None,
                worktree: Some(PathBuf::from(&w.path)),
                head,
                own_upstream: false,
                scratch: true,
                current: w.current,
            });
        }

        let mut entries = in_parallel(&subjects, |s| entry_of(s, &shared));
        crate::order::canonical(&mut entries);

        let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
        for e in &entries {
            *tallies.entry(e.state.as_str().to_string()).or_default() += 1;
        }
        let automatic = entries.iter().filter(|e| e.automatic).count();
        Ok(Reconciliation {
            schema: RECONCILE_SCHEMA.to_string(),
            repository: topology.repository.primary_worktree.clone(),
            trunk: topology.trunk.branch.clone(),
            base: shared.base.as_ref().map(|(name, _)| name.clone()),
            entries,
            tallies,
            automatic,
            settled: automatic == 0,
        })
    }

    /// Carry out the steps whose proof is complete, each measured again immediately before
    /// it is taken.
    ///
    /// The listing and the act are two moments. A commit made, a file written, a process
    /// started or a branch moved between them is seen, because the subject is read again by
    /// the same [`decide`] and acted on only when it reaches the same step at the same
    /// commit. A worktree goes without `--force`; a branch goes by `git update-ref -d`
    /// naming the commit that was judged. Running it twice changes nothing the second time.
    ///
    /// ```
    /// use std::process::Command;
    /// use majordomus_cli::worktree::reconcile::ReconcileOptions;
    /// use majordomus_cli::worktree::WorktreeService;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let repo = dir.path().join("repo");
    /// std::fs::create_dir_all(&repo).unwrap();
    /// let git = |args: &[&str]| {
    ///     let ok = Command::new("git").current_dir(&repo)
    ///         .args(["-c", "user.email=d@example.com", "-c", "user.name=doc", "-c", "commit.gpgsign=false"])
    ///         .args(args).status().unwrap().success();
    ///     assert!(ok, "git {args:?}");
    /// };
    /// git(&["init", "-q", "-b", "master"]);
    /// git(&["commit", "-q", "--allow-empty", "-m", "base"]);
    /// git(&["branch", "feature/cut"]);
    ///
    /// // nothing here is removable on proof, so an apply does nothing, twice
    /// let svc = WorktreeService::open(&repo).unwrap();
    /// for _ in 0..2 {
    ///     let outcome = svc.reconcile(&ReconcileOptions::default()).unwrap();
    ///     assert!(outcome.removed.is_empty() && outcome.deleted.is_empty());
    /// }
    /// ```
    pub fn reconcile(&self, options: &ReconcileOptions) -> Result<ReconcileOutcome> {
        let now = self.refreshed()?;
        let plan = now.reconciliation()?;
        let primary = now.identity().primary_worktree().path.clone();
        let mut outcome = ReconcileOutcome::default();

        for listed in plan.entries.iter().filter(|e| takes(e, options)) {
            let subject = listed.subject().to_string();
            let measured = match now.remeasure(listed) {
                Ok(Some(measured)) => measured,
                Ok(None) => {
                    outcome.refused.push(ReconcileRefusal {
                        subject,
                        reason: "it no longer exists".into(),
                    });
                    continue;
                }
                Err(e) => {
                    outcome.refused.push(ReconcileRefusal {
                        subject,
                        reason: format!("could not be read again: {e}"),
                    });
                    continue;
                }
            };
            if measured.head != listed.head || measured.step != listed.step {
                outcome.refused.push(ReconcileRefusal {
                    subject,
                    reason: format!(
                        "changed since it was listed: now {} ({})",
                        measured.state.as_str(),
                        measured.reasons.first().cloned().unwrap_or_default()
                    ),
                });
                continue;
            }
            if listed.step.removes_worktree() {
                let Some(path) = listed.worktree.as_deref() else {
                    continue;
                };
                match now.remove(path, false) {
                    Ok(report) => outcome.removed.push(report.path),
                    Err(e) => {
                        outcome.refused.push(ReconcileRefusal {
                            subject,
                            reason: format!("{e}"),
                        });
                        continue;
                    }
                }
            }
            if listed.step.deletes_branch() {
                let Some(branch) = listed.branch.as_deref() else {
                    continue;
                };
                // Compare-and-delete: the ref goes only while it still names the commit
                // that was judged.
                let name = format!("refs/heads/{branch}");
                match git::run(
                    &primary,
                    &["update-ref", "-d", name.as_str(), listed.head.as_str()],
                ) {
                    Ok(_) => outcome.deleted.push(DeletedBranch {
                        branch: branch.to_string(),
                        head: listed.head.clone(),
                    }),
                    Err(e) => outcome.refused.push(ReconcileRefusal {
                        subject,
                        reason: format!("{e}"),
                    }),
                }
            }
        }
        Ok(outcome)
    }

    /// This service with git's registrations read again. A service reads them when it is
    /// opened; a worktree added or removed since is exactly what a reconciliation is asked
    /// about.
    fn refreshed(&self) -> Result<WorktreeService> {
        let mut identity = self.identity().clone();
        identity.refresh()?;
        WorktreeService::over(identity)
    }

    /// The readings every subject of one reconciliation is judged against.
    fn shared(&self) -> Result<Shared> {
        let primary = self.identity().primary_worktree().path.clone();
        let base = match self.identity().trunk().branch.as_deref() {
            Some(trunk) => base_of(&primary, trunk)?,
            None => None,
        };
        Ok(Shared {
            unpublished: state::commits_no_remote_reaches(&primary)?,
            remote_names: remote_branch_names(&primary)?,
            working_directories: state::working_directories(),
            base,
            primary,
        })
    }

    /// One listed subject, read again now. `None` when it is gone.
    fn remeasure(&self, listed: &ReconcileEntry) -> Result<Option<ReconcileEntry>> {
        let shared = self.shared()?;
        let subject = match listed.branch.as_deref() {
            Some(name) => {
                let Some(now) = state::branch(&shared.primary, name)? else {
                    return Ok(None);
                };
                Subject {
                    branch: Some(now.name),
                    worktree: now.worktree.filter(|p| p.is_dir()),
                    head: now.head,
                    own_upstream: own_upstream(
                        name,
                        now.upstream.as_ref().map(|u| u.name.as_str()),
                    ),
                    scratch: listed.scratch,
                    current: listed.here,
                }
            }
            None => {
                let Some(path) = listed.worktree.as_deref().map(PathBuf::from) else {
                    return Ok(None);
                };
                if !path.is_dir() {
                    return Ok(None);
                }
                let Some(head) = git::head_of(&path)? else {
                    return Ok(None);
                };
                Subject {
                    branch: None,
                    worktree: Some(path),
                    head,
                    own_upstream: false,
                    scratch: true,
                    current: listed.here,
                }
            }
        };
        Ok(Some(entry_of(&subject, &shared)))
    }
}

/// Is `upstream` a remote branch of `branch`'s own name?
///
/// A branch cut from `origin/master` is given `origin/master` as its upstream by git, so an
/// upstream alone says nothing about whether the branch itself was ever published; an
/// upstream that carries the branch's name does, and still does after the remote deleted it.
fn own_upstream(branch: &str, upstream: Option<&str>) -> bool {
    upstream
        .and_then(|u| u.split_once('/'))
        .is_some_and(|(_, name)| name == branch)
}

/// Does `--apply` take this entry under these options?
fn takes(entry: &ReconcileEntry, options: &ReconcileOptions) -> bool {
    entry.automatic
        || (options.include_scratch && entry.scratch && !entry.here && entry.step.removes())
}

/// Read one subject and decide it.
fn entry_of(subject: &Subject, shared: &Shared) -> ReconcileEntry {
    let relation = shared
        .base
        .as_ref()
        .and_then(|(base, tree)| relation_of(&shared.primary, base, tree, &subject.head));
    let behind = match (relation, shared.base.as_ref()) {
        (Some(TrunkRelation::Mergeable), Some((base, _))) => {
            count(&shared.primary, &format!("{}..{base}", subject.head))
        }
        _ => None,
    };
    let unpublished = match &subject.branch {
        Some(_) => Some(shared.unpublished.contains(&subject.head)),
        None => on_no_remote(&shared.primary, &subject.head),
    };
    let readings = Readings {
        detached: subject.branch.is_none(),
        checked_out: subject.worktree.is_some(),
        occupied: subject.worktree.as_deref().and_then(|p| {
            shared
                .working_directories
                .as_deref()
                .map(|dirs| state::is_occupied(p, dirs))
        }),
        clean: subject
            .worktree
            .as_deref()
            .and_then(|p| state::dirty_state(p).ok())
            .map(|d| d.clean),
        relation,
        unpublished,
        ever_published: subject.own_upstream
            || subject
                .branch
                .as_deref()
                .is_some_and(|b| shared.remote_names.contains(b)),
        held_by_a_ref: subject.branch.is_some() || held_by_a_ref(&shared.primary, &subject.head),
        behind,
    };
    let decided = decide(&readings);
    let worktree = subject.worktree.as_deref().map(display);
    let mut reasons = decided.reasons;
    let removable = decided.step.removes();
    if removable && subject.current {
        reasons.push("the call came from this worktree, which is never removed from inside".into());
    }
    if removable && subject.scratch {
        reasons.push(
            "a scratch checkout: whoever made it removes it, or --include-scratch does".into(),
        );
    }
    ReconcileEntry {
        command: command_for(decided.step, subject.branch.as_deref(), worktree.as_deref()),
        automatic: removable && !subject.current && !subject.scratch,
        scratch: subject.scratch,
        here: subject.current,
        branch: subject.branch.clone(),
        worktree,
        head: subject.head.clone(),
        state: decided.state,
        step: decided.step,
        relation,
        behind,
        reasons,
    }
}

/// The command a person runs to take `step`.
fn command_for(step: ReconcileStep, branch: Option<&str>, worktree: Option<&str>) -> String {
    let name = branch.or(worktree).unwrap_or("");
    match step {
        ReconcileStep::Keep => String::new(),
        ReconcileStep::Commit => match worktree {
            Some(w) => format!("git -C {w} status"),
            None => String::new(),
        },
        ReconcileStep::Publish => format!("git push -u origin {name}"),
        ReconcileStep::Name => match worktree {
            Some(w) => format!("git -C {w} switch -c <branch>"),
            None => String::new(),
        },
        ReconcileStep::RemoveWorktree
        | ReconcileStep::DeleteBranch
        | ReconcileStep::RemoveWorktreeAndDeleteBranch => {
            "majordomus worktree reconcile --apply".to_string()
        }
        ReconcileStep::Refresh => format!("majordomus prs repair {name}"),
        ReconcileStep::Integrate => format!("majordomus prs explain {name}"),
        ReconcileStep::Resolve => format!("majordomus worktree ensure {name}"),
    }
}

/// The ref the trunk's relations are decided against, with its tree: the remote-tracking
/// branch the trunk follows when it has one, the trunk itself otherwise.
fn base_of(primary: &Path, trunk: &str) -> Result<Option<(String, String)>> {
    let upstream = format!("{trunk}@{{upstream}}");
    let followed = git::try_run(
        primary,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            upstream.as_str(),
        ],
    )?;
    let name = match (followed.status, followed.text()) {
        (Some(0), Ok(name)) if !name.is_empty() => name,
        _ => trunk.to_string(),
    };
    let tree = git::try_run(
        primary,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{tree}}"),
        ],
    )?;
    Ok(match (tree.status, tree.text()) {
        (Some(0), Ok(tree)) if !tree.is_empty() => Some((name, tree)),
        _ => None,
    })
}

/// How `head` stands against `base`, or `None` when git could not say.
///
/// Reachability first, in one cheap call. Otherwise `git merge-tree --write-tree` performs
/// the merge in memory — no index, no working tree, the repository's own merge drivers —
/// and answers with the tree it would produce: the base's own tree means every change
/// arrived another way, exit 1 means a conflict.
fn relation_of(primary: &Path, base: &str, base_tree: &str, head: &str) -> Option<TrunkRelation> {
    let reached = git::try_run(primary, &["merge-base", "--is-ancestor", head, base]).ok()?;
    match reached.status {
        Some(0) => return Some(TrunkRelation::Landed),
        Some(1) => {}
        _ => return None,
    }
    let merged = git::try_run(
        primary,
        &["merge-tree", "--write-tree", "--no-messages", base, head],
    )
    .ok()?;
    match merged.status {
        Some(0) => {
            let tree = merged.text().ok()?;
            Some(if tree.lines().next() == Some(base_tree) {
                TrunkRelation::Equivalent
            } else {
                TrunkRelation::Mergeable
            })
        }
        Some(1) => Some(TrunkRelation::Conflicting),
        _ => None,
    }
}

/// `git rev-list --count <range>`, or `None` when git could not count it.
fn count(primary: &Path, range: &str) -> Option<u64> {
    let out = git::try_run(primary, &["rev-list", "--count", range]).ok()?;
    if out.status != Some(0) {
        return None;
    }
    out.text().ok()?.trim().parse().ok()
}

/// Is `commit` on no remote-tracking ref? For a detached worktree, whose commit
/// `rev-list --branches` does not walk.
fn on_no_remote(primary: &Path, commit: &str) -> Option<bool> {
    let out = git::try_run(
        primary,
        &["rev-list", "--max-count=1", commit, "--not", "--remotes"],
    )
    .ok()?;
    if out.status != Some(0) {
        return None;
    }
    Some(!out.text().ok()?.trim().is_empty())
}

/// Does any branch, remote-tracking ref or tag hold `commit`?
fn held_by_a_ref(primary: &Path, commit: &str) -> bool {
    git::try_run(
        primary,
        &[
            "for-each-ref",
            "--count=1",
            "--contains",
            commit,
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )
    .ok()
    .filter(|o| o.status == Some(0))
    .and_then(|o| o.text().ok())
    .is_some_and(|t| !t.trim().is_empty())
}

/// The branch names remote-tracking refs carry, with the remote's own name removed.
fn remote_branch_names(primary: &Path) -> Result<BTreeSet<String>> {
    let out = git::run(
        primary,
        &[
            "for-each-ref",
            "--format=%(refname:lstrip=3)",
            "refs/remotes",
        ],
    )?;
    Ok(out
        .text()?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Apply `f` to every item on the machine's parallelism, answers in input order. Each call
/// waits on git subprocesses, not on this process's CPU.
fn in_parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    if items.len() < 2 {
        return items.iter().map(&f).collect();
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16)
        .min(items.len());
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let tx = tx.clone();
            let next = &next;
            let f = &f;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else {
                    return;
                };
                if tx.send((i, f(item))).is_err() {
                    return;
                }
            });
        }
        // The receiver below ends when the last sender is gone, so this one must go first.
        drop(tx);
        for (i, r) in rx {
            slots[i] = Some(r);
        }
    });
    slots.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A primary checkout on `master` with one file, pushed to a bare origin beside it.
    fn repository() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "master"]);
        write(&repo.join("a.txt"), "one\n");
        git(&repo, &["add", "a.txt"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let origin = root.join("origin.git");
        git(&root, &["init", "-q", "--bare", origin.to_str().unwrap()]);
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&repo, &["push", "-q", "-u", "origin", "master"]);
        (dir, repo)
    }

    /// A branch with one commit changing `file`, in its canonical worktree, pushed.
    fn published_branch(repo: &Path, name: &str, file: &str, text: &str) -> PathBuf {
        let svc = WorktreeService::open(repo).unwrap();
        let path = svc.expected_path_of(name).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        git(
            repo,
            &["worktree", "add", "-q", "-b", name, path.to_str().unwrap()],
        );
        write(&path.join(file), text);
        git(&path, &["add", file]);
        git(&path, &["commit", "-q", "-m", name]);
        git(&path, &["push", "-q", "-u", "origin", name]);
        path
    }

    /// Merge `name` into master with a merge commit and push the trunk.
    fn land(repo: &Path, name: &str) {
        git(repo, &["merge", "-q", "--no-ff", "-m", "land", name]);
        git(repo, &["push", "-q", "origin", "master"]);
    }

    fn entry<'a>(plan: &'a Reconciliation, subject: &str) -> &'a ReconcileEntry {
        plan.entries
            .iter()
            .find(|e| e.subject() == subject)
            .unwrap_or_else(|| panic!("no {subject}: {:#?}", plan.entries))
    }

    fn plan(repo: &Path) -> Reconciliation {
        WorktreeService::open(repo)
            .unwrap()
            .reconciliation()
            .unwrap()
    }

    fn apply(repo: &Path, include_scratch: bool) -> ReconcileOutcome {
        WorktreeService::open(repo)
            .unwrap()
            .reconcile(&ReconcileOptions { include_scratch })
            .unwrap()
    }

    /// The words are the contract: a reader that persists them must see the same ones.
    #[test]
    fn the_words_are_stable() {
        assert_eq!(WorkState::Active.as_str(), "active");
        assert_eq!(WorkState::Unstarted.as_str(), "unstarted");
        assert_eq!(WorkState::Orphaned.as_str(), "orphaned");
        assert_eq!(ReconcileStep::Keep.as_str(), "keep");
        assert_eq!(ReconcileStep::Commit.as_str(), "commit");
        assert_eq!(ReconcileStep::Publish.as_str(), "publish");
        assert_eq!(ReconcileStep::Name.as_str(), "name");
        assert_eq!(ReconcileStep::RemoveWorktree.as_str(), "remove_worktree");
        assert_eq!(ReconcileStep::DeleteBranch.as_str(), "delete_branch");
        assert_eq!(ReconcileStep::Integrate.as_str(), "integrate");
        assert_eq!(ReconcileStep::Resolve.as_str(), "resolve");
        assert_eq!(TrunkRelation::Mergeable.as_str(), "mergeable");
        assert_eq!(TrunkRelation::Conflicting.as_str(), "conflicting");
    }

    /// Every branch of the decision, from readings alone: what is asked first wins, and a
    /// reading that failed never reads as the harmless answer.
    #[test]
    fn the_decision_asks_what_cannot_be_recovered_first() {
        let idle = Readings {
            checked_out: true,
            occupied: Some(false),
            clean: Some(true),
            unpublished: Some(false),
            ever_published: true,
            held_by_a_ref: true,
            ..Readings::default()
        };
        let of = |r: Readings| {
            let d = decide(&r);
            (d.state, d.step)
        };
        use ReconcileStep as S;
        use TrunkRelation as T;
        use WorkState as W;

        // the worktree's own readings come before anything about commits
        let landed = Readings {
            relation: Some(T::Landed),
            ..idle.clone()
        };
        assert_eq!(
            of(Readings {
                occupied: Some(true),
                clean: Some(false),
                ..landed.clone()
            }),
            (W::Active, S::Keep)
        );
        assert_eq!(
            of(Readings {
                clean: Some(false),
                ..landed.clone()
            }),
            (W::Dirty, S::Commit)
        );
        assert_eq!(
            of(Readings {
                occupied: None,
                ..landed.clone()
            }),
            (W::Unreadable, S::Keep)
        );
        assert_eq!(
            of(Readings {
                clean: None,
                ..landed.clone()
            }),
            (W::Unreadable, S::Keep)
        );
        assert_eq!(
            of(Readings {
                relation: None,
                ..idle.clone()
            }),
            (W::Unreadable, S::Keep)
        );

        // landed
        assert_eq!(
            of(landed.clone()),
            (W::Merged, S::RemoveWorktreeAndDeleteBranch)
        );
        assert_eq!(
            of(Readings {
                checked_out: false,
                ..landed.clone()
            }),
            (W::Merged, S::DeleteBranch)
        );
        assert_eq!(
            of(Readings {
                ever_published: false,
                ..landed.clone()
            }),
            (W::Unstarted, S::Keep)
        );
        assert_eq!(
            of(Readings {
                detached: true,
                ever_published: false,
                ..landed.clone()
            }),
            (W::Merged, S::RemoveWorktree)
        );

        // equivalent
        let same = Readings {
            relation: Some(T::Equivalent),
            ..idle.clone()
        };
        assert_eq!(
            of(same.clone()),
            (W::Equivalent, S::RemoveWorktreeAndDeleteBranch)
        );
        assert_eq!(
            of(Readings {
                checked_out: false,
                ..same.clone()
            }),
            (W::Equivalent, S::DeleteBranch)
        );
        assert_eq!(
            of(Readings {
                detached: true,
                ..same.clone()
            }),
            (W::Equivalent, S::RemoveWorktree)
        );
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                ..same.clone()
            }),
            (W::Equivalent, S::RemoveWorktree)
        );
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                checked_out: false,
                ..same.clone()
            }),
            (W::Equivalent, S::Keep)
        );
        assert_eq!(
            of(Readings {
                unpublished: None,
                ..same.clone()
            }),
            (W::Unreadable, S::Keep)
        );

        // a detached worktree whose commit changes nothing is still that commit's only name
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                detached: true,
                held_by_a_ref: false,
                ..same.clone()
            }),
            (W::Orphaned, S::Name)
        );

        // carrying changes the trunk lacks
        let ahead = Readings {
            relation: Some(T::Mergeable),
            behind: Some(0),
            ..idle.clone()
        };
        assert_eq!(of(ahead.clone()), (W::Ready, S::Integrate));
        assert_eq!(
            of(Readings {
                behind: Some(3),
                ..ahead.clone()
            }),
            (W::Stale, S::Refresh)
        );
        assert_eq!(
            of(Readings {
                behind: None,
                ..ahead.clone()
            }),
            (W::Stale, S::Refresh)
        );
        assert_eq!(
            of(Readings {
                relation: Some(T::Conflicting),
                ..ahead.clone()
            }),
            (W::Conflicted, S::Resolve)
        );
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                ..ahead.clone()
            }),
            (W::Unpublished, S::Publish)
        );
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                relation: Some(T::Conflicting),
                ..ahead.clone()
            }),
            (W::Unpublished, S::Publish)
        );
        assert_eq!(
            of(Readings {
                unpublished: Some(true),
                detached: true,
                held_by_a_ref: false,
                ..ahead.clone()
            }),
            (W::Orphaned, S::Name)
        );
        assert_eq!(
            of(Readings {
                unpublished: None,
                ..ahead
            }),
            (W::Unreadable, S::Keep)
        );
    }

    /// The command printed beside a step is the one that takes it.
    #[test]
    fn every_step_names_the_command_that_takes_it() {
        let c = |s| command_for(s, Some("feature/x"), Some("/srv/r-wt/feature/x"));
        assert_eq!(c(ReconcileStep::Keep), "");
        assert_eq!(
            c(ReconcileStep::Commit),
            "git -C /srv/r-wt/feature/x status"
        );
        assert_eq!(c(ReconcileStep::Publish), "git push -u origin feature/x");
        assert_eq!(
            c(ReconcileStep::Name),
            "git -C /srv/r-wt/feature/x switch -c <branch>"
        );
        for s in [
            ReconcileStep::RemoveWorktree,
            ReconcileStep::DeleteBranch,
            ReconcileStep::RemoveWorktreeAndDeleteBranch,
        ] {
            assert_eq!(c(s), "majordomus worktree reconcile --apply");
        }
        assert_eq!(c(ReconcileStep::Refresh), "majordomus prs repair feature/x");
        assert_eq!(
            c(ReconcileStep::Integrate),
            "majordomus prs explain feature/x"
        );
        assert_eq!(
            c(ReconcileStep::Resolve),
            "majordomus worktree ensure feature/x"
        );
        assert_eq!(command_for(ReconcileStep::Commit, Some("b"), None), "");
        assert_eq!(command_for(ReconcileStep::Name, None, None), "");
    }

    /// The whole reading against a real repository: one branch in every state git can put
    /// it in, each decided from what git says about it.
    #[test]
    fn every_subject_is_given_the_state_git_supports() {
        let (_dir, repo) = repository();

        // landed by a merge commit; the forge then deleted its branch
        let landed = published_branch(&repo, "feature/landed", "landed.txt", "x\n");
        land(&repo, "feature/landed");
        git(
            &repo,
            &["push", "-q", "origin", "--delete", "feature/landed"],
        );
        git(&repo, &["fetch", "-q", "--prune", "origin"]);

        // landed, and somebody left a file behind
        let dirty = published_branch(&repo, "feature/dirty", "dirty.txt", "x\n");
        land(&repo, "feature/dirty");
        write(&dirty.join("draft.txt"), "half written\n");

        // the same change reached the trunk by another commit
        let same = published_branch(&repo, "feature/same", "same.txt", "identical\n");
        write(&repo.join("same.txt"), "identical\n");
        git(&repo, &["add", "same.txt"]);
        git(
            &repo,
            &["commit", "-q", "-m", "the same change, another way"],
        );
        git(&repo, &["push", "-q", "origin", "master"]);

        // conflicts with what the trunk says now
        published_branch(&repo, "feature/conflict", "a.txt", "theirs\n");
        write(&repo.join("a.txt"), "ours\n");
        git(
            &repo,
            &["commit", "-q", "-am", "the trunk moves the same line"],
        );
        git(&repo, &["push", "-q", "origin", "master"]);

        // merges cleanly, behind the trunk
        published_branch(&repo, "feature/stale", "stale.txt", "x\n");
        write(&repo.join("b.txt"), "the trunk moves on\n");
        git(&repo, &["add", "b.txt"]);
        git(&repo, &["commit", "-q", "-m", "trunk"]);
        git(&repo, &["push", "-q", "origin", "master"]);

        // merges cleanly, cut from the trunk as it is now
        published_branch(&repo, "feature/ready", "ready.txt", "x\n");

        // a commit nobody else has, and a branch nobody committed to
        git(&repo, &["branch", "feature/local"]);
        let local = repo.parent().unwrap().join("local");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                local.to_str().unwrap(),
                "feature/local",
            ],
        );
        // an empty commit would carry no change, and a branch that changes nothing is
        // equivalent whoever holds it
        write(&local.join("local.txt"), "only here\n");
        git(&local, &["add", "local.txt"]);
        git(&local, &["commit", "-q", "-m", "only here"]);
        git(&repo, &["branch", "feature/cut"]);
        // cut from the remote trunk, which git makes its upstream: still nobody's work yet
        git(
            &repo,
            &["branch", "--track", "feature/tracking", "origin/master"],
        );

        // a detached worktree at a commit no ref holds
        let orphan = repo.parent().unwrap().join("orphan");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                orphan.to_str().unwrap(),
            ],
        );
        write(&orphan.join("orphan.txt"), "nameless\n");
        git(&orphan, &["add", "orphan.txt"]);
        git(&orphan, &["commit", "-q", "-m", "nameless"]);

        let plan = plan(&repo);
        assert_eq!(plan.base.as_deref(), Some("origin/master"));
        let state = |s: &str| (entry(&plan, s).state, entry(&plan, s).step);
        use ReconcileStep as S;
        use WorkState as W;
        assert_eq!(
            state("feature/landed"),
            (W::Merged, S::RemoveWorktreeAndDeleteBranch)
        );
        assert_eq!(state("feature/dirty"), (W::Dirty, S::Commit));
        assert_eq!(
            state("feature/same"),
            (W::Equivalent, S::RemoveWorktreeAndDeleteBranch)
        );
        assert_eq!(state("feature/conflict"), (W::Conflicted, S::Resolve));
        assert_eq!(state("feature/stale"), (W::Stale, S::Refresh));
        assert_eq!(entry(&plan, "feature/stale").behind, Some(1));
        assert_eq!(state("feature/ready"), (W::Ready, S::Integrate));
        assert_eq!(state("feature/local"), (W::Unpublished, S::Publish));
        assert_eq!(state("feature/cut"), (W::Unstarted, S::Keep));
        assert_eq!(state("feature/tracking"), (W::Unstarted, S::Keep));
        assert_eq!(state(&display(&orphan)), (W::Orphaned, S::Name));

        // exactly the two with a complete proof are taken on their own, and they come first
        let automatic: Vec<&str> = plan
            .entries
            .iter()
            .filter(|e| e.automatic)
            .map(|e| e.subject())
            .collect();
        assert_eq!(automatic, vec!["feature/landed", "feature/same"]);
        assert!(plan.entries[0].automatic && plan.entries[1].automatic);
        assert_eq!((plan.automatic, plan.settled), (2, false));
        assert_eq!(plan.tallies.get("merged"), Some(&1));
        assert!(plan.summary().starts_with("2 of 10 subject(s)"));

        // the act removes those two and nothing else, and names what it deleted
        let landed_head = entry(&plan, "feature/landed").head.clone();
        let outcome = apply(&repo, false);
        assert_eq!(outcome.removed, vec![display(&landed), display(&same)]);
        assert_eq!(outcome.deleted.len(), 2);
        assert_eq!(outcome.deleted[0].branch, "feature/landed");
        assert_eq!(outcome.deleted[0].head, landed_head);
        assert!(outcome.refused.is_empty(), "{:#?}", outcome.refused);
        assert!(!landed.exists() && !same.exists());
        assert!(dirty.join("draft.txt").exists(), "uncommitted work stays");
        assert!(orphan.exists() && local.exists());
        let left = git(&repo, &["branch", "--format=%(refname:short)"]);
        assert!(!left.contains("feature/landed") && !left.contains("feature/same"));
        assert!(left.contains("feature/dirty") && left.contains("feature/conflict"));

        // and a second run finds nothing to do
        let again = apply(&repo, false);
        assert_eq!(again, ReconcileOutcome::default());
        assert!(self::plan(&repo).settled);
        // a deleted branch is one command from being back
        git(
            &repo,
            &["branch", "feature/landed", outcome.deleted[0].head.as_str()],
        );
    }

    /// A detached worktree at a trunk commit is a scratch checkout: listed with its step,
    /// left alone by default, removed when asked.
    #[test]
    fn a_scratch_checkout_goes_only_when_asked() {
        let (_dir, repo) = repository();
        let scratch = repo.parent().unwrap().join("scratch");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                scratch.to_str().unwrap(),
            ],
        );
        let listed = plan(&repo);
        let e = entry(&listed, &display(&scratch));
        assert_eq!(
            (e.state, e.step, e.automatic, e.scratch),
            (
                WorkState::Merged,
                ReconcileStep::RemoveWorktree,
                false,
                true
            )
        );
        assert!(e.reasons.iter().any(|r| r.contains("--include-scratch")));
        assert!(listed.settled);

        assert_eq!(apply(&repo, false), ReconcileOutcome::default());
        assert!(scratch.exists());
        let outcome = apply(&repo, true);
        assert_eq!(outcome.removed, vec![display(&scratch)]);
        assert!(outcome.deleted.is_empty() && !scratch.exists());
    }

    /// The worktree the call came from is never removed from inside, however proven.
    #[test]
    fn the_calling_worktree_is_not_removed_from_inside() {
        let (_dir, repo) = repository();
        let landed = published_branch(&repo, "feature/here", "here.txt", "x\n");
        land(&repo, "feature/here");
        let from_inside = WorktreeService::open(&landed).unwrap();
        let listed = from_inside.reconciliation().unwrap();
        let e = entry(&listed, "feature/here");
        assert_eq!(e.step, ReconcileStep::RemoveWorktreeAndDeleteBranch);
        assert!(!e.automatic);
        assert!(e.reasons.iter().any(|r| r.contains("from inside")));
        let outcome = from_inside.reconcile(&ReconcileOptions::default()).unwrap();
        assert_eq!(outcome, ReconcileOutcome::default());
        assert!(landed.exists());
    }

    /// What changes between the listing and the act is seen: the subject is read again and
    /// a different answer is a refusal, never a removal.
    #[test]
    fn a_subject_that_changed_after_the_listing_is_refused() {
        let (_dir, repo) = repository();
        let landed = published_branch(&repo, "feature/moved", "moved.txt", "x\n");
        land(&repo, "feature/moved");
        let svc = WorktreeService::open(&repo).unwrap();
        let listed = svc.reconciliation().unwrap();
        let before = entry(&listed, "feature/moved").clone();
        assert!(before.automatic);

        // a file written after the listing
        write(&landed.join("late.txt"), "written after the listing\n");
        let now = svc.remeasure(&before).unwrap().unwrap();
        assert_eq!(now.state, WorkState::Dirty);
        let outcome = svc.reconcile(&ReconcileOptions::default()).unwrap();
        assert!(outcome.removed.is_empty() && landed.exists());

        // a commit made after the listing moves the head the act would have named
        git(&landed, &["add", "late.txt"]);
        git(&landed, &["commit", "-q", "-m", "after the listing"]);
        let now = svc.remeasure(&before).unwrap().unwrap();
        assert_ne!(now.head, before.head);
        assert_eq!(now.state, WorkState::Unpublished);

        // a branch deleted after the listing is gone, not an error
        git(
            &repo,
            &["worktree", "remove", "--force", landed.to_str().unwrap()],
        );
        git(&repo, &["branch", "-D", "feature/moved"]);
        assert!(svc.remeasure(&before).unwrap().is_none());

        // and a detached worktree whose directory went away likewise
        let scratch = repo.parent().unwrap().join("scratch");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                scratch.to_str().unwrap(),
            ],
        );
        let listed = svc.reconciliation().unwrap();
        let detached = entry(&listed, &display(&scratch)).clone();
        assert!(svc.remeasure(&detached).unwrap().is_some());
        std::fs::remove_dir_all(&scratch).unwrap();
        assert!(svc.remeasure(&detached).unwrap().is_none());
    }

    /// A repository with no remote decides against its own trunk, and nothing in it is
    /// published: a landed branch there reads as one nobody started.
    #[test]
    fn a_repository_with_no_remote_is_judged_against_its_own_trunk() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "master"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        git(&repo, &["branch", "feature/cut"]);
        let listed = plan(&repo);
        assert_eq!(listed.base.as_deref(), Some("master"));
        assert_eq!(entry(&listed, "feature/cut").state, WorkState::Unstarted);
        assert!(base_of(&repo, "no-such-branch").unwrap().is_none());
    }

    /// The small readings answer `None` when git cannot, and never the harmless value.
    #[test]
    fn a_reading_git_cannot_take_is_none() {
        let (_dir, repo) = repository();
        let missing = "1111111111111111111111111111111111111111";
        let tree = git(&repo, &["rev-parse", "master^{tree}"]);
        assert_eq!(relation_of(&repo, "master", &tree, missing), None);
        assert_eq!(count(&repo, &format!("{missing}..master")), None);
        assert_eq!(on_no_remote(&repo, missing), None);
        assert!(!held_by_a_ref(&repo, missing));
        let head = git(&repo, &["rev-parse", "master"]);
        assert_eq!(
            relation_of(&repo, "master", &tree, &head),
            Some(TrunkRelation::Landed)
        );
        assert_eq!(on_no_remote(&repo, &head), Some(false));
        assert!(held_by_a_ref(&repo, &head));
        assert!(remote_branch_names(&repo).unwrap().contains("master"));
    }

    /// An upstream counts as the branch's own only when it carries the branch's name.
    #[test]
    fn an_upstream_of_another_name_is_not_a_publication() {
        assert!(own_upstream("feature/x", Some("origin/feature/x")));
        assert!(!own_upstream("feature/x", Some("origin/master")));
        assert!(!own_upstream("feature/x", Some("feature/x")));
        assert!(!own_upstream("feature/x", None));
    }

    /// Answers come back in input order whatever order the threads finished in.
    #[test]
    fn parallel_answers_keep_their_order() {
        let items: Vec<u64> = (0..50).collect();
        assert_eq!(
            in_parallel(&items, |n| n * 2),
            items.iter().map(|n| n * 2).collect::<Vec<_>>()
        );
        assert_eq!(in_parallel(&[7u64], |n| n + 1), vec![8]);
        assert!(in_parallel(&[] as &[u64], |n| *n).is_empty());
    }
}
