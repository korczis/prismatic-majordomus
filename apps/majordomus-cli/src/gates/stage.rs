//! The lifecycle stage a task stands at, derived from the answers to the completion policy
//! and never written down by anybody.
//!
//! # Why the stage is derived
//!
//! A lifecycle written as a state a worker sets — `implementing`, `validated`, `deployed`
//! — is a claim, and the artifact a worker learns to advance. The alternative is a pile of
//! booleans (`tests_done`, `deployed`) that a report toggles. Both are the boolean soup
//! this repository refuses: canonical facts first, status derived second.
//!
//! So the stage is a fold over the policy's stages in order. A stage whose every question
//! passes or is exempt is behind the task; the first stage with a question still owed is
//! where the task stands; a stage with a question that *refuses* — a failing gate, stale
//! evidence — is where it is blocked. `completed` is the word for a task that is past every
//! stage, and nothing else earns it.
//!
//! ```
//! use majordomus_cli::gates::{DoneQuestion, GateStatus, StageDecl, StageState, derive_stage};
//!
//! let stages = vec![
//!     StageDecl { id: "build".into(), title: "Build".into(), summary: "s".into() },
//!     StageDecl { id: "ship".into(), title: "Ship".into(), summary: "s".into() },
//! ];
//! let q = |id: &str, status: GateStatus| DoneQuestion {
//!     id: id.into(), stage: if id == "committed" { "build".into() } else { "ship".into() },
//!     question: id.into(), status, evidence: "e".into(), source: "s".into(), remediation: "r".into(),
//! };
//!
//! // committed passes, ci never reported: the task stands at Ship, pending
//! let s = derive_stage(&stages, &[q("committed", GateStatus::Pass), q("ci", GateStatus::Queued)]);
//! assert_eq!(s.id, "ship");
//! assert_eq!(s.state, StageState::Pending);
//! assert_eq!(s.owing, ["ci"]);
//! assert!(!s.complete);
//!
//! // a refusal anywhere is where the task is blocked, even behind a later pending one
//! let s = derive_stage(&stages, &[q("committed", GateStatus::Stale), q("ci", GateStatus::Queued)]);
//! assert_eq!((s.id.as_str(), s.state), ("build", StageState::Blocked));
//!
//! // every question answered: complete, and the stage is the last one
//! let s = derive_stage(&stages, &[q("committed", GateStatus::Pass), q("ci", GateStatus::Exempt)]);
//! assert!(s.complete);
//! assert_eq!(s.state, StageState::Complete);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::done::DoneQuestion;
use super::judge::GateStatus;
use super::policy::StageDecl;

/// Where one stage stands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum StageState {
    /// Every question of the stage passes or is exempt.
    Complete,
    /// A question of the stage has not been answered: queued or unknown.
    Pending,
    /// A question of the stage refuses: failing, stale, or blocked by one that is.
    Blocked,
    /// The stage declares no question that applies here.
    Empty,
}

impl StageState {
    /// The word, as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Pending => "pending",
            Self::Blocked => "blocked",
            Self::Empty => "empty",
        }
    }
}

/// One stage of the lifecycle, with what its questions said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StageReport {
    /// The stage's identity.
    pub id: String,
    /// The stage as a heading.
    pub title: String,
    /// Where it stands.
    pub state: StageState,
    /// The questions of this stage, in policy order.
    pub questions: Vec<String>,
    /// The questions still owed or refusing, in policy order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owing: Vec<String>,
}

/// Where the task stands: the one stage a reader acts on, and every stage behind and
/// ahead of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LifecycleStage {
    /// The stage the task stands at: the first one blocked, else the first one pending,
    /// else the last stage of the policy.
    pub id: String,
    /// That stage as a heading.
    pub title: String,
    /// Where it stands.
    pub state: StageState,
    /// True only when every stage is complete or empty. The word `completed` is earned
    /// here and nowhere else.
    pub complete: bool,
    /// The questions still owed or refusing at the stage the task stands at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owing: Vec<String>,
    /// Every stage of the policy, in order.
    pub stages: Vec<StageReport>,
}

