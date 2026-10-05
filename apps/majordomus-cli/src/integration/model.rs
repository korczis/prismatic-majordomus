//! The canonical pull-request state and the one disposition model.
//!
//! Everything a surface says about a pull request is a field of [`PullRequestAssessment`], and every
//! surface — the command line, the HTTP route, the MCP tool, the Cockpit — renders that one
//! value. Nothing downstream re-derives a verdict from raw forge data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the forge said about one open pull request, as observed. Plain data: the forge
/// adapter fills it, a test builds it by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestObservation {
    /// The number.
    pub number: u64,
    /// The title.
    pub title: String,
    /// The author's login.
    pub author: String,
    /// The branch it asks to merge.
    pub head_ref: String,
    /// The commit that branch pointed at when observed.
    pub head_sha: String,
    /// The branch it asks to merge into.
    pub base_ref: String,
    /// A draft is not asking to be merged.
    pub draft: bool,
    /// The label names.
    #[serde(default)]
    pub labels: Vec<String>,
    /// When it was opened, RFC 3339.
    pub created_at: String,
    /// When it last changed, RFC 3339.
    pub updated_at: String,
    /// The body, for dependency declarations; never rendered.
    #[serde(default)]
    pub body: String,
    /// Every check reported on the head commit.
    #[serde(default)]
    pub checks: Vec<CheckObservation>,
    /// The forge's review decision, verbatim (`APPROVED`, `CHANGES_REQUESTED`,
    /// `REVIEW_REQUIRED`) or empty when the repository asks for none.
    #[serde(default)]
    pub review_decision: String,
    /// Whether the forge has auto-merge armed on it.
    #[serde(default)]
    pub auto_merge: bool,
    /// Whether the head lives in another repository (a fork): its branch cannot be
    /// refreshed from here.
    #[serde(default)]
    pub cross_repository: bool,
    /// Each reviewer's latest review, with the commit it was given on. A review is about one
    /// commit: an approval of another commit is not an approval of the head.
    #[serde(default)]
    pub latest_reviews: Vec<ReviewObservation>,
    /// Who has been asked to review and has not yet: logins, or team slugs.
    #[serde(default)]
    pub review_requests: Vec<String>,
}

/// One reviewer's latest review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewObservation {
    /// The reviewer's login.
    pub author: String,
    /// The forge's word for it, verbatim (`APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`,
    /// `DISMISSED`).
    pub state: String,
    /// The commit the review was given on; empty when the forge did not say.
    #[serde(default)]
    pub commit: String,
}

/// What the base requires of reviews, read from its protection and rulesets together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewPolicy {
    /// How many approving reviews a merge needs; 0 when none.
    pub approvals: u64,
    /// Whether a code owner's approval is required.
    pub code_owners: bool,
    /// Whether the forge dismisses an approval when the head moves.
    pub dismiss_stale: bool,
}

/// One check the base requires: a status context, and the app that must write it when the
/// protection binds it to one. A check of that name from any other writer is not this one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct RequiredCheck {
    /// The status context, or the check run's name.
    pub context: String,
    /// The app bound to it, when the protection names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<u64>,
}

impl From<&str> for RequiredCheck {
    fn from(context: &str) -> Self {
        RequiredCheck {
            context: context.to_string(),
            app_id: None,
        }
    }
}

impl std::fmt::Display for RequiredCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.app_id {
            Some(app) => write!(f, "{} (app {app})", self.context),
            None => f.write_str(&self.context),
        }
    }
}

/// What reported a check: a check run (written by an app) or a commit status context.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// A check run.
    #[default]
    CheckRun,
    /// A commit status context.
    StatusContext,
}

/// One check run or status context on a head commit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckObservation {
    /// The check's name or status context.
    pub name: String,
    /// Where it stands.
    pub state: CheckRunState,
    /// What reported it.
    #[serde(default)]
    pub kind: CheckKind,
    /// The app that wrote it, when the forge said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<u64>,
    /// When it completed (or, for a status context, was set), RFC 3339; empty while it runs
    /// or when the forge did not say. The newest report of a context is its verdict.
    #[serde(default)]
    pub completed_at: String,
}

/// Where one check run stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckRunState {
    /// Completed and passed.
    Passed,
    /// Completed and failed (failure, timed out, cancelled, action required).
    Failed,
    /// Queued or running; also what a report that says nothing is taken to be, so a
    /// default is never a pass.
    #[default]
    Pending,
    /// Completed as skipped or neutral.
    Skipped,
}

/// The state of the repository's *required* checks on one head, which is the only check
/// state a merge decision reads. A visible green check that is not required proves nothing,
/// and a required check that is absent is not a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequiredCheckState {
    /// Every required check passed on this head.
    Passed,
    /// Every required check passed, at least one of them by a skip the policy permits for
    /// that context. A skip the policy does not permit is not a pass: it is `missing`.
    Skipped,
    /// A required check is queued or running.
    Pending,
    /// A required check failed.
    Failed,
    /// A required check has not reported on this head at all.
    Missing,
    /// The repository's required checks could not be read.
    Unknown,
}

