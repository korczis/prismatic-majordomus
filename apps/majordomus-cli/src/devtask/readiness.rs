//! Task readiness: a derived projection of the canonical status, and not a second
//! vocabulary for it.
//!
//! # Why there is one at all
//!
//! [`crate::plan::ISSUE_STATUSES`] is the canonical vocabulary — `READY`, `BLOCKED`,
//! `ACTIVE`, `VERIFY`, `DONE`, `CANCELLED` — and it is right for what it is: the six words
//! `lib/project.awk` derives, held to byte equality with the shell engine by
//! `test/cases/99_plan_capabilities.sh`. Nothing here changes, replaces or renames any of
//! them; every [`TaskReadiness`] carries the canonical status it was derived from, so a caller
//! that wants the canonical word has it on the same object.
//!
//! What it adds is exactly the distinctions the canonical six *already make in the data*
//! and cannot express in the word:
//!
//! * **`BLOCKED` is two situations.** The plan's own derivation appends
//!   `milestone:<id>` to an issue's `blocked_by` when the milestone gate holds it, on top of
//!   whatever issue dependencies it has. An issue with every dependency `DONE`, held back
//!   only because the outcome it belongs to waits on another outcome, is not the same
//!   problem as an issue waiting on a peer: the first is resolved somewhere else entirely.
//!   [`TaskReadiness::Waiting`] is that case; [`TaskReadiness::Blocked`] is the other.
//! * **`VERIFY` is two situations.** The plan reaches `VERIFY` when `verified_at` or
//!   `completed_at` is set, and it emits the `evidence_missing` finding for the second of
//!   those when the required evidence is not all there. So `VERIFY` covers both "the work
//!   is done and someone must confirm it" and "someone already declared it complete and the
//!   evidence does not back that up" — the second is a defect a person has to act on, the
//!   first is a queue. [`TaskReadiness::Review`] and [`TaskReadiness::CompletionBlocked`].
//! * **an id the model does not have.** `plan.issues` cannot answer for one at all, and
//!   answering `BLOCKED` or an empty object for a typo is the one answer a work surface must
//!   never give. [`TaskReadiness::Undeclared`].
//!
//! and nothing else. There is no `in_review`, no `needs_triage`, no `wontfix`: three words
//! that no field of any canonical record could distinguish, which is what makes them
//! frontend strings rather than workflow.
//!
//! # The mapping is total and one-way
//!
//! Every canonical status maps to exactly one readiness, and [`TaskReadiness::canonical`] maps
//! back. The three added distinctions are refinements *inside* a canonical status, never
//! across two, so the round trip through `canonical` is lossless in the direction that
//! matters: no readiness can claim a canonical status the plan did not derive.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a development task stands, as work.
///
/// ```
/// use majordomus_cli::devtask::TaskReadiness;
///
/// // every state names the canonical status it refines, and refines only one
/// assert_eq!(TaskReadiness::Ready.canonical(), "READY");
/// assert_eq!(TaskReadiness::Waiting.canonical(), "BLOCKED");
/// assert_eq!(TaskReadiness::Blocked.canonical(), "BLOCKED");
/// assert_eq!(TaskReadiness::Review.canonical(), "VERIFY");
/// assert_eq!(TaskReadiness::CompletionBlocked.canonical(), "VERIFY");
///
/// // and one state is the absence of a record rather than a status of one
/// assert!(TaskReadiness::Undeclared.canonical().is_empty());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskReadiness {
    /// Nothing holds it back and nobody has started it. Canonical `READY`.
    Ready,
    /// At least one issue it depends on is not `DONE`. Canonical `BLOCKED`.
    Blocked,
    /// Every issue dependency is satisfied; the milestone gate holds it. Canonical
    /// `BLOCKED`, and a distinct situation because it is resolved in another milestone.
    Waiting,
    /// Execution began: the record carries `started_at`. Canonical `ACTIVE`.
    InProgress,
    /// Implementation is declared finished and completion has not been recorded: the record
    /// carries `verified_at`. Canonical `VERIFY`.
    Review,
    /// Completion *was* recorded and the evidence the record itself requires is not all
    /// there, so the plan refuses to call it `DONE`. Canonical `VERIFY`, and the case the
    /// `evidence_missing` finding is about.
    CompletionBlocked,
    /// Finished on its own terms: not cancelled, completion recorded, every required
    /// evidence token present. Canonical `DONE`.
    Complete,
    /// Withdrawn. Canonical `CANCELLED`.
    Cancelled,
    /// The canonical model does not declare this id. Not a status: the absence of a record.
    Undeclared,
}