/// Fold the answered questions over the policy's stages.
pub fn derive_stage(stages: &[StageDecl], questions: &[DoneQuestion]) -> LifecycleStage {
    let reports: Vec<StageReport> = stages
        .iter()
        .map(|s| {
            let mine: Vec<&DoneQuestion> = questions.iter().filter(|q| q.stage == s.id).collect();
            let owing: Vec<String> = mine
                .iter()
                .filter(|q| !matches!(q.status, GateStatus::Pass | GateStatus::Exempt))
                .map(|q| q.id.clone())
                .collect();
            let state = if mine.is_empty() {
                StageState::Empty
            } else if mine.iter().any(|q| q.status.refuses()) {
                StageState::Blocked
            } else if owing.is_empty() {
                StageState::Complete
            } else {
                StageState::Pending
            };
            StageReport {
                id: s.id.clone(),
                title: s.title.clone(),
                state,
                questions: mine.iter().map(|q| q.id.clone()).collect(),
                owing,
            }
        })
        .collect();

    let at = reports
        .iter()
        .find(|r| r.state == StageState::Blocked)
        .or_else(|| reports.iter().find(|r| r.state == StageState::Pending))
        .or_else(|| reports.last());
    let complete = reports
        .iter()
        .all(|r| matches!(r.state, StageState::Complete | StageState::Empty))
        && !reports.is_empty();
    match at {
        Some(r) => LifecycleStage {
            id: r.id.clone(),
            title: r.title.clone(),
            state: if complete {
                StageState::Complete
            } else {
                r.state
            },
            complete,
            owing: r.owing.clone(),
            stages: reports.clone(),
        },
        None => LifecycleStage {
            id: String::new(),
            title: String::new(),
            state: StageState::Empty,
            complete: false,
            owing: Vec::new(),
            stages: reports,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(id: &str) -> StageDecl {
        StageDecl {
            id: id.into(),
            title: id.to_uppercase(),
            summary: "s".into(),
        }
    }
    fn q(id: &str, stage: &str, status: GateStatus) -> DoneQuestion {
        DoneQuestion {
            id: id.into(),
            stage: stage.into(),
            question: id.into(),
            status,
            evidence: "e".into(),
            source: "s".into(),
            remediation: "r".into(),
        }
    }

    #[test]
    fn unknown_is_pending_never_complete() {
        let s = derive_stage(&[decl("a")], &[q("x", "a", GateStatus::Unknown)]);
        assert_eq!(s.state, StageState::Pending);
        assert!(!s.complete);
        assert_eq!(s.owing, ["x"]);
    }

    #[test]
    fn a_blocked_later_stage_wins_over_an_earlier_pending_one() {
        let s = derive_stage(
            &[decl("a"), decl("b")],
            &[
                q("x", "a", GateStatus::Queued),
                q("y", "b", GateStatus::Fail),
            ],
        );
        assert_eq!((s.id.as_str(), s.state), ("b", StageState::Blocked));
        assert_eq!(s.stages[0].state, StageState::Pending);
    }

    #[test]
    fn an_empty_stage_neither_blocks_nor_completes_on_its_own() {
        let s = derive_stage(&[decl("a"), decl("b")], &[q("x", "a", GateStatus::Pass)]);
        assert!(s.complete, "an empty stage is not owed");
        assert_eq!(s.stages[1].state, StageState::Empty);
        let none = derive_stage(&[], &[]);
        assert!(!none.complete, "no policy is no completion");
    }

    #[test]
    fn the_order_is_the_policy_s_and_a_question_of_no_stage_is_ignored() {
        let s = derive_stage(
            &[decl("b"), decl("a")],
            &[
                q("x", "a", GateStatus::Queued),
                q("y", "b", GateStatus::Queued),
                q("z", "zz", GateStatus::Fail),
            ],
        );
        assert_eq!(s.id, "b", "the first pending stage in policy order");
        assert_eq!(
            s.stages.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
    }
}