/// Where the forge's review policy stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestReview {
    /// The branch protection requires no review.
    NotRequired,
    /// Approved.
    Approved,
    /// A reviewer asked for changes.
    ChangesRequested,
    /// A required review has not been given.
    Pending,
    /// The approvals were given on another commit than the head: an approval is of one
    /// commit, and these are not of this one.
    Stale,
    /// The approvals are there, but a code owner's is still required.
    CodeOwnersPending,
    /// The protection could not be read.
    Unknown,
}

/// What the pull request's head is to the current master, decided locally by git with
/// this repository's own merge drivers — never by the forge's `mergeable`, which cannot
/// run the derived-file driver and calls nearly every pull request here conflicting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RelationToMaster {
    /// The head is an ancestor of master: every commit already landed.
    Contained,
    /// Merging the head into master changes nothing: its patch is already fully there.
    Superseded,
    /// The merge would conflict or change only derived output, but every commit the head has
    /// and master lacks is on master already as an equal patch (`git cherry` marks each `-`):
    /// the change landed, and master moved on past it. Never a partial match, and never a
    /// head that carries a merge commit of its own, whose resolution has no patch to compare.
    PatchIdsUpstream {
        /// How many commits, every one of them already on master.
        commits: u64,
    },
    /// Merging changes only derived artifacts: the authored change is already on master,
    /// and what differs is output the generators rewrite.
    DerivedOnly {
        /// The derived paths that would change.
        paths: Vec<String>,
    },
    /// The head already contains master and merges cleanly.
    UpToDate {
        /// Authored (non-derived) paths the merge changes.
        authored: Vec<String>,
    },
    /// The head does not contain master, but the merge is clean with the derived driver.
    Behind {
        /// Commits on master the head does not have.
        behind: u64,
        /// Authored paths the merge changes.
        authored: Vec<String>,
    },
    /// Merging conflicts on authored paths: a person must resolve it.
    Conflicting {
        /// The conflicting authored paths.
        paths: Vec<String>,
    },
    /// Git could not answer (the head is not fetched, or git failed); the reason.
    Unknown {
        /// Why.
        reason: String,
    },
}

/// Explicit ordering between pull requests, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestDependency {
    /// The pull request this one waits for.
    pub number: u64,
    /// How it is known.
    pub certainty: DependencyCertainty,
    /// Whether it is satisfied: that pull request has landed (or is not open any more).
    pub satisfied: bool,
}

/// How a dependency is known. Only a declared one blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyCertainty {
    /// Declared: a `Depends on #N` / `Stacked on #N` line in the body, or a base branch
    /// that is another open pull request's head.
    Confirmed,
    /// Inferred: the head contains the other's head commit. Evidence, never a block.
    Inferred,
}

/// How much could go wrong if this lands and something was missed. Planning information:
/// it orders work and it never relaxes a policy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationRisk {
    /// Documentation, tests, content.
    Low,
    /// Code of the executable or the tool.
    Medium,
    /// CI, release, governance, schemas, or a large change.
    High,
}

/// The one classification of an open pull request. Every value carries a machine-readable
/// reason in [`PullRequestAssessment::reasons`]; there is no `mergeable: bool`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestDisposition {
    /// Contains the current master, every required check passed on its head, nothing
    /// blocks it: the executor may merge it.
    Ready,
    /// Merges cleanly but does not contain master: the executor may bring master in (a
    /// merge commit on the branch, never a rewrite), after which CI must run again.
    NeedsRefresh,
    /// Contains master; a required check has not finished on this head.
    WaitingForChecks,
    /// A required review is missing or changes were requested.
    WaitingForReview,
    /// A declared dependency has not landed.
    WaitingForDependency,
    /// A draft.
    Draft,
    /// A required check failed on this head.
    NeedsRepair,
    /// Conflicts on authored paths: a person resolves them.
    Conflicting,
    /// A label whose policy is to hold holds it, or the repository allows no merge commit,
    /// which is the only way the executor merges.
    Blocked,
    /// The forge has auto-merge armed on it: the forge would merge it on its own, outside the
    /// executor and against whatever master is then. Held until a person disarms it.
    Unsafe,
    /// Its head already landed, its merge changes nothing, or every one of its commits is on
    /// master as an equal patch: strong evidence from git, eligible for closure under policy.
    /// (Before 0.13 this was the word `superseded`.)
    Redundant,
    /// Its body, or the body of another pull request, declares that a successor replaces it,
    /// and that successor landed: no longer open, its head contained in master. The successor
    /// is [`PullRequestAssessment::superseded_by`]. Eligible for closure under policy.
    Superseded,
    /// Weak evidence: only derived artifacts would change, or its declared successor was closed
    /// without its head landing. Surfaced for a person; never closed automatically.
    PossiblyRedundant,
    /// Targets a branch other than the integration base.
    OtherBase,
    /// Something needed to decide could not be observed.
    Unknown,
}