impl TaskReadiness {
    /// The canonical status this refines, or the empty string for [`TaskReadiness::Undeclared`],
    /// which refines none.
    pub fn canonical(self) -> &'static str {
        match self {
            TaskReadiness::Ready => "READY",
            TaskReadiness::Blocked | TaskReadiness::Waiting => "BLOCKED",
            TaskReadiness::InProgress => "ACTIVE",
            TaskReadiness::Review | TaskReadiness::CompletionBlocked => "VERIFY",
            TaskReadiness::Complete => "DONE",
            TaskReadiness::Cancelled => "CANCELLED",
            TaskReadiness::Undeclared => "",
        }
    }

    /// The word a projection prints.
    pub fn as_str(self) -> &'static str {
        match self {
            TaskReadiness::Ready => "ready",
            TaskReadiness::Blocked => "blocked",
            TaskReadiness::Waiting => "waiting",
            TaskReadiness::InProgress => "in_progress",
            TaskReadiness::Review => "review",
            TaskReadiness::CompletionBlocked => "completion_blocked",
            TaskReadiness::Complete => "complete",
            TaskReadiness::Cancelled => "cancelled",
            TaskReadiness::Undeclared => "undeclared",
        }
    }

    /// May a worker pick this up right now?
    ///
    /// ```
    /// use majordomus_cli::devtask::TaskReadiness;
    /// assert!(TaskReadiness::Ready.is_startable());
    /// assert!(!TaskReadiness::InProgress.is_startable(), "somebody already has it");
    /// ```
    pub fn is_startable(self) -> bool {
        self == TaskReadiness::Ready
    }

    /// Is there nothing further to do, whatever happens elsewhere?
    pub fn is_terminal(self) -> bool {
        matches!(self, TaskReadiness::Complete | TaskReadiness::Cancelled)
    }

    /// Every state, in the order a board reads them: the executable ones first, the
    /// finished ones last. Declared once so that no surface writes the sequence down.
    pub const ALL: &'static [TaskReadiness] = &[
        TaskReadiness::Ready,
        TaskReadiness::InProgress,
        TaskReadiness::Review,
        TaskReadiness::CompletionBlocked,
        TaskReadiness::Blocked,
        TaskReadiness::Waiting,
        TaskReadiness::Complete,
        TaskReadiness::Cancelled,
        TaskReadiness::Undeclared,
    ];

    /// Derive the readiness from the facts the plan already holds.
    ///
    /// The arguments are exactly what [`crate::plan`] derives and what the canonical record
    /// declares; nothing is read here, and nothing is guessed. `status` is the canonical
    /// status, `blockers` the plan's own `blocked_by` list — whose `milestone:` prefix is
    /// what separates [`TaskReadiness::Waiting`] from [`TaskReadiness::Blocked`] — and
    /// `completion_recorded` / `evidence_covered` are what separate
    /// [`TaskReadiness::CompletionBlocked`] from [`TaskReadiness::Review`].
    ///
    /// ```
    /// use majordomus_cli::devtask::TaskReadiness;
    ///
    /// // blocked only by its milestone is waiting, not blocked
    /// let waiting = TaskReadiness::derive("BLOCKED", &["milestone:m1".to_string()], false, true);
    /// assert_eq!(waiting, TaskReadiness::Waiting);
    ///
    /// // blocked by a peer, milestone gate or not, is blocked
    /// let blocked = TaskReadiness::derive(
    ///     "BLOCKED",
    ///     &["I0002".to_string(), "milestone:m1".to_string()],
    ///     false,
    ///     true,
    /// );
    /// assert_eq!(blocked, TaskReadiness::Blocked);
    ///
    /// // completion recorded without the evidence it requires
    /// let stuck = TaskReadiness::derive("VERIFY", &[], true, false);
    /// assert_eq!(stuck, TaskReadiness::CompletionBlocked);
    ///
    /// // verified and waiting to be closed
    /// assert_eq!(TaskReadiness::derive("VERIFY", &[], false, false), TaskReadiness::Review);
    ///
    /// // a status the plan never derives is not silently mapped onto one that looks fine
    /// assert_eq!(TaskReadiness::derive("NONESUCH", &[], false, false), TaskReadiness::Undeclared);
    /// ```
    pub fn derive(
        status: &str,
        blockers: &[String],
        completion_recorded: bool,
        evidence_covered: bool,
    ) -> TaskReadiness {
        match status {
            "READY" => TaskReadiness::Ready,
            "BLOCKED" => {
                let only_the_gate = !blockers.is_empty()
                    && blockers.iter().all(|b| b.starts_with(MILESTONE_BLOCKER));
                if only_the_gate {
                    TaskReadiness::Waiting
                } else {
                    TaskReadiness::Blocked
                }
            }
            "ACTIVE" => TaskReadiness::InProgress,
            "VERIFY" => {
                if completion_recorded && !evidence_covered {
                    TaskReadiness::CompletionBlocked
                } else {
                    TaskReadiness::Review
                }
            }
            "DONE" => TaskReadiness::Complete,
            "CANCELLED" => TaskReadiness::Cancelled,
            // A status the plan does not declare cannot be mapped onto one that reads as
            // work. The plan itself reports it as an `unknown_status` finding; here it is
            // the absence of a usable state, which is what `Undeclared` means.
            _ => TaskReadiness::Undeclared,
        }
    }
}

