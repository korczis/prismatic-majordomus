//! The typed lifecycle: one vocabulary for a worktree, a branch and a pull request, and the
//! evidence that produced every state.
//!
//! Nothing in here decides anything. It is what [`super::service`] measured, written down so
//! that serde, JSON Schema, OpenAPI, MCP, the command line and the Cockpit all render the
//! same value. Every state below names the measurement that produces it, because a state
//! inferred from a directory name, a branch prefix or a person's memory is exactly the
//! failure this module exists to prevent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `schema` every lifecycle document carries.
pub const SCHEMA: &str = "majordomus/work-lifecycle/v1";

// ---------------------------------------------------------------- the state

/// Where a piece of work stands, from measurement alone.
///
/// The order of the variants is the order the classifier applies them: the first whose
/// evidence holds wins, so a worktree that is both dirty and owned by a live session is
/// [`WorkState::Active`], and one that is both landed and carries unique commits is
/// [`WorkState::Superseded`] rather than [`WorkState::Landed`]. That precedence is the
/// whole point: the dangerous readings must outrank the reassuring ones.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    /// A registration with no usable subject: the directory git registered is gone, git
    /// reports the registration prunable, or the worktree holds no branch at all.
    /// Evidence: `worktree.standing` is `missing`, git's `prunable` field, or a detached
    /// HEAD. Nothing here is owned by anybody, and nothing here is removed automatically
    /// either — a detached HEAD can still hold commits no ref carries.
    Orphaned,
    /// Somebody is working here now. Evidence: a peer of the shared board names this path,
    /// or git holds the worktree locked, or the newest commit is younger than the policy's
    /// `active_within`. An active worktree is never stale, never pruned and never judged
    /// for residue: its dirty state is work in progress, not debris.
    Active,
    /// Work in progress that cannot proceed: an unresolved merge, rebase, cherry-pick,
    /// revert or bisect, or a path in conflict. Evidence: `git status` reported an
    /// operation in progress or a conflicted entry.
    Blocked,
    /// The work landed and the branch still carries something the landing does not.
    /// Evidence: a merge into the trunk names this branch (or its tip is reachable from the
    /// trunk) **and** `git rev-list HEAD --not --remotes=origin` is non-empty, or the tree
    /// is dirty. This is the reading a naive classifier gets wrong — "the pull request is
    /// merged, so the branch is disposable" — and destroying it is the reason this
    /// vocabulary exists. The provenance (which merge, which pull request, which commits
    /// are the residue) travels with the state.
    Superseded,
    /// Commits exist here that are reachable from no ref on origin, and no open pull
    /// request carries them. Evidence: `git rev-list --count HEAD --not --remotes=origin`
    /// is greater than zero. The work is one disk away from being lost.
    Stranded,
    /// An open pull request carries this branch and everything local is on origin.
    /// Evidence: a pull request record in the `open` state whose head is this branch, and
    /// no unique commits.
    Landing,
    /// Unique work is entirely on origin and no pull request carries it yet. Evidence: the
    /// branch is ahead of the trunk, every commit is reachable from origin, and no pull
    /// request names the branch.
    Ready,
    /// Uncommitted work only: nothing unique is committed, nothing is landing, and no
    /// session is live here. Evidence: `git status` is not clean and the unique count is
    /// zero.
    Dirty,
    /// The work landed and left nothing behind. Evidence: a merge into the trunk names the
    /// branch, or the tip is reachable from the trunk; the unique count is zero and the
    /// tree is clean.
    Landed,
    /// Nothing unique, nothing uncommitted, no pull request, no live owner, and no commit
    /// newer than the policy's `worktree_inactive_after`. Evidence: the age of the newest
    /// commit, measured against the one clock.
    Stale,
    /// Preserved on purpose, with the reason recorded in the repository. Evidence: the tip
    /// is reachable from an annotated tag under `refs/tags/archive/` whose message carries
    /// the reason. A rescue tag is *not* an archive: it records no reason, so work under
    /// one stays [`WorkState::Stranded`].
    Archived,
    /// Nothing above applies: the branch is at the trunk, with nothing unique, nothing
    /// uncommitted, and activity inside the inactivity threshold.
    Idle,
}

