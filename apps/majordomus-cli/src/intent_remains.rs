//! What remains of an intent (ADR 0117): for each unmet criterion, what is in the way, why,
//! and the next action that justifies; for each intent, one outcome.
//!
//! Everything here is a pure function of facts other modules derive — the criterion's evidence
//! state (the intent engine) and its coverage entry (the strength and the live issues serving
//! it, from `intent_plan`) — and nothing is stored or performed. The words are the ones the
//! mission names: `progressing`, `blocked`, `needs_evidence`, `failed`, `exhausted`, `unknown`,
//! and `satisfied` for an intent whose verdict is.
//!
//! The table, first match wins ("open" is `READY`, `BLOCKED`, `ACTIVE` or `VERIFY`):
//!
//! | | condition | remains | basis |
//! |---|---|---|---|
//! | 1 | an open covering issue exists, and every open one is `BLOCKED` | `blocked` | `work_blocked` |
//! | 2 | an open covering issue exists, and the evidence is `failing` | `progressing` | `work_open_failing` |
//! | 3 | an open covering issue exists | `progressing` | `work_open` |
//! | 4 | the evidence is `not_derivable` | `unknown` | `not_derivable` |
//! | 5 | the evidence is `failing` and a covering issue exists | `failed` | `closed_work` |
//! | 6 | the evidence is `failing` and coverage is `observed` | `failed` | `gap_observation` |
//! | 7 | the evidence is `failing` | `failed` | `no_work` |
//! | 8 | a covering issue exists | `needs_evidence` | `closed_work` |
//! | 9 | coverage is `observed` | `needs_evidence` | `gap_observation` |
//! | 10 | the evidence is `stale` | `needs_evidence` | `passed_before` |
//! | 11 | otherwise | `exhausted` | `no_work` |
//!
//! ```
//! use majordomus_cli::intent::IntentEvidenceState as E;
//! use majordomus_cli::intent_plan::{CoverageStrength as C, CoveringIssue};
//! use majordomus_cli::intent_remains::{remains, Basis, Remains};
//! let active = CoveringIssue {
//!     id: "I1".into(), milestone: "m".into(), status: "ACTIVE".into(), evidence_need: 1,
//! };
//! // a test written first fails while its issue is open: progress, and named as failing
//! assert_eq!(
//!     remains(E::Failing, &[active], C::Covered),
//!     (Remains::Progressing, Basis::WorkOpenFailing),
//! );
//! // a criterion a gap observed satisfied, served by nothing, whose evidence now fails
//! assert_eq!(remains(E::Failing, &[], C::Observed), (Remains::Failed, Basis::GapObservation));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::intent::{IntentEvidenceState, IntentGuard, IntentVerdictState};
use crate::intent_plan::{CoverageStrength, CoveringIssue};

/// What is in the way of one unmet criterion. Declared in the order an intent's outcome reads
/// them: the first one a required criterion carries decides it.
///
/// ```
/// use majordomus_cli::intent_remains::Remains;
/// assert_eq!(serde_json::to_string(&Remains::NeedsEvidence).unwrap(), "\"needs_evidence\"");
/// assert!(Remains::Progressing < Remains::Blocked, "moving work outranks blocked work");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Remains {
    /// Its evidence fails and no open work serves it.
    Failed,
    /// Nothing serves it, nothing observed it, and it never passed: no path to proof.
    Exhausted,
    /// No open work serves it and its evidence is not current.
    NeedsEvidence,
    /// Its evidence is of a kind the ledger cannot derive, and no open work serves it.
    Unknown,
    /// Open work serves it.
    Progressing,
    /// Every open issue serving it is blocked.
    Blocked,
}

/// Why a criterion's [`Remains`] is what it is: which row of the table decided it.
///
/// ```
/// use majordomus_cli::intent_remains::Basis;
/// assert_eq!(serde_json::to_string(&Basis::GapObservation).unwrap(), "\"gap_observation\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Every open covering issue is blocked.
    WorkBlocked,
    /// An open issue serves it and its evidence fails.
    WorkOpenFailing,
    /// An open issue serves it.
    WorkOpen,
    /// The evidence is of a kind the ledger cannot derive.
    NotDerivable,
    /// Issues served it and every one of them is done.
    ClosedWork,
    /// No issue serves it, and a gap observed it satisfied.
    GapObservation,
    /// Its evidence passed once and is stale now; nothing serves it.
    PassedBefore,
    /// No issue serves it and nothing observed it.
    NoWork,
}