impl PullRequestDisposition {
    /// Every disposition, in declaration order.
    pub const ALL: [PullRequestDisposition; 15] = [
        PullRequestDisposition::Ready,
        PullRequestDisposition::NeedsRefresh,
        PullRequestDisposition::WaitingForChecks,
        PullRequestDisposition::WaitingForReview,
        PullRequestDisposition::WaitingForDependency,
        PullRequestDisposition::Draft,
        PullRequestDisposition::NeedsRepair,
        PullRequestDisposition::Conflicting,
        PullRequestDisposition::Blocked,
        PullRequestDisposition::Unsafe,
        PullRequestDisposition::Redundant,
        PullRequestDisposition::Superseded,
        PullRequestDisposition::PossiblyRedundant,
        PullRequestDisposition::OtherBase,
        PullRequestDisposition::Unknown,
    ];

    /// The word as serialised.
    ///
    /// ```text
    /// use crate::integration::PullRequestDisposition;
    /// assert_eq!(PullRequestDisposition::NeedsRefresh.as_str(), "needs_refresh");
    /// ```text
    pub fn as_str(self) -> &'static str {
        match self {
            PullRequestDisposition::Ready => "ready",
            PullRequestDisposition::NeedsRefresh => "needs_refresh",
            PullRequestDisposition::WaitingForChecks => "waiting_for_checks",
            PullRequestDisposition::WaitingForReview => "waiting_for_review",
            PullRequestDisposition::WaitingForDependency => "waiting_for_dependency",
            PullRequestDisposition::Draft => "draft",
            PullRequestDisposition::NeedsRepair => "needs_repair",
            PullRequestDisposition::Conflicting => "conflicting",
            PullRequestDisposition::Blocked => "blocked",
            PullRequestDisposition::Unsafe => "unsafe",
            PullRequestDisposition::Redundant => "redundant",
            PullRequestDisposition::Superseded => "superseded",
            PullRequestDisposition::PossiblyRedundant => "possibly_redundant",
            PullRequestDisposition::OtherBase => "other_base",
            PullRequestDisposition::Unknown => "unknown",
        }
    }

    /// The queue a disposition belongs to on every surface.
    pub fn lane(self) -> IntegrationLane {
        match self {
            PullRequestDisposition::Ready => IntegrationLane::Ready,
            PullRequestDisposition::NeedsRefresh
            | PullRequestDisposition::WaitingForChecks
            | PullRequestDisposition::WaitingForReview
            | PullRequestDisposition::WaitingForDependency => IntegrationLane::Waiting,
            PullRequestDisposition::NeedsRepair | PullRequestDisposition::Conflicting => {
                IntegrationLane::Repair
            }
            PullRequestDisposition::Redundant
            | PullRequestDisposition::Superseded
            | PullRequestDisposition::PossiblyRedundant => IntegrationLane::Cleanup,
            PullRequestDisposition::Draft
            | PullRequestDisposition::Blocked
            | PullRequestDisposition::Unsafe
            | PullRequestDisposition::OtherBase
            | PullRequestDisposition::Unknown => IntegrationLane::Held,
        }
    }
}

/// The queues the Cockpit and the status table group pull requests into.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationLane {
    /// The executor may merge it now.
    Ready,
    /// Something will happen without a person: a refresh, a check, a dependency.
    Waiting,
    /// A person must change the branch.
    Repair,
    /// Its work is on master already, or a successor that landed replaced it.
    Cleanup,
    /// Not asking to be merged, held by a label or a setting, unsafe for the executor, or
    /// undecidable.
    Held,
}

/// What kind of fact a piece of evidence is. The wire words are the ones the evidence carried
/// while its kind was a string, so a trail written then still reads.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// One required check's state on the head: one entry per context the base requires.
    RequiredCheck,
    /// The required checks together: their verdict, and each context's state.
    RequiredChecks,
    /// The review policy and the forge's decision, or one reviewer's latest review.
    Review,
    /// What the head is to master, as git decided it.
    RelationToMaster,
    /// One declared dependency.
    Dependency,
    /// One label whose policy is to hold.
    Label,
    /// Whether it is a draft; always emitted.
    Draft,
    /// The branch it targets; always emitted.
    Base,
    /// How old the observation is. Declared so the vocabulary is settled; nothing emits it yet.
    Freshness,
    /// The forge has auto-merge armed; emitted whenever it is.
    AutoMerge,
    /// One declared successor: which pull request replaces this one, whose body said so, and
    /// whether it landed. One entry per successor declared.
    Supersession,
    /// What the repository's settings allow the executor; emitted when they allow no merge
    /// commit.
    RepositorySettings,
}

impl EvidenceKind {
    /// The wire word.
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::RequiredCheck => "required_check",
            EvidenceKind::RequiredChecks => "required_checks",
            EvidenceKind::Review => "review",
            EvidenceKind::RelationToMaster => "relation_to_master",
            EvidenceKind::Dependency => "dependency",
            EvidenceKind::Label => "label",
            EvidenceKind::Draft => "draft",
            EvidenceKind::Base => "base",
            EvidenceKind::Freshness => "freshness",
            EvidenceKind::AutoMerge => "auto_merge",
            EvidenceKind::Supersession => "supersession",
            EvidenceKind::RepositorySettings => "repository_settings",
        }
    }
}

impl std::fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.as_str())
    }
}

