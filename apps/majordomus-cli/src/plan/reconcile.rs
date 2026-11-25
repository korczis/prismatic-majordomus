//! What a merge supports the plan recording, and what each issue still owes: the proposal
//! `plan.reconcile` answers, and `majordomus plan reconcile --apply` writes through
//! `plan.transition` (ADR 0238, I4773).
//!
//! [`super::delivery`] says how far each issue's work has travelled, and [`super::drift`]
//! names where the stamps contradict it. Neither says what to do. Settling it by hand cost
//! one adopter repository hours: one `plan start|verify|evidence|done` at a time, each issue
//! looked up, its merge found, its validation re-run. This module derives the moves instead:
//!
//! - a merge proves that execution began, so a READY issue is proposed `start`;
//! - `done` is proposed only where the done guard (`plan::check`, asked through the
//!   caller's `admits`) admits it, the issue declares the evidence it requires, and every
//!   covering entry was recorded at or after the newest merge naming the issue: evidence
//!   older than the delivery is not evidence of it;
//! - `verify` is never proposed. It is a worker's attestation that the implementation is
//!   complete, and a merge can be one slice of a split issue;
//! - a BLOCKED issue is held, with the guard's own reason; an issue another open pull
//!   request still names is still landing, and nothing is proposed for it.
//!
//! Each proposal carries the proof (the merge and how it names the issue), every required
//! token covered, stale or missing, the declared validation commands that would cover them,
//! and every acceptance criterion as `unjudged`: a criterion is prose, and a merge is not a
//! criterion met. Nothing here writes; the command line applies the moves, one
//! `plan.transition` each, re-reading the plan between them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::delivery::{
    DeliveryMerge, DeliveryObservation, DeliveryReading, DeliverySource, DeliveryStage,
    IssueDeliveryReport,
};
use super::{IssueOwes, Transition};

/// The version of the report's shape.
pub(crate) const RECONCILE_SCHEMA: u32 = 1;

/// Exit code of an `--apply` one of whose moves was refused.
pub(crate) const EXIT_REFUSED: u8 = 10;

/// Exit code of an `--apply` over a proposal that is not settled (the forge unread).
pub(crate) const EXIT_UNSETTLED: u8 = 12;

/// What a merge supports for one issue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReconcileClass {
    /// `start` is proposed and `done` is not.
    Recordable,
    /// `done` is admitted: the evidence it requires is recorded, and recorded since the merge.
    Acceptable,
    /// The guard admits no move: BLOCKED, or a `done` held by something other than evidence.
    Held,
    /// ACTIVE or VERIFY, merged, with evidence missing or older than the merge.
    Unaccepted,
    /// Another open pull request still names it: the work is still landing.
    PendingPr,
}

impl ReconcileClass {
    /// Every class, in order.
    pub(crate) const ALL: [ReconcileClass; 5] = [
        ReconcileClass::Recordable,
        ReconcileClass::Acceptable,
        ReconcileClass::Held,
        ReconcileClass::Unaccepted,
        ReconcileClass::PendingPr,
    ];

    /// The word the report spells it with.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ReconcileClass::Recordable => "recordable",
            ReconcileClass::Acceptable => "acceptable",
            ReconcileClass::Held => "held",
            ReconcileClass::Unaccepted => "unaccepted",
            ReconcileClass::PendingPr => "pending_pr",
        }
    }
}

/// One `plan.transition` the plan admits now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ReconcileMove {
    /// The issue.
    pub issue: String,
    /// The move, as `plan.transition` takes it.
    pub transition: Transition,
    /// The same move on the command line.
    pub command: String,
}

/// A move the plan does not admit, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ReconcileRefusal {
    /// The move.
    pub transition: Transition,
    /// The guard's reason, or the evidence that is owed.
    pub reason: String,
}

/// One required evidence token, and whether the record covers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TokenStanding {
    /// The `evidence_required` token.
    pub token: String,
    /// An evidence entry covers it.
    pub covered: bool,
    /// A covering entry was recorded at or after the newest merge naming the issue.
    pub fresh: bool,
    /// The newest `recorded_at` among the covering entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
}