impl WorkState {
    /// The word this state is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkState::Orphaned => "orphaned",
            WorkState::Active => "active",
            WorkState::Blocked => "blocked",
            WorkState::Superseded => "superseded",
            WorkState::Stranded => "stranded",
            WorkState::Landing => "landing",
            WorkState::Ready => "ready",
            WorkState::Dirty => "dirty",
            WorkState::Landed => "landed",
            WorkState::Stale => "stale",
            WorkState::Archived => "archived",
            WorkState::Idle => "idle",
        }
    }

    /// Every state, in classifier precedence, for the documentation and for the test that
    /// holds the vocabulary complete.
    pub const ALL: &'static [WorkState] = &[
        WorkState::Orphaned,
        WorkState::Active,
        WorkState::Blocked,
        WorkState::Superseded,
        WorkState::Stranded,
        WorkState::Landing,
        WorkState::Ready,
        WorkState::Dirty,
        WorkState::Landed,
        WorkState::Stale,
        WorkState::Archived,
        WorkState::Idle,
    ];

    /// Whether this state means work exists that the canonical integration path does not
    /// hold. The governance rule is stated over exactly this predicate.
    pub fn holds_unconverged_work(self) -> bool {
        matches!(
            self,
            WorkState::Stranded | WorkState::Superseded | WorkState::Blocked | WorkState::Dirty
        )
    }
}

// ---------------------------------------------------------------- pull requests

/// Where a pull request stands on its way to the trunk.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    /// Merged, or closed by a merge commit the trunk carries.
    Landed,
    /// Closed without landing.
    Abandoned,
    /// Still open, but its head is already reachable from the trunk: the content landed by
    /// another route and the pull request is a formality nobody closed.
    Superseded,
    /// Open and marked a draft by its author.
    Draft,
    /// Open, and merging it into the trunk really does conflict — proved locally with
    /// `git merge-tree --write-tree`, never taken from the forge's own `mergeable` field,
    /// which cannot run this repository's `merge=derived` driver and therefore reports a
    /// conflict in every generated file.
    Blocked,
    /// Open, no conflict, and at least one required check has failed.
    Failing,
    /// Open, no conflict, and checks are still running or have not reported.
    AwaitingCi,
    /// Open, no conflict, checks green: nothing but a person stands between it and the
    /// trunk. A pull request that has been here longer than the policy's
    /// `pull_request_mergeable_after` is a finding, not a fact of life.
    Mergeable,
    /// Open, and this process could not reach the forge to say more. Never a silent green:
    /// the report says which pull requests it could not judge.
    Unknown,
}

impl PullRequestState {
    /// The word this state is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            PullRequestState::Landed => "landed",
            PullRequestState::Abandoned => "abandoned",
            PullRequestState::Superseded => "superseded",
            PullRequestState::Draft => "draft",
            PullRequestState::Blocked => "blocked",
            PullRequestState::Failing => "failing",
            PullRequestState::AwaitingCi => "awaiting_ci",
            PullRequestState::Mergeable => "mergeable",
            PullRequestState::Unknown => "unknown",
        }
    }

    /// Every state, for the documentation and the completeness test.
    pub const ALL: &'static [PullRequestState] = &[
        PullRequestState::Landed,
        PullRequestState::Abandoned,
        PullRequestState::Superseded,
        PullRequestState::Draft,
        PullRequestState::Blocked,
        PullRequestState::Failing,
        PullRequestState::AwaitingCi,
        PullRequestState::Mergeable,
        PullRequestState::Unknown,
    ];

    /// Still on its way: neither landed nor abandoned.
    pub fn is_open(self) -> bool {
        !matches!(self, PullRequestState::Landed | PullRequestState::Abandoned)
    }
}

/// Where the pull request facts came from, and therefore how much of the answer is real.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestSource {
    /// The forge answered: open pull requests, drafts, checks and ages are all real.
    Forge,
    /// Only git was available. Landed pull requests are read from the trunk's own merge
    /// commits, which is exact; **open** pull requests are invisible from here, and the
    /// report says so rather than reporting none.
    Git,
}

impl PullRequestSource {
    /// The word this source is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            PullRequestSource::Forge => "forge",
            PullRequestSource::Git => "git",
        }
    }
}