impl PartialEq<&str> for EvidenceKind {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Where a piece of evidence was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum EvidenceSource {
    /// The forge observation, at its moment.
    Forge {
        /// When the forge was observed, RFC 3339.
        observed_at: String,
    },
    /// Git, on this pair of commits.
    Git {
        /// The master commit.
        master_sha: String,
        /// The head commit.
        head_sha: String,
    },
}

/// One piece of evidence behind a disposition: what was read, where, and what it said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationEvidence {
    /// What kind of fact.
    pub kind: EvidenceKind,
    /// What it said, as a word.
    pub status: String,
    /// The detail a person reads.
    pub detail: String,
    /// Where it was read. Every assessment names it; a trail line written before evidence
    /// carried it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EvidenceSource>,
}

/// The revisions a decision was taken against, and when the forge was observed. A decision is
/// valid only while both revisions still hold: the executor compares them with what it
/// re-reads before it acts.
#[derive(Debug, Clone, Default, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvaluatedAgainst {
    /// The master commit.
    pub master_sha: String,
    /// The pull request's head commit.
    pub head_sha: String,
    /// When the forge was observed, RFC 3339; empty in a value written before it was named.
    #[serde(default)]
    pub observed_at: String,
}

/// Two are equal when they name the same revisions. The moment is when, not what: the same
/// master and head observed a moment later is the same decision, which is what the executor's
/// stale-decision comparison asks.
impl PartialEq for EvaluatedAgainst {
    fn eq(&self, other: &Self) -> bool {
        self.master_sha == other.master_sha && self.head_sha == other.head_sha
    }
}

/// One question of the policy, in the order [`IntegrationGate::ALL`] asks them. The
/// disposition is the first that fails; every one is answered.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationGate {
    /// It targets the integration base.
    Base,
    /// It is not a draft.
    Draft,
    /// It carries no label whose policy is to hold.
    Label,
    /// The forge has no auto-merge armed on it.
    AutoMerge,
    /// No successor is declared to replace it. Asked before the relation to master, because
    /// a pull request whose successor landed usually conflicts with what the successor
    /// brought, and is superseded rather than conflicting.
    Supersession,
    /// Its head is not on master already, and git could say that it merges cleanly.
    RelationToMaster,
    /// The repository allows a merge commit, the only way the executor merges. Asked after
    /// the relation, so work already on master is still `redundant` and may be closed.
    MergeMethod,
    /// Every declared dependency has landed.
    Dependency,
    /// The review policy is satisfied on the head.
    Review,
    /// No required check failed on the head.
    NoFailingCheck,
    /// The head contains the current master.
    Freshness,
    /// Every required check passed on the head.
    RequiredChecks,
}

impl IntegrationGate {
    /// Every gate, in policy order.
    pub const ALL: [IntegrationGate; 12] = [
        IntegrationGate::Base,
        IntegrationGate::Draft,
        IntegrationGate::Label,
        IntegrationGate::AutoMerge,
        IntegrationGate::Supersession,
        IntegrationGate::RelationToMaster,
        IntegrationGate::MergeMethod,
        IntegrationGate::Dependency,
        IntegrationGate::Review,
        IntegrationGate::NoFailingCheck,
        IntegrationGate::Freshness,
        IntegrationGate::RequiredChecks,
    ];

    /// The wire word.
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationGate::Base => "base",
            IntegrationGate::Draft => "draft",
            IntegrationGate::Label => "label",
            IntegrationGate::AutoMerge => "auto_merge",
            IntegrationGate::Supersession => "supersession",
            IntegrationGate::MergeMethod => "merge_method",
            IntegrationGate::RelationToMaster => "relation_to_master",
            IntegrationGate::Dependency => "dependency",
            IntegrationGate::Review => "review",
            IntegrationGate::NoFailingCheck => "no_failing_check",
            IntegrationGate::Freshness => "freshness",
            IntegrationGate::RequiredChecks => "required_checks",
        }
    }
}

/// How one gate answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GateResult {
    /// The gate.
    pub gate: IntegrationGate,
    /// Whether it passed. A gate that fails only because an earlier one did (the head is not
    /// fresh when its merge conflicts) fails without a reason of its own.
    pub passed: bool,
}

/// What [`ReasonCode`]'s schema says, since its wire form is a string: the vocabulary.
const REASON_VOCABULARY: &str = "A reason code, `code` or `code:payload`, one of: \
`stacked_on:#N`, `base_is:BRANCH`, `draft`, `label:NAME`, `auto_merge_armed`, \
`merge_commit_not_allowed`, `superseded_by:#N`, `successor_open:#N`, \
`successor_not_landed:#N`, `successor_unread:#N`, `head_reachable_from_master`, \
`merge_changes_nothing`, `patch_ids_upstream`, `only_derived_artifacts_differ`, \
`relation_unknown:WHY`, \
`conflicts_on:COUNT`, `depends_on:#N`, `review:STATE`, `review_policy_unread`, \
`required_check_failed`, `behind_master:COMMITS`, `fork_head`, `required_checks:STATE`, \
`no_required_checks`, `required_checks_unread`, `contains_master`, `required_checks_passed`, \
`required_checks_skipped`, `executor_merge_refused:HEAD`, `executor_refresh_failed:MASTER`. \
A code outside this list (an older trail's) is carried verbatim.";