/// The prefix the plan's own `blocked_by` uses for the milestone gate. Stated once, because
/// two readers of the same token is how a prefix quietly changes on one side only.
pub const MILESTONE_BLOCKER: &str = "milestone:";

/// What kind of thing is holding a task back. The five are the five the canonical model can
/// actually produce; there is no `other`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    /// An issue this one depends on that is not `DONE`.
    Issue,
    /// The milestone gate: an outcome this one's milestone requires is not `DONE`.
    Milestone,
    /// A required evidence token the record does not carry, with completion recorded.
    Evidence,
    /// A dependency that resolves to nothing: the model has no such issue.
    Reference,
    /// A dependency cycle this task is in or downstream of, so it can never become ready.
    Cycle,
}

impl BlockerKind {
    /// The word a projection prints.
    pub fn as_str(self) -> &'static str {
        match self {
            BlockerKind::Issue => "issue",
            BlockerKind::Milestone => "milestone",
            BlockerKind::Evidence => "evidence",
            BlockerKind::Reference => "reference",
            BlockerKind::Cycle => "cycle",
        }
    }

    /// Is this a defect in the model rather than work still to be done? A reference that
    /// resolves to nothing and a cycle are both bugs in the records; the other three are
    /// the ordinary state of a plan being executed.
    ///
    /// ```
    /// use majordomus_cli::devtask::BlockerKind;
    /// assert!(BlockerKind::Reference.is_defect());
    /// assert!(!BlockerKind::Issue.is_defect());
    /// ```
    pub fn is_defect(self) -> bool {
        matches!(self, BlockerKind::Reference | BlockerKind::Cycle)
    }
}

/// One thing standing between a task and being startable, with what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Blocker {
    /// What kind of thing it is.
    pub kind: BlockerKind,
    /// The thing: an issue id, a milestone id, an evidence token.
    pub subject: String,
    /// What is wrong, in one line a person can act on.
    pub detail: String,
}

impl Blocker {
    /// One blocker.
    pub fn new(kind: BlockerKind, subject: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            subject: subject.into(),
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::ISSUE_STATUSES;

    /// Every canonical status the plan can derive maps to a readiness that names it back.
    /// The assertion that keeps this a projection rather than a parallel vocabulary: a
    /// seventh canonical status added to `lib/project.awk` and mirrored into
    /// `ISSUE_STATUSES` fails here rather than reaching a surface as `undeclared`.
    #[test]
    fn every_canonical_status_is_covered_and_maps_back_to_itself() {
        for status in ISSUE_STATUSES {
            let r = TaskReadiness::derive(status, &[], false, false);
            assert_ne!(
                r,
                TaskReadiness::Undeclared,
                "{status} reaches no readiness state"
            );
            assert_eq!(
                r.canonical(),
                *status,
                "{status} maps to {} which names {} instead",
                r.as_str(),
                r.canonical()
            );
        }
    }

    /// The refinements refine: each added state names the same canonical status as the state
    /// it was split from, so no readiness can claim a status the plan did not derive.
    #[test]
    fn no_readiness_claims_a_status_the_plan_does_not_declare() {
        for r in TaskReadiness::ALL {
            let c = r.canonical();
            if c.is_empty() {
                assert_eq!(*r, TaskReadiness::Undeclared);
                continue;
            }
            assert!(
                ISSUE_STATUSES.contains(&c),
                "{} claims the status {c}, which the plan does not declare",
                r.as_str()
            );
        }
    }

    /// `ALL` is the whole vocabulary: a variant added and not listed would be invisible to
    /// every board that reads the order from here.
    #[test]
    fn the_declared_order_holds_every_state_once() {
        let mut seen: Vec<&str> = TaskReadiness::ALL.iter().map(|r| r.as_str()).collect();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n, "a state is listed twice");
        // one per canonical status, plus the three refinements, plus the absence
        assert_eq!(n, ISSUE_STATUSES.len() + 3);
    }

    /// A blocked issue with no blockers at all would be a contradiction in the plan; it is
    /// reported as `blocked` rather than as `waiting`, because "only the gate holds it" is a
    /// claim about a gate that is not there.
    #[test]
    fn blocked_with_no_blocker_named_is_not_waiting() {
        assert_eq!(
            TaskReadiness::derive("BLOCKED", &[], false, false),
            TaskReadiness::Blocked
        );
    }

    /// Only `Ready` is startable, and the two finished states are the only terminal ones.
    #[test]
    fn startable_and_terminal_are_disjoint_and_narrow() {
        let startable: Vec<&str> = TaskReadiness::ALL
            .iter()
            .filter(|r| r.is_startable())
            .map(|r| r.as_str())
            .collect();
        assert_eq!(startable, ["ready"]);
        let terminal: Vec<&str> = TaskReadiness::ALL
            .iter()
            .filter(|r| r.is_terminal())
            .map(|r| r.as_str())
            .collect();
        assert_eq!(terminal, ["complete", "cancelled"]);
    }
}