/// Whether a covering issue is still to be done.
fn open(issue: &CoveringIssue) -> bool {
    matches!(
        issue.status.as_str(),
        "READY" | "BLOCKED" | "ACTIVE" | "VERIFY"
    )
}

/// What remains of one unmet criterion, by the table: its evidence state, the live issues
/// coverage found serving it, and coverage's strength.
///
/// ```
/// use majordomus_cli::intent::IntentEvidenceState as E;
/// use majordomus_cli::intent_plan::CoverageStrength as C;
/// use majordomus_cli::intent_remains::{remains, Basis, Remains};
/// assert_eq!(remains(E::NotRun, &[], C::Uncovered), (Remains::Exhausted, Basis::NoWork));
/// assert_eq!(
///     remains(E::Stale, &[], C::Uncovered),
///     (Remains::NeedsEvidence, Basis::PassedBefore),
/// );
/// ```
pub fn remains(
    state: IntentEvidenceState,
    issues: &[CoveringIssue],
    strength: CoverageStrength,
) -> (Remains, Basis) {
    let open: Vec<&CoveringIssue> = issues.iter().filter(|i| open(i)).collect();
    let failing = state == IntentEvidenceState::Failing;
    let observed = strength == CoverageStrength::Observed;
    if !open.is_empty() && open.iter().all(|i| i.status == "BLOCKED") {
        (Remains::Blocked, Basis::WorkBlocked)
    } else if !open.is_empty() && failing {
        (Remains::Progressing, Basis::WorkOpenFailing)
    } else if !open.is_empty() {
        (Remains::Progressing, Basis::WorkOpen)
    } else if state == IntentEvidenceState::NotDerivable {
        (Remains::Unknown, Basis::NotDerivable)
    } else if failing && !issues.is_empty() {
        (Remains::Failed, Basis::ClosedWork)
    } else if failing && observed {
        (Remains::Failed, Basis::GapObservation)
    } else if failing {
        (Remains::Failed, Basis::NoWork)
    } else if !issues.is_empty() {
        (Remains::NeedsEvidence, Basis::ClosedWork)
    } else if observed {
        (Remains::NeedsEvidence, Basis::GapObservation)
    } else if state == IntentEvidenceState::Stale {
        (Remains::NeedsEvidence, Basis::PassedBefore)
    } else {
        (Remains::Exhausted, Basis::NoWork)
    }
}

/// The next action a criterion's [`Remains`] justifies, as data. Nothing performs it.
///
/// ```
/// use majordomus_cli::intent_remains::NextAction;
/// let n = NextAction::PlanWork { serves: "x#a".into() };
/// assert!(serde_json::to_string(&n).unwrap().contains("plan_work"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NextAction {
    /// Carry on with the open issues serving it.
    Work {
        /// The open issues, each with its status.
        issues: Vec<CoveringIssue>,
    },
    /// Unblock the issues serving it.
    Unblock {
        /// The blocked issues.
        issues: Vec<String>,
    },
    /// Run its evidence on a clean tree, stamp the tree the run left and record the run
    /// (`majordomus evidence stamp`, then `majordomus evidence record`).
    RecordEvidence {
        /// The command that produces its evidence, when one is known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reproduce: Option<String>,
    },
    /// Find why the evidence of work that closed fails, from the failing run.
    Repair {
        /// The command that reproduces the failure, when one is known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reproduce: Option<String>,
    },
    /// Plan an issue that serves it; nothing in the plan will make it true.
    PlanWork {
        /// The criterion, as an issue's `serves` names it.
        serves: String,
    },
    /// No action can be derived.
    None {
        /// Why.
        because: String,
    },
}