/// One machine-readable reason, typed. Its wire form is the `code` or `code:payload` string
/// the reasons always had ([`std::fmt::Display`] and [`std::str::FromStr`]), so the trail,
/// the OpenAPI string arrays and the Cockpit read what they read before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasonCode {
    /// `stacked_on:#N`: it targets the head branch of open pull request N.
    StackedOn {
        /// The pull request it is stacked on.
        number: u64,
    },
    /// `base_is:BRANCH`: it targets another branch than the base, and no open one's head.
    BaseIs {
        /// The branch it targets.
        base: String,
    },
    /// `draft`.
    Draft,
    /// `label:NAME`: a label whose policy is to hold holds it.
    Label {
        /// The label, as the forge spells it.
        name: String,
    },
    /// `auto_merge_armed`: the forge would merge it on its own, outside the executor.
    AutoMergeArmed,
    /// `merge_commit_not_allowed`: the repository's settings allow no merge commit, and the
    /// executor never squashes or rebases.
    MergeCommitNotAllowed,
    /// `superseded_by:#N`: a declared successor, N, landed (no longer open, its head contained
    /// in master).
    SupersededBy {
        /// The successor.
        number: u64,
    },
    /// `successor_open:#N`: a declared successor, N, is still open; this one waits for it to
    /// land and must not land itself meanwhile.
    SuccessorOpen {
        /// The successor.
        number: u64,
    },
    /// `successor_not_landed:#N`: a declared successor, N, is no longer open, but its head is
    /// not contained in master (closed unmerged, or merged by a squash or a rebase).
    SuccessorNotLanded {
        /// The successor.
        number: u64,
    },
    /// `successor_unread:#N`: a declared successor, N, is not open, and the forge or git could
    /// not say what became of it.
    SuccessorUnread {
        /// The successor.
        number: u64,
    },
    /// `head_reachable_from_master`: every commit already landed.
    HeadReachableFromMaster,
    /// `merge_changes_nothing`: its patch is already fully on master.
    MergeChangesNothing,
    /// `patch_ids_upstream`: every commit it has and master lacks is on master as an equal
    /// patch.
    PatchIdsUpstream,
    /// `only_derived_artifacts_differ`.
    OnlyDerivedArtifactsDiffer,
    /// `relation_unknown:WHY`: git could not say what the head is to master.
    RelationUnknown {
        /// Why.
        reason: String,
    },
    /// `conflicts_on:COUNT`: the merge conflicts on this many authored paths (the paths are
    /// in the relation; the wire form has always carried the count).
    ConflictsOn {
        /// How many authored paths conflict.
        count: usize,
    },
    /// `depends_on:#N`: a declared dependency is still open.
    DependsOn {
        /// The pull request it waits for.
        number: u64,
    },
    /// `review:STATE`: the review policy is not satisfied.
    Review {
        /// The review state.
        state: PullRequestReview,
    },
    /// `review_policy_unread`.
    ReviewPolicyUnread,
    /// `required_check_failed`.
    RequiredCheckFailed,
    /// `behind_master:COMMITS`: the head does not contain master.
    BehindMaster {
        /// Commits on master the head does not have.
        commits: u64,
    },
    /// `fork_head`: the head is a fork's branch, which cannot be refreshed from here.
    ForkHead,
    /// `required_checks:STATE`: the required checks have not all passed.
    RequiredChecks {
        /// Their verdict.
        state: RequiredCheckState,
    },
    /// `no_required_checks`: the base requires none (owner decision D5).
    NoRequiredChecks,
    /// `required_checks_unread`.
    RequiredChecksUnread,
    /// `contains_master`: why a ready one is ready.
    ContainsMaster,
    /// `required_checks_passed`: why a ready one is ready.
    RequiredChecksPassed,
    /// `required_checks_skipped`: ready, a permitted skip among its checks.
    RequiredChecksSkipped,
    /// `executor_merge_refused:HEAD`: the forge refused the executor's merge of this head,
    /// against this master; a new head or a new master clears it.
    ExecutorMergeRefused {
        /// The head whose merge was refused.
        head: String,
    },
    /// `executor_refresh_failed:MASTER`: bringing this master into the branch failed; a new
    /// head or a new master clears it.
    ExecutorRefreshFailed {
        /// The master that could not be brought in.
        master: String,
    },
    /// A code this vocabulary does not name, verbatim: what an older trail line may carry.
    /// Nothing here produces one, and [`std::str::FromStr`] refuses it.
    Unrecognised(String),
}