/// One pull request, as the lifecycle sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestView {
    /// Its number.
    pub number: u64,
    /// Its title, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The branch it would land.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Where it stands.
    pub state: PullRequestState,
    /// Where the facts came from.
    pub source: PullRequestSource,
    /// The commit that landed it, when it landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    /// How old it is, in whole days, measured against the one clock. Absent when the forge
    /// was not reached and git could not date it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_days: Option<i64>,
    /// Merging its head into the trunk conflicts — proved locally, not taken from the
    /// forge. Absent when the head is not in this object store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts_locally: Option<bool>,
    /// What the forge said its mergeability was. Recorded, never believed: this repository
    /// configures a per-clone merge driver the forge does not have, so this field reports a
    /// conflict for pull requests that merge cleanly here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge_mergeable: Option<String>,
    /// Why it is in this state, in one line.
    pub evidence: String,
}

// ---------------------------------------------------------------- evidence

/// One measured fact that put a piece of work in its state. The state is a name; this is
/// the reason, and every report carries both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// The measurement, as a stable machine name (`unique_commits`, `dirty`, `live_owner`).
    pub measure: String,
    /// What it said, rendered for a person.
    pub value: String,
}

impl Evidence {
    /// A measurement and its value.
    pub fn new(measure: impl Into<String>, value: impl Into<String>) -> Self {
        Evidence {
            measure: measure.into(),
            value: value.into(),
        }
    }
}

/// Why a piece of work may not be removed. A blocker is never a guess: each one names the
/// measurement that refuses, so that clearing it is a defined action rather than a hope.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    /// Commits here are reachable from no ref on origin.
    UniqueCommits,
    /// Tracked changes, or untracked files, are present.
    UncommittedWork,
    /// A peer of the shared board, or a git lock, names this worktree.
    LiveOwner,
    /// A merge, rebase, cherry-pick, revert or bisect is in progress.
    OperationInProgress,
    /// A merged pull request landed this branch, and the branch carries a commit that
    /// merge does not contain.
    ResidueAfterLanding,
    /// This is the primary checkout, or it holds the trunk.
    PrimaryOrTrunk,
    /// Git holds the worktree locked.
    Locked,
    /// An open pull request depends on this branch.
    OpenPullRequest,
    /// HEAD is detached, so no branch preserves what is here.
    DetachedHead,
    /// The facts needed to decide were not all available. A check that cannot see its whole
    /// subject refuses; it does not report "safe" with a footnote.
    Unproven,
}

impl Blocker {
    /// The word this blocker is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            Blocker::UniqueCommits => "unique_commits",
            Blocker::UncommittedWork => "uncommitted_work",
            Blocker::LiveOwner => "live_owner",
            Blocker::OperationInProgress => "operation_in_progress",
            Blocker::ResidueAfterLanding => "residue_after_landing",
            Blocker::PrimaryOrTrunk => "primary_or_trunk",
            Blocker::Locked => "locked",
            Blocker::OpenPullRequest => "open_pull_request",
            Blocker::DetachedHead => "detached_head",
            Blocker::Unproven => "unproven",
        }
    }

    /// Every blocker, for the documentation and the completeness test.
    pub const ALL: &'static [Blocker] = &[
        Blocker::UniqueCommits,
        Blocker::UncommittedWork,
        Blocker::LiveOwner,
        Blocker::OperationInProgress,
        Blocker::ResidueAfterLanding,
        Blocker::PrimaryOrTrunk,
        Blocker::Locked,
        Blocker::OpenPullRequest,
        Blocker::DetachedHead,
        Blocker::Unproven,
    ];
}

/// One reason a piece of work may not be removed, with what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClosureBlocker {
    /// The stable code.
    pub code: Blocker,
    /// What is in the way, in one line.
    pub message: String,
    /// The command a person runs to clear it.
    pub remedy: String,
}

// ---------------------------------------------------------------- the records

/// What landed this branch, when something did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Landing {
    /// The merge commit on the trunk.
    pub merge_commit: String,
    /// The pull request that merge named, when it named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// The tip the merge brought in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_head: Option<String>,
    /// Commits on the branch that this landing does **not** contain. Empty when the landing
    /// is complete; non-empty is what makes the branch [`WorkState::Superseded`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residue: Vec<String>,
}