/// The next action for criterion `criterion` of intent `intent`.
///
/// ```
/// use majordomus_cli::intent_remains::{next, Basis, NextAction, Remains};
/// assert_eq!(
///     next("x", "a", (Remains::Exhausted, Basis::NoWork), &[], None),
///     NextAction::PlanWork { serves: "x#a".into() },
/// );
/// ```
pub fn next(
    intent: &str,
    criterion: &str,
    (remains, basis): (Remains, Basis),
    issues: &[CoveringIssue],
    reproduce: Option<&str>,
) -> NextAction {
    let reproduce = reproduce.map(str::to_string);
    let open_issues = || issues.iter().filter(|i| open(i)).cloned();
    match (remains, basis) {
        (Remains::Progressing, _) => NextAction::Work {
            issues: open_issues().collect(),
        },
        (Remains::Blocked, _) => NextAction::Unblock {
            issues: open_issues().map(|i| i.id).collect(),
        },
        (Remains::Failed, Basis::ClosedWork) => NextAction::Repair { reproduce },
        (Remains::Failed, _) | (Remains::Exhausted, _) => NextAction::PlanWork {
            serves: format!("{intent}#{criterion}"),
        },
        (Remains::NeedsEvidence, _) => NextAction::RecordEvidence { reproduce },
        (Remains::Unknown, _) => NextAction::None {
            because: "its evidence is of a kind the ledger cannot derive".into(),
        },
    }
}

/// What stands between one intent and its criteria, as one word: the first answer in ADR
/// 0117's order that a required unmet criterion carries, or what the verdict and the guards
/// decide before any criterion is read.
///
/// ```
/// use majordomus_cli::intent_remains::IntentOutcome;
/// assert_eq!(
///     serde_json::to_string(&IntentOutcome::NeedsEvidence).unwrap(),
///     "\"needs_evidence\"",
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentOutcome {
    /// The verdict is satisfied.
    Satisfied,
    /// A guard is violated, or a required criterion is [`Remains::Failed`].
    Failed,
    /// A required criterion has no path to proof.
    Exhausted,
    /// A required criterion's work is done and its evidence is not current.
    NeedsEvidence,
    /// A required criterion cannot be judged, or nothing required is unmet and the verdict is
    /// not satisfied; the verdict says why.
    Unknown,
    /// Work is moving on the remaining required criteria.
    Progressing,
    /// The only work on the remaining required criteria is blocked.
    Blocked,
}

/// The outcome of an intent from its verdict, its guards and the [`Remains`] of each unmet
/// criterion with whether that criterion is optional. Optional criteria decide nothing.
///
/// ```
/// use majordomus_cli::intent::IntentVerdictState;
/// use majordomus_cli::intent_remains::{outcome, IntentOutcome, Remains};
/// let unmet = [(Remains::Blocked, false), (Remains::Progressing, false), (Remains::Failed, true)];
/// assert_eq!(outcome(IntentVerdictState::Unsatisfied, &[], &unmet), IntentOutcome::Progressing);
/// ```
pub fn outcome(
    verdict: IntentVerdictState,
    guards: &[IntentGuard],
    unmet: &[(Remains, bool)],
) -> IntentOutcome {
    if verdict == IntentVerdictState::Satisfied {
        return IntentOutcome::Satisfied;
    }
    if guards.iter().any(|g| g.violated) {
        return IntentOutcome::Failed;
    }
    match unmet
        .iter()
        .filter(|(_, optional)| !optional)
        .map(|(r, _)| *r)
        .min()
    {
        Some(Remains::Failed) => IntentOutcome::Failed,
        Some(Remains::Exhausted) => IntentOutcome::Exhausted,
        Some(Remains::NeedsEvidence) => IntentOutcome::NeedsEvidence,
        Some(Remains::Progressing) => IntentOutcome::Progressing,
        Some(Remains::Blocked) => IntentOutcome::Blocked,
        Some(Remains::Unknown) | None => IntentOutcome::Unknown,
    }
}