impl ReasonCode {
    /// The code: the wire form before its payload.
    pub fn code(&self) -> &str {
        match self {
            ReasonCode::StackedOn { .. } => "stacked_on",
            ReasonCode::BaseIs { .. } => "base_is",
            ReasonCode::Draft => "draft",
            ReasonCode::Label { .. } => "label",
            ReasonCode::AutoMergeArmed => "auto_merge_armed",
            ReasonCode::MergeCommitNotAllowed => "merge_commit_not_allowed",
            ReasonCode::SupersededBy { .. } => "superseded_by",
            ReasonCode::SuccessorOpen { .. } => "successor_open",
            ReasonCode::SuccessorNotLanded { .. } => "successor_not_landed",
            ReasonCode::SuccessorUnread { .. } => "successor_unread",
            ReasonCode::HeadReachableFromMaster => "head_reachable_from_master",
            ReasonCode::MergeChangesNothing => "merge_changes_nothing",
            ReasonCode::PatchIdsUpstream => "patch_ids_upstream",
            ReasonCode::OnlyDerivedArtifactsDiffer => "only_derived_artifacts_differ",
            ReasonCode::RelationUnknown { .. } => "relation_unknown",
            ReasonCode::ConflictsOn { .. } => "conflicts_on",
            ReasonCode::DependsOn { .. } => "depends_on",
            ReasonCode::Review { .. } => "review",
            ReasonCode::ReviewPolicyUnread => "review_policy_unread",
            ReasonCode::RequiredCheckFailed => "required_check_failed",
            ReasonCode::BehindMaster { .. } => "behind_master",
            ReasonCode::ForkHead => "fork_head",
            ReasonCode::RequiredChecks { .. } => "required_checks",
            ReasonCode::NoRequiredChecks => "no_required_checks",
            ReasonCode::RequiredChecksUnread => "required_checks_unread",
            ReasonCode::ContainsMaster => "contains_master",
            ReasonCode::RequiredChecksPassed => "required_checks_passed",
            ReasonCode::RequiredChecksSkipped => "required_checks_skipped",
            ReasonCode::ExecutorMergeRefused { .. } => "executor_merge_refused",
            ReasonCode::ExecutorRefreshFailed { .. } => "executor_refresh_failed",
            ReasonCode::Unrecognised(s) => s.split_once(':').map_or(s.as_str(), |(c, _)| c),
        }
    }
}

/// The serialised word of a unit enum.
fn wire_word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => "unknown".into(),
    }
}

impl std::fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReasonCode::StackedOn { number }
            | ReasonCode::DependsOn { number }
            | ReasonCode::SupersededBy { number }
            | ReasonCode::SuccessorOpen { number }
            | ReasonCode::SuccessorNotLanded { number }
            | ReasonCode::SuccessorUnread { number } => {
                write!(f, "{}:#{number}", self.code())
            }
            ReasonCode::BaseIs { base: s }
            | ReasonCode::Label { name: s }
            | ReasonCode::RelationUnknown { reason: s }
            | ReasonCode::ExecutorMergeRefused { head: s }
            | ReasonCode::ExecutorRefreshFailed { master: s } => write!(f, "{}:{s}", self.code()),
            ReasonCode::ConflictsOn { count } => write!(f, "{}:{count}", self.code()),
            ReasonCode::BehindMaster { commits } => write!(f, "{}:{commits}", self.code()),
            ReasonCode::Review { state } => write!(f, "{}:{}", self.code(), wire_word(state)),
            ReasonCode::RequiredChecks { state } => {
                write!(f, "{}:{}", self.code(), wire_word(state))
            }
            ReasonCode::Unrecognised(s) => f.write_str(s),
            _ => f.write_str(self.code()),
        }
    }
}

impl std::str::FromStr for ReasonCode {
    type Err = String;