/// One acceptance criterion, as the record states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct CriterionStanding {
    /// The criterion's prose.
    pub criterion: String,
    /// Always `unjudged`: reconcile never reads a merge as a criterion met.
    pub standing: String,
}

/// What a merge supports for one issue, with everything that proves or holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ReconcileProposal {
    /// The issue id.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// Its milestone.
    pub milestone: String,
    /// Its declared status.
    pub status: String,
    /// Its inferred delivery stage.
    pub stage: DeliveryStage,
    /// What a merge supports.
    pub class: ReconcileClass,
    /// The newest merge naming it: its proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<DeliveryMerge>,
    /// The open pull request that still names it, for `pending_pr`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_pr: Option<u64>,
    /// The moves the plan admits now, in order.
    pub moves: Vec<ReconcileMove>,
    /// The moves it does not, with why.
    pub refused: Vec<ReconcileRefusal>,
    /// Every required token.
    pub evidence: Vec<TokenStanding>,
    /// The declared validation commands: what a worker runs to cover a token.
    pub validation: Vec<String>,
    /// Every acceptance criterion, unjudged.
    pub acceptance: Vec<CriterionStanding>,
}

/// Every issue whose merged work the plan has not finished recording, with what may be
/// written now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ReconcileReport {
    /// The version of this shape.
    pub schema: u32,
    /// Where and when the delivery was observed.
    pub observed: DeliveryObservation,
    /// Every source the delivery read, or did not.
    pub sources: Vec<DeliverySource>,
    /// True when the forge was read: only then can "no open pull request names it" be said,
    /// and only then may the moves be applied.
    pub settled: bool,
    /// Why it is not settled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsettled: Option<String>,
    /// The class vocabulary, in order.
    pub classes: Vec<ReconcileClass>,
    /// How many proposals of each class, keyed by every class.
    pub tallies: BTreeMap<String, usize>,
    /// The proposals, in issue order.
    pub proposals: Vec<ReconcileProposal>,
    /// How many moves are proposed in all.
    pub moves: usize,
}

/// The tokens of `owes` that no evidence entry covers, in declared order, each once.
pub(crate) fn missing_tokens(owes: &IssueOwes) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for t in &owes.evidence_required {
        if !owes.evidence.iter().any(|e| &e.covers == t) && !missing.contains(t) {
            missing.push(t.clone());
        }
    }
    missing
}

/// Each required token's standing against a merge made at `merged_at` (RFC 3339; stamps
/// compare as text). A covering entry without a time, or with a bare date, sorts before any
/// merge of that day: what cannot be shown to postdate the delivery does not.
pub(crate) fn standing(owes: &IssueOwes, merged_at: &str) -> Vec<TokenStanding> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for t in &owes.evidence_required {
        if seen.contains(&t.as_str()) {
            continue;
        }
        seen.push(t);
        let covering: Vec<&str> = owes
            .evidence
            .iter()
            .filter(|e| &e.covers == t)
            .map(|e| e.recorded_at.as_str())
            .collect();
        let recorded_at = covering
            .iter()
            .copied()
            .filter(|r| !r.is_empty())
            .max()
            .map(str::to_string);
        out.push(TokenStanding {
            token: t.clone(),
            covered: !covering.is_empty(),
            fresh: recorded_at
                .as_deref()
                .is_some_and(|r| r >= merged_at && r.len() > 10),
            recorded_at,
        });
    }
    out
}

