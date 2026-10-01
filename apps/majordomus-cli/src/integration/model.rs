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
}

/// One check run or status context on a head commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckObservation {
    /// The check's name or status context.
    pub name: String,
    /// Where it stands.
    pub state: CheckRunState,
}

/// Where one check run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckRunState {
    /// Completed and passed.
    Passed,
    /// Completed and failed (failure, timed out, cancelled, action required).
    Failed,
    /// Queued or running.
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
    /// A blocking label holds it.
    Blocked,
    /// Its head already landed, or its patch is already fully on master: strong evidence,
    /// eligible for closure under policy.
    Superseded,
    /// Only derived artifacts would change: the authored change appears to be on master
    /// already. Surfaced for a person; never closed automatically.
    PossiblyRedundant,
    /// Targets a branch other than the integration base.
    OtherBase,
    /// Something needed to decide could not be observed.
    Unknown,
}

impl PullRequestDisposition {
    /// Every disposition, in declaration order.
    #[cfg(test)]
    pub const ALL: [PullRequestDisposition; 13] = [
        PullRequestDisposition::Ready,
        PullRequestDisposition::NeedsRefresh,
        PullRequestDisposition::WaitingForChecks,
        PullRequestDisposition::WaitingForReview,
        PullRequestDisposition::WaitingForDependency,
        PullRequestDisposition::Draft,
        PullRequestDisposition::NeedsRepair,
        PullRequestDisposition::Conflicting,
        PullRequestDisposition::Blocked,
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
            PullRequestDisposition::Superseded | PullRequestDisposition::PossiblyRedundant => {
                IntegrationLane::Cleanup
            }
            PullRequestDisposition::Draft
            | PullRequestDisposition::Blocked
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
    /// Its work is on master already.
    Cleanup,
    /// Not asking to be merged, or undecidable.
    Held,
}

/// One piece of evidence behind a disposition: what was read, and what it said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationEvidence {
    /// What kind of fact (`required_checks`, `relation_to_master`, `review`, `label`,
    /// `dependency`, `draft`, `base`).
    pub kind: String,
    /// What it said, as a word.
    pub status: String,
    /// The detail a person reads.
    pub detail: String,
}

/// The revisions a decision was taken against. A decision is valid only while both still
/// hold: the executor compares them with what it re-reads before it acts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvaluatedAgainst {
    /// The master commit.
    pub master_sha: String,
    /// The pull request's head commit.
    pub head_sha: String,
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
    /// The queue it is in.
    pub lane: IntegrationLane,
    /// Machine-readable reason codes, most decisive first.
    pub reasons: Vec<String>,
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
    pub action: String,
}

/// Shared authored paths with one other open pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PathOverlap {
    /// The other pull request.
    pub number: u64,
    /// The paths both change.
    pub paths: Vec<String>,
}