    /// The reason a wire string names; an error for a code outside the vocabulary or a
    /// payload that is not the code's.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("not a reason code: {s:?}");
        let number = |p: &str| p.strip_prefix('#').and_then(|n| n.parse::<u64>().ok());
        let word = |p: &str| serde_json::Value::String(p.to_string());
        let (code, payload) = match s.split_once(':') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let reason = match (code, payload) {
            ("stacked_on", Some(p)) => ReasonCode::StackedOn {
                number: number(p).ok_or_else(bad)?,
            },
            ("depends_on", Some(p)) => ReasonCode::DependsOn {
                number: number(p).ok_or_else(bad)?,
            },
            ("superseded_by", Some(p)) => ReasonCode::SupersededBy {
                number: number(p).ok_or_else(bad)?,
            },
            ("successor_open", Some(p)) => ReasonCode::SuccessorOpen {
                number: number(p).ok_or_else(bad)?,
            },
            ("successor_not_landed", Some(p)) => ReasonCode::SuccessorNotLanded {
                number: number(p).ok_or_else(bad)?,
            },
            ("successor_unread", Some(p)) => ReasonCode::SuccessorUnread {
                number: number(p).ok_or_else(bad)?,
            },
            ("base_is", Some(p)) => ReasonCode::BaseIs { base: p.into() },
            ("label", Some(p)) => ReasonCode::Label { name: p.into() },
            ("relation_unknown", Some(p)) => ReasonCode::RelationUnknown { reason: p.into() },
            ("conflicts_on", Some(p)) => ReasonCode::ConflictsOn {
                count: p.parse().map_err(|_| bad())?,
            },
            ("behind_master", Some(p)) => ReasonCode::BehindMaster {
                commits: p.parse().map_err(|_| bad())?,
            },
            ("review", Some(p)) => ReasonCode::Review {
                state: serde_json::from_value(word(p)).map_err(|_| bad())?,
            },
            ("required_checks", Some(p)) => ReasonCode::RequiredChecks {
                state: serde_json::from_value(word(p)).map_err(|_| bad())?,
            },
            ("draft", None) => ReasonCode::Draft,
            ("auto_merge_armed", None) => ReasonCode::AutoMergeArmed,
            ("merge_commit_not_allowed", None) => ReasonCode::MergeCommitNotAllowed,
            ("head_reachable_from_master", None) => ReasonCode::HeadReachableFromMaster,
            ("merge_changes_nothing", None) => ReasonCode::MergeChangesNothing,
            ("patch_ids_upstream", None) => ReasonCode::PatchIdsUpstream,
            ("only_derived_artifacts_differ", None) => ReasonCode::OnlyDerivedArtifactsDiffer,
            ("review_policy_unread", None) => ReasonCode::ReviewPolicyUnread,
            ("required_check_failed", None) => ReasonCode::RequiredCheckFailed,
            ("fork_head", None) => ReasonCode::ForkHead,
            ("no_required_checks", None) => ReasonCode::NoRequiredChecks,
            ("required_checks_unread", None) => ReasonCode::RequiredChecksUnread,
            ("contains_master", None) => ReasonCode::ContainsMaster,
            ("required_checks_passed", None) => ReasonCode::RequiredChecksPassed,
            ("required_checks_skipped", None) => ReasonCode::RequiredChecksSkipped,
            ("executor_merge_refused", Some(p)) => {
                ReasonCode::ExecutorMergeRefused { head: p.into() }
            }
            ("executor_refresh_failed", Some(p)) => {
                ReasonCode::ExecutorRefreshFailed { master: p.into() }
            }
            _ => return Err(bad()),
        };
        Ok(reason)
    }
}

impl Serialize for ReasonCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Never fails on a string: a code outside the vocabulary is [`ReasonCode::Unrecognised`],
/// so a trail line an older executor wrote still reads.
impl<'de> Deserialize<'de> for ReasonCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(s.parse().unwrap_or(ReasonCode::Unrecognised(s)))
    }
}

/// A string on the wire, its vocabulary in the description: the OpenAPI arrays of reasons
/// stay arrays of strings.
impl JsonSchema for ReasonCode {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ReasonCode".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": REASON_VOCABULARY,
        })
    }
}

/// Whether `reason`'s wire form is `wire`, compared as it is written, without building it.
fn wire_is(reason: &ReasonCode, wire: &str) -> bool {
    struct Against<'a> {
        rest: &'a str,
        same: bool,
    }
    impl std::fmt::Write for Against<'_> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            match self.rest.strip_prefix(s) {
                Some(rest) if self.same => self.rest = rest,
                _ => self.same = false,
            }
            Ok(())
        }
    }
    let mut against = Against {
        rest: wire,
        same: true,
    };
    let _ = std::fmt::Write::write_fmt(&mut against, format_args!("{reason}"));
    against.same && against.rest.is_empty()
}

impl PartialEq<str> for ReasonCode {
    fn eq(&self, other: &str) -> bool {
        wire_is(self, other)
    }
}

impl PartialEq<&str> for ReasonCode {
    fn eq(&self, other: &&str) -> bool {
        wire_is(self, other)
    }
}

impl PartialEq<String> for ReasonCode {
    fn eq(&self, other: &String) -> bool {
        wire_is(self, other)
    }
}

/// Reasons as a person reads them: their wire forms, joined by `sep`.
pub fn reason_list(reasons: &[ReasonCode], sep: &str) -> String {
    reasons
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(sep)
}

/// The canonical state of one open pull request: what was observed, what it is to master,
/// how it is classified and why. The one value every surface renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestAssessment {
    /// The number.
    pub number: u64,
    /// The title.
    pub title: String,
    /// The author.
    pub author: String,
    /// The head branch.
    pub head_ref: String,
    /// The base branch.
    pub base_ref: String,
    /// The revisions this was decided against.
    pub evaluated_against: EvaluatedAgainst,
    /// The classification.
    pub disposition: PullRequestDisposition,
    /// The successor that landed and replaces it, exactly when the disposition is
    /// `superseded`: the disposition stays one word on the wire, and this is its `by`.
    /// Absent otherwise; a successor still open, or closed without landing, is in the
    /// `supersession` evidence and the reasons instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<u64>,
    /// The queue it is in.
    pub lane: IntegrationLane,
    /// Machine-readable reasons, one for every failing gate's every finding, in policy order:
    /// the first is the decisive one. A ready pull request carries why it is ready.
    pub reasons: Vec<ReasonCode>,
    /// Every gate of the policy, in order, and whether it passed.
    #[serde(default)]
    pub gates: Vec<GateResult>,
    /// What a person or the executor does next, when anything.
    pub next_action: Option<String>,
    /// The required checks on the head.
    pub required_checks: RequiredCheckState,
    /// The review state.
    pub review: PullRequestReview,
    /// What the head is to master.
    pub relation: RelationToMaster,
    /// Declared and inferred dependencies.
    pub dependencies: Vec<PullRequestDependency>,
    /// Paths the pull request changes (authored ones; derived output is excluded).
    pub authored_paths: Vec<String>,
    /// Other open pull requests changing a same authored path, with the shared paths.
    pub overlaps: Vec<PathOverlap>,
    /// The planning risk.
    pub risk: IntegrationRisk,
    /// Why the risk is what it is.
    pub risk_factors: Vec<String>,
    /// Every fact read.
    pub evidence: Vec<IntegrationEvidence>,
    /// When it was opened.
    pub created_at: String,
    /// How long it has been the executor's to act on, and how often another was chosen
    /// instead — derived from the audit trail ([`super::wait`]), present only while it is
    /// ready or refreshable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<ExecutorWait>,
    /// Why it stands where it does in the rank: every component the order compares, in the
    /// order it compares them. Set by the planner; absent on an assessment never ranked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_factors: Option<RankFactors>,
}