/// One worktree, or one branch without a worktree, with its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkRecord {
    /// The name this record is shown under: the branch, or `detached/<short commit>`.
    pub label: String,
    /// The branch, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The worktree holding it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The commit at the tip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Where it stands.
    pub state: WorkState,
    /// What put it there.
    pub evidence: Vec<Evidence>,
    /// Commits reachable from **no** ref on origin:
    /// `git rev-list --count HEAD --not --remotes=origin`. Stricter and more correct than
    /// "the branch is not on origin": a branch whose tip nobody pushed can still be wholly
    /// contained in a pushed integration branch, and one whose name is on origin can still
    /// carry a commit nothing on origin holds.
    pub unique_commits: usize,
    /// The first few of those commits, subject included, so that a person can see what is
    /// at risk without a second command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_sample: Vec<String>,
    /// Uncommitted work here, when a worktree holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<crate::worktree::DirtyState>,
    /// What landed this branch, when something did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<Landing>,
    /// The pull requests that name this branch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pull_requests: Vec<PullRequestView>,
    /// The issue this branch provably names, from the topology's own attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The task this work belongs to, when a task record names the branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The milestone the issue belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The tip is reachable from the trunk.
    pub merged_into_trunk: bool,
    /// Annotated archive tags containing the tip, with the reason each records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archived_as: Vec<String>,
    /// How old the newest commit here is, in whole days, against the one clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_days: Option<i64>,
    /// The peer of the shared board that named this path, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Removing this is provably safe: nothing unique, nothing uncommitted, nobody on it.
    pub cleanup_safe: bool,
    /// Why not, when not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<ClosureBlocker>,
}

/// A finding: something a person should act on, with the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// The stable code (`lifecycle.stranded_work`, `lifecycle.pull_request_aged`).
    pub code: String,
    /// How serious it is.
    pub severity: crate::model::Severity,
    /// What it is about: a branch, a worktree or a pull request.
    pub subject: String,
    /// What is wrong, in one line.
    pub message: String,
    /// What a person does about it.
    pub remedy: String,
}

/// How many records are in each state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LifecycleTallies {
    /// Records examined.
    pub records: usize,
    /// Records per state, in the vocabulary's own order.
    pub by_state: Vec<StateCount>,
    /// Records carrying work the canonical path does not hold.
    pub unconverged: usize,
    /// Commits, across every record, reachable from no ref on origin.
    pub unique_commits: usize,
    /// Records whose removal is provably safe.
    pub cleanup_safe: usize,
    /// Pull requests examined.
    pub pull_requests: usize,
    /// Findings raised.
    pub findings: usize,
}

/// One state and how many records are in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateCount {
    /// The state.
    pub state: WorkState,
    /// How many.
    pub count: usize,
}

/// The whole lifecycle of this repository's work: every worktree, every branch, every pull
/// request, with the evidence and the findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LifecycleReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// When this was measured, RFC 3339, from the one clock.
    pub measured_at: String,
    /// The trunk every question here is asked against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// Where the pull request facts came from.
    pub pull_request_source: PullRequestSource,
    /// False when something could not be measured; `limitations` says what.
    pub complete: bool,
    /// What this report could not see. A check that cannot reach its whole subject says so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    /// The thresholds this report judged ages against, as resolved from the policy.
    pub policy: super::policy::AgingPolicy,
    /// Every worktree and every branch, the riskiest states first.
    pub records: Vec<WorkRecord>,
    /// Every pull request the source could see.
    pub pull_requests: Vec<PullRequestView>,
    /// What to act on.
    pub findings: Vec<Finding>,
    /// The counts.
    pub tallies: LifecycleTallies,
}

/// What a prune would do, and what it refuses to do. Planning changes nothing: this is the
/// document `majordomus lifecycle prune` prints, and the one it applies only with
/// `--apply`, and only for the entries it proved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CleanupPlan {
    /// [`SCHEMA`].
    pub schema: String,
    /// When this was measured.
    pub measured_at: String,
    /// Worktrees whose removal is proved safe.
    pub removable: Vec<CleanupStep>,
    /// Worktrees this refuses to touch, each with why.
    pub refused: Vec<CleanupStep>,
    /// Nothing was applied by producing this.
    pub applied: bool,
}

/// One worktree a prune considered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CleanupStep {
    /// The worktree.
    pub path: String,
    /// Its branch, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Where it stands.
    pub state: WorkState,
    /// The command that would remove it.
    pub command: String,
    /// Why it is safe, or why it is not.
    pub reason: String,
    /// The blockers, when it is refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<ClosureBlocker>,
}