/// Why the evidence does not admit `done`, or `None` when it does.
fn evidence_hold(evidence: &[TokenStanding]) -> Option<String> {
    if evidence.is_empty() {
        return Some(
            "the issue declares no evidence_required, so nothing a merge shows can accept it"
                .into(),
        );
    }
    let missing: Vec<&str> = evidence
        .iter()
        .filter(|t| !t.covered)
        .map(|t| t.token.as_str())
        .collect();
    let stale: Vec<&str> = evidence
        .iter()
        .filter(|t| t.covered && !t.fresh)
        .map(|t| t.token.as_str())
        .collect();
    let mut why = Vec::new();
    if !missing.is_empty() {
        why.push(format!("evidence missing for {}", missing.join(", ")));
    }
    if !stale.is_empty() {
        why.push(format!(
            "evidence for {} was not recorded after the merge",
            stale.join(", ")
        ));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

fn move_of(t: Transition, id: &str) -> ReconcileMove {
    ReconcileMove {
        issue: id.to_string(),
        transition: t,
        command: command(t, id),
    }
}

fn command(t: Transition, id: &str) -> String {
    let word = match t {
        Transition::Start => "start",
        Transition::Verify => "verify",
        Transition::Done => "done",
    };
    format!("majordomus plan {word} {id}")
}

/// The proposal over `delivery`, for the issues of `only` (every issue when empty).
///
/// `owes` is what each record declares it owes ([`super::owed`]). `admits(id, transition,
/// after_start)` is the guard: whether the plan admits the move now, or after the proposed
/// `start` when `after_start` is set, and the guard's reason when it does not. It is a
/// parameter so that this function stays pure; the capability passes `plan::check` over the
/// index. Pure.
pub(crate) fn propose(
    delivery: IssueDeliveryReport,
    owes: &BTreeMap<String, IssueOwes>,
    only: &[String],
    admits: &dyn Fn(&str, Transition, bool) -> Result<(), String>,
) -> ReconcileReport {
    let forge = delivery.sources.iter().find(|s| s.name == "forge");
    let unsettled = match forge {
        Some(s) if s.state == DeliveryReading::Read => None,
        Some(s) => Some(format!(
            "the forge was not read ({}): no open pull request naming an issue can be ruled out",
            s.detail
        )),
        None => Some(
            "the forge was not read: no open pull request naming an issue can be ruled out".into(),
        ),
    };
    let mut proposals = Vec::new();
    for d in &delivery.issues {
        if !only.is_empty() && !only.contains(&d.issue) {
            continue;
        }
        if d.stage < DeliveryStage::Merged
            || !matches!(d.status.as_str(), "READY" | "BLOCKED" | "ACTIVE" | "VERIFY")
        {
            continue;
        }
        let o = owes.get(&d.issue).cloned().unwrap_or_default();
        let merged_at = d.merged.as_ref().map_or("", |m| m.merged_at.as_str());
        let evidence = standing(&o, merged_at);
        let id = d.issue.as_str();
        let mut moves: Vec<ReconcileMove> = Vec::new();
        let mut refused: Vec<ReconcileRefusal> = Vec::new();
        // the `done` step, after a proposed start or from ACTIVE/VERIFY
        let done_step = |after_start: bool,
                         moves: &mut Vec<ReconcileMove>,
                         refused: &mut Vec<ReconcileRefusal>|
         -> ReconcileClass {
            let short = if after_start {
                ReconcileClass::Recordable
            } else {
                ReconcileClass::Unaccepted
            };
            if let Some(reason) = evidence_hold(&evidence) {
                refused.push(ReconcileRefusal {
                    transition: Transition::Done,
                    reason,
                });
                return short;
            }
            match admits(id, Transition::Done, after_start) {
                Ok(()) => {
                    moves.push(move_of(Transition::Done, id));
                    ReconcileClass::Acceptable
                }
                Err(reason) => {
                    refused.push(ReconcileRefusal {
                        transition: Transition::Done,
                        reason,
                    });
                    if after_start {
                        ReconcileClass::Recordable
                    } else {
                        ReconcileClass::Held
                    }
                }
            }
        };
        let class = if d.pr_open.is_some() {
            ReconcileClass::PendingPr
        } else if d.status == "READY" {
            match admits(id, Transition::Start, false) {
                Ok(()) => {
                    moves.push(move_of(Transition::Start, id));
                    done_step(true, &mut moves, &mut refused)
                }
                Err(reason) => {
                    refused.push(ReconcileRefusal {
                        transition: Transition::Start,
                        reason,
                    });
                    ReconcileClass::Held
                }
            }
        } else if d.status == "BLOCKED" {
            let reason = admits(id, Transition::Start, false)
                .err()
                .unwrap_or_else(|| format!("{id} is BLOCKED"));
            refused.push(ReconcileRefusal {
                transition: Transition::Start,
                reason,
            });
            ReconcileClass::Held
        } else {
            done_step(false, &mut moves, &mut refused)
        };
        proposals.push(ReconcileProposal {
            issue: d.issue.clone(),
            title: d.title.clone(),
            milestone: d.milestone.clone(),
            status: d.status.clone(),
            stage: d.stage,
            class,
            merge: d.merged.clone(),
            pending_pr: d.pr_open.as_ref().map(|p| p.number),
            moves,
            refused,
            evidence,
            validation: o.validation.clone(),
            acceptance: o
                .acceptance_criteria
                .iter()
                .map(|c| CriterionStanding {
                    criterion: c.clone(),
                    standing: "unjudged".into(),
                })
                .collect(),
        });
    }
    let mut tallies: BTreeMap<String, usize> = ReconcileClass::ALL
        .iter()
        .map(|c| (c.as_str().to_string(), 0))
        .collect();
    for p in &proposals {
        *tallies.entry(p.class.as_str().to_string()).or_default() += 1;
    }
    let moves = proposals.iter().map(|p| p.moves.len()).sum();
    ReconcileReport {
        schema: RECONCILE_SCHEMA,
        observed: delivery.observed,
        sources: delivery.sources,
        settled: unsettled.is_none(),
        unsettled,
        classes: ReconcileClass::ALL.to_vec(),
        tallies,
        proposals,
        moves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::delivery::{DeliveryPullRequest, IssueDelivery};
    use crate::plan::RecordedEvidence;

    fn source(name: &str, read: bool) -> DeliverySource {
        DeliverySource {
            name: name.into(),
            state: if read {
                DeliveryReading::Read
            } else {
                DeliveryReading::Unread
            },
            detail: if read {
                String::new()
            } else {
                "never observed".into()
            },
            observed_at: None,
        }
    }

    fn merged(id: &str, status: &str) -> IssueDelivery {
        IssueDelivery {
            issue: id.into(),
            title: format!("the {id}"),
            milestone: "m".into(),
            status: status.into(),
            stage: DeliveryStage::Merged,
            settled: true,
            unread: vec![],
            branch: None,
            pr_open: None,
            merged: Some(DeliveryMerge {
                commit: "c0ffee".into(),
                pull_request: Some(7),
                branch: Some("feature/x".into()),
                title: Some("x".into()),
                via: "trailer".into(),
                merged_at: "2026-11-22T12:00:00Z".into(),
                merges: 1,
                observed_at: "2026-11-25T00:00:00Z".into(),
            }),
            planned: None,
            released: None,
            deployed: None,
        }
    }

    fn report(issues: Vec<IssueDelivery>, forge: bool) -> IssueDeliveryReport {
        IssueDeliveryReport {
            schema: 1,
            observed: DeliveryObservation {
                mode: "live".into(),
                trunk: "master".into(),
                commit: "abc".into(),
                observed_at: "2026-11-25T00:00:00Z".into(),
            },
            stages: DeliveryStage::ALL.to_vec(),
            sources: vec![source("forge", forge)],
            tallies: BTreeMap::new(),
            unsettled: 0,
            issues,
        }
    }

    fn owes(tokens: &[&str], evidence: &[(&str, &str)]) -> IssueOwes {
        IssueOwes {
            evidence_required: tokens.iter().map(|t| (*t).to_string()).collect(),
            evidence: evidence
                .iter()
                .map(|(c, r)| RecordedEvidence {
                    covers: (*c).into(),
                    recorded_at: (*r).into(),
                })
                .collect(),
            acceptance_criteria: vec!["the work is done".into()],
            validation: vec!["cargo test".into()],
        }
    }

    /// The guard of a plan in which every move is admitted.
    fn admit_all(_: &str, _: Transition, _: bool) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn a_ready_merged_issue_is_started_and_done_only_on_evidence_recorded_since_the_merge() {
        let mut o = BTreeMap::new();
        o.insert("I1".to_string(), owes(&["rust"], &[]));
        o.insert(
            "I2".to_string(),
            owes(&["rust"], &[("rust", "2026-11-23T08:00:00Z")]),
        );
        o.insert(
            "I3".to_string(),
            owes(&["rust", "rust"], &[("rust", "2026-11-20T08:00:00Z")]),
        );
        let r = propose(
            report(
                vec![
                    merged("I1", "READY"),
                    merged("I2", "READY"),
                    merged("I3", "READY"),
                ],
                true,
            ),
            &o,
            &[],
            &admit_all,
        );
        assert!(r.settled && r.unsettled.is_none());
        let p = |id: &str| r.proposals.iter().find(|p| p.issue == id).unwrap();

        let i1 = p("I1");
        assert_eq!(i1.class, ReconcileClass::Recordable);
        assert_eq!(
            i1.moves
                .iter()
                .map(|m| m.command.as_str())
                .collect::<Vec<_>>(),
            ["majordomus plan start I1"]
        );
        assert!(
            i1.moves.iter().all(|m| m.transition != Transition::Verify),
            "verify is a worker's attestation, never proposed"
        );
        assert_eq!(i1.refused[0].reason, "evidence missing for rust");
        assert_eq!(i1.validation, ["cargo test"]);
        assert_eq!(i1.acceptance[0].standing, "unjudged");
        assert_eq!(i1.merge.as_ref().unwrap().via, "trailer");

        let i2 = p("I2");
        assert_eq!(i2.class, ReconcileClass::Acceptable);
        assert_eq!(
            i2.moves.iter().map(|m| m.transition).collect::<Vec<_>>(),
            [Transition::Start, Transition::Done]
        );
        assert!(i2.evidence[0].fresh);

        let i3 = p("I3");
        assert_eq!(i3.class, ReconcileClass::Recordable, "older than the merge");
        assert_eq!(i3.evidence.len(), 1, "a token declared twice stands once");
        assert!(i3.evidence[0].covered && !i3.evidence[0].fresh);
        assert_eq!(
            i3.refused[0].reason,
            "evidence for rust was not recorded after the merge"
        );
        assert_eq!(r.moves, 4);
        assert_eq!(r.tallies["recordable"], 2);
        assert_eq!(r.tallies["acceptable"], 1);
    }

    #[test]
    fn blocked_pending_and_guarded_issues_get_no_move_and_say_why() {
        let mut o = BTreeMap::new();
        o.insert(
            "I4".to_string(),
            owes(&["t"], &[("t", "2026-11-23T00:00:00Z")]),
        );
        o.insert(
            "I5".to_string(),
            owes(&["t"], &[("t", "2026-11-23T00:00:00Z")]),
        );
        o.insert("I6".to_string(), owes(&["t"], &[]));
        o.insert("I7".to_string(), owes(&[], &[]));
        let mut pending = merged("I3", "ACTIVE");
        pending.pr_open = Some(DeliveryPullRequest {
            number: 9,
            title: "more (I3)".into(),
            head_ref: "fix/i3".into(),
            draft: false,
            via: "title_or_branch".into(),
            observed_at: "t".into(),
        });
        let guard = |id: &str, t: Transition, after: bool| -> Result<(), String> {
            match (id, t, after) {
                ("I1", Transition::Start, _) => {
                    Err("I1 is BLOCKED, not READY (waiting on I0)".into())
                }
                ("I5", Transition::Done, false) => {
                    Err("I5 cannot be DONE while I0 is not DONE".into())
                }
                ("I8", Transition::Start, _) => Err("the binding is refused".into()),
                ("I9", Transition::Done, true) => Err("held after start".into()),
                _ => Ok(()),
            }
        };
        let mut o9 = o.clone();
        o9.insert(
            "I9".to_string(),
            owes(&["t"], &[("t", "2026-11-23T00:00:00Z")]),
        );
        let issues = vec![
            merged("I1", "BLOCKED"),
            merged("I2", "BLOCKED"),
            pending,
            merged("I4", "VERIFY"),
            merged("I5", "ACTIVE"),
            merged("I6", "VERIFY"),
            merged("I7", "ACTIVE"),
            merged("I8", "READY"),
            merged("I9", "READY"),
            merged("I10", "DONE"),
        ];
        let mut not_merged = merged("I11", "READY");
        not_merged.stage = DeliveryStage::PrOpen;
        let mut all = issues;
        all.push(not_merged);
        let r = propose(report(all, true), &o9, &[], &guard);
        let class = |id: &str| r.proposals.iter().find(|p| p.issue == id).map(|p| p.class);
        assert_eq!(class("I1"), Some(ReconcileClass::Held));
        assert_eq!(
            r.proposals[0].refused[0].reason,
            "I1 is BLOCKED, not READY (waiting on I0)"
        );
        let i2 = r.proposals.iter().find(|p| p.issue == "I2").unwrap();
        assert_eq!(
            i2.refused[0].reason, "I2 is BLOCKED",
            "a guard that admits says nothing"
        );
        assert_eq!(class("I3"), Some(ReconcileClass::PendingPr));
        assert_eq!(
            r.proposals
                .iter()
                .find(|p| p.issue == "I3")
                .unwrap()
                .pending_pr,
            Some(9)
        );
        assert_eq!(class("I4"), Some(ReconcileClass::Acceptable));
        assert_eq!(class("I5"), Some(ReconcileClass::Held));
        assert_eq!(class("I6"), Some(ReconcileClass::Unaccepted));
        assert_eq!(class("I7"), Some(ReconcileClass::Unaccepted));
        assert!(r
            .proposals
            .iter()
            .find(|p| p.issue == "I7")
            .unwrap()
            .refused[0]
            .reason
            .contains("declares no evidence_required"));
        assert_eq!(class("I8"), Some(ReconcileClass::Held));
        assert_eq!(class("I9"), Some(ReconcileClass::Recordable));
        assert_eq!(class("I10"), None, "DONE is recorded");
        assert_eq!(class("I11"), None, "not merged");

        let one = propose(
            report(vec![merged("I4", "VERIFY")], true),
            &o,
            &["I5".into()],
            &guard,
        );
        assert!(one.proposals.is_empty(), "only the issues asked");
    }

    #[test]
    fn an_unread_forge_leaves_the_proposal_unsettled() {
        let r = propose(
            report(vec![merged("I1", "READY")], false),
            &BTreeMap::new(),
            &[],
            &admit_all,
        );
        assert!(!r.settled);
        assert!(r.unsettled.as_deref().unwrap().contains("never observed"));
        let mut none = report(vec![], true);
        none.sources.clear();
        assert!(!propose(none, &BTreeMap::new(), &[], &admit_all).settled);
    }

    #[test]
    fn a_token_is_missing_until_an_entry_covers_it_whole() {
        let o = owes(&["rust", "test", "rust"], &[("rust-ish", ""), ("test", "")]);
        assert_eq!(missing_tokens(&o), ["rust"]);
        let s = standing(&o, "2026-11-22T12:00:00Z");
        assert!(
            s[1].covered && !s[1].fresh,
            "an entry with no time is not fresh"
        );
        let dated = owes(&["t"], &[("t", "2026-11-22")]);
        assert!(
            !standing(&dated, "2026-11-22T00:00:00Z")[0].fresh,
            "a bare date is not fresh"
        );
        assert_eq!(
            command(Transition::Verify, "I1"),
            "majordomus plan verify I1"
        );
        assert_eq!(ReconcileClass::PendingPr.as_str(), "pending_pr");
    }
}