/// What the rank compares, in order: the first component that differs between two pull
/// requests decides which comes first. Each is a value of the assessment or of the queue
/// around it, so the order can be explained and is the same whatever order the forge listed
/// the pull requests in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RankFactors {
    /// The lane: what kind of work the executor does with it.
    pub lane: IntegrationLane,
    /// The disposition, within the lane.
    pub disposition: PullRequestDisposition,
    /// The planning risk: lower first.
    pub risk: IntegrationRisk,
    /// How many other ready or refreshable pull requests change an authored path it changes:
    /// fewer first, because landing it invalidates less.
    pub contention: usize,
    /// How many open pull requests declare that they wait for this one and are not yet
    /// satisfied: more first, because landing it unblocks them.
    pub dependents: usize,
    /// How many authored paths it changes: fewer first, a smaller change is cheaper to land
    /// and to undo.
    pub authored_paths: usize,
    /// When it was opened: older first, so easy new work cannot starve old work.
    pub created_at: String,
    /// The number: the last tie-break, total.
    pub number: u64,
}

/// How long a pull request has waited for the executor, from the audit trail. Starvation is
/// made visible here, never resolved by changing the rank: the rank's age tie-break already
/// prefers the older of two otherwise equal candidates, and a wait this long is a fact a
/// person reads, not a reason to merge anything less safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutorWait {
    /// When it was first observed ready or refreshable in its current run of being so,
    /// RFC 3339.
    pub actionable_since: String,
    /// How many times the executor selected another pull request while this one was ready
    /// or refreshable.
    pub passed_over: u32,
    /// The last of those times.
    pub last_passed_over: Option<PassedOver>,
}

/// One time the executor chose another pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PassedOver {
    /// When, RFC 3339.
    pub at: String,
    /// The pull request chosen instead.
    pub for_pr: u64,
    /// What it was chosen for (`selected` to merge, `refresh_selected` to bring master in).
    pub action: super::drain::IntegrationAction,
}

/// Shared authored paths with one other open pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PathOverlap {
    /// The other pull request.
    pub number: u64,
    /// The paths both change.
    pub paths: Vec<String>,
}

#[cfg(test)]
mod vocabulary_branches {
    //! The wire vocabulary's refusals and less common forms, each asked directly.

    use super::*;

    #[test]
    fn a_check_bound_to_an_app_names_it() {
        let c = RequiredCheck {
            context: "ci".into(),
            app_id: Some(15368),
        };
        assert_eq!(c.to_string(), "ci (app 15368)");
        assert_eq!(RequiredCheck::from("ci").to_string(), "ci");
        assert_eq!(EvidenceKind::Freshness.as_str(), "freshness");
    }

    #[test]
    fn an_unrecognised_reason_keeps_its_code_and_a_unit_without_a_word_is_unknown() {
        assert_eq!(
            ReasonCode::Unrecognised("old_code:x".into()).code(),
            "old_code"
        );
        assert_eq!(ReasonCode::Unrecognised("bare".into()).code(), "bare");
        assert_eq!(wire_word(&5u8), "unknown");
    }

    #[test]
    fn every_payload_that_is_not_its_codes_is_refused() {
        for wire in [
            "stacked_on:x",
            "depends_on:#x",
            "superseded_by:7",
            "successor_open:#",
            "successor_not_landed:x",
            "successor_unread:x",
            "conflicts_on:many",
            "behind_master:-1",
            "review:maybe",
            "required_checks:green",
        ] {
            assert!(wire.parse::<ReasonCode>().is_err(), "{wire} parsed");
        }
    }

    #[test]
    fn a_reason_reads_from_a_string_only_and_compares_by_its_wire_form() {
        assert!(
            serde_json::from_str::<ReasonCode>("5").is_err(),
            "not a string"
        );
        let r: ReasonCode = serde_json::from_str("\"draft\"").unwrap();
        assert!(PartialEq::<str>::eq(&r, "draft"));
        assert!(!PartialEq::<str>::eq(&r, "label:x"), "a different prefix");
        assert!(!PartialEq::<str>::eq(&r, "draftx"), "a longer wire");
    }
}