/// Fill what remains into a realization from the coverage `intent validate` computes: each
/// unmet criterion of a live intent gets its `remains`, `basis` and `next`, and the intent its
/// `outcome`. A cancelled or superseded intent gets none of them: it owes nothing.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_plan::IntentCoverage;
/// use majordomus_cli::intent_realization::IntentRealization;
/// use majordomus_cli::intent_remains::fill;
/// let mut r = IntentRealization { intents: vec![], work: vec![], orphans: 0, findings: vec![] };
/// let intents = Intents { intents: vec![], findings: vec![] };
/// let coverage = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
/// fill(&mut r, &intents, &coverage, &|_| None);
/// assert!(r.intents.is_empty());
/// ```
pub fn fill(
    realization: &mut crate::intent_realization::IntentRealization,
    intents: &crate::intent::Intents,
    coverage: &crate::intent_plan::IntentCoverage,
    revision: &dyn Fn(&str) -> Option<String>,
) {
    use crate::intent::IntentStage;
    for view in &mut realization.intents {
        let Some(intent) = intents.intent(&view.intent) else {
            continue;
        };
        if matches!(
            intent.stage,
            IntentStage::Cancelled | IntentStage::Superseded
        ) {
            continue;
        }
        let mut answered = Vec::with_capacity(view.unmet.len());
        for c in &mut view.unmet {
            let entry = coverage
                .criteria
                .iter()
                .find(|e| e.intent == view.intent && e.criterion == c.id);
            let (issues, strength) = entry.map_or((&[][..], CoverageStrength::Uncovered), |e| {
                (&e.issues[..], e.strength)
            });
            let answer = remains(c.state, issues, strength);
            c.remains = Some(answer.0);
            c.basis = Some(answer.1);
            c.next = Some(next(
                &view.intent,
                &c.id,
                answer,
                issues,
                c.reproduce.as_deref(),
            ));
            answered.push((answer.0, c.optional));
        }
        view.outcome = Some(outcome(intent.verdict.state, &intent.guards, &answered));
        view.review_revision = revision(&view.intent);
        view.remains_digest = Some(digest(view));
    }
}

/// A digest of what remains of one intent: its outcome and each unmet criterion's id, evidence
/// state, `remains` and `basis`. A recorded run moves it, and so does a serving issue that
/// closes; the plan's wording, an issue's scope and its dependencies are the review revision's.
///
/// ```
/// use majordomus_cli::intent::IntentStage;
/// use majordomus_cli::intent_realization::IntentRealizationView;
/// use majordomus_cli::intent_remains::digest;
/// let v = IntentRealizationView {
///     intent: "x".into(), title: "X".into(), stage: IntentStage::Planned,
///     criteria: 1, met: 0, unmet: vec![], work: vec![], providers: vec![], findings: vec![],
///     outcome: None, review_revision: None, remains_digest: None, change: None,
/// };
/// assert_eq!(digest(&v), digest(&v.clone()), "a function of the reading alone");
/// ```
pub fn digest(view: &crate::intent_realization::IntentRealizationView) -> String {
    let unmet: Vec<serde_json::Value> = view
        .unmet
        .iter()
        .map(|c| serde_json::json!([c.id, c.state, c.remains, c.basis]))
        .collect();
    crate::policy::sha256_hex(
        &serde_json::json!({ "outcome": view.outcome, "unmet": unmet }).to_string(),
    )
}

/// The two values of an earlier reading of one intent, as the caller kept them.
///
/// ```
/// use majordomus_cli::intent_remains::Since;
/// let s: Since = serde_json::from_str(r#"{"review_revision":"a","remains_digest":"b"}"#).unwrap();
/// assert_eq!(s.remains_digest, "b");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Since {
    /// The `review_revision` that reading answered.
    pub review_revision: String,
    /// The `remains_digest` that reading answered.
    pub remains_digest: String,
}

/// How a reading differs from an earlier one.
///
/// ```
/// use majordomus_cli::intent_remains::IntentChange;
/// assert_eq!(serde_json::to_string(&IntentChange::RemainsMoved).unwrap(), "\"remains_moved\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentChange {
    /// Neither value moved.
    Unchanged,
    /// What remains moved — a run was recorded, a serving issue closed — and the plan did not.
    RemainsMoved,
    /// The plan a review judges changed; outranks a move of what remains, because a reader
    /// whose criteria were rewritten must read them again first.
    PlanChanged,
}

/// Compare an earlier reading with this one.
///
/// ```
/// use majordomus_cli::intent_remains::{change, IntentChange, Since};
/// let then = Since { review_revision: "r".into(), remains_digest: "d".into() };
/// assert_eq!(change(&then, Some("r"), Some("d")), IntentChange::Unchanged);
/// assert_eq!(change(&then, Some("r"), Some("e")), IntentChange::RemainsMoved);
/// assert_eq!(change(&then, Some("s"), Some("d")), IntentChange::PlanChanged);
/// ```
pub fn change(
    since: &Since,
    review_revision: Option<&str>,
    remains_digest: Option<&str>,
) -> IntentChange {
    if review_revision != Some(since.review_revision.as_str()) {
        IntentChange::PlanChanged
    } else if remains_digest != Some(since.remains_digest.as_str()) {
        IntentChange::RemainsMoved
    } else {
        IntentChange::Unchanged
    }
}
