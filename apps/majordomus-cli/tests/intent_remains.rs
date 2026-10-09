//! What remains of an intent (ADR 0117), in process: every combination of an unmet
//! criterion's evidence state, the work covering it and the gap's observation is checked
//! against the row of the ADR's table whose own predicate it satisfies — written here
//! independently of the function's order — and the intent's outcome against its own order.

// claims: intent-remains-derived

use majordomus_cli::intent::{IntentEvidenceState as E, IntentGuard, IntentVerdictState as V};
use majordomus_cli::intent_plan::{CoverageStrength as C, CoveringIssue};
use majordomus_cli::intent_remains::{
    next, outcome, remains, Basis, IntentOutcome as O, NextAction, Remains as R,
};

/// The five states an unmet criterion's evidence can be in; `current` is met.
const UNMET: [E; 5] = [
    E::Stale,
    E::Failing,
    E::NotRun,
    E::NotDerivable,
    E::Unresolved,
];

fn issue(id: &str, status: &str) -> CoveringIssue {
    CoveringIssue {
        id: id.into(),
        milestone: "m".into(),
        status: status.into(),
        evidence_need: 1,
    }
}

/// The work that can cover a criterion, by name: none, done, open, blocked, a mix.
fn works() -> Vec<(&'static str, Vec<CoveringIssue>)> {
    vec![
        ("none", vec![]),
        ("done", vec![issue("I1", "DONE")]),
        ("ready", vec![issue("I1", "READY")]),
        ("active", vec![issue("I1", "ACTIVE")]),
        ("verify", vec![issue("I1", "VERIFY")]),
        ("blocked", vec![issue("I1", "BLOCKED")]),
        (
            "blocked+done",
            vec![issue("I1", "BLOCKED"), issue("I2", "DONE")],
        ),
        (
            "blocked+active",
            vec![issue("I1", "BLOCKED"), issue("I2", "ACTIVE")],
        ),
        ("done+done", vec![issue("I1", "DONE"), issue("I2", "DONE")]),
    ]
}

/// Each row of ADR 0117's table as its own predicate, in the ADR's order. A combination's
/// expected answer is the first whose predicate holds.
fn expected(state: E, work: &[CoveringIssue], observed: bool) -> (R, Basis) {
    let open: Vec<&CoveringIssue> = work
        .iter()
        .filter(|i| ["READY", "BLOCKED", "ACTIVE", "VERIFY"].contains(&i.status.as_str()))
        .collect();
    let rows: [(bool, (R, Basis)); 11] = [
        (
            !open.is_empty() && open.iter().all(|i| i.status == "BLOCKED"),
            (R::Blocked, Basis::WorkBlocked),
        ),
        (
            !open.is_empty() && state == E::Failing,
            (R::Progressing, Basis::WorkOpenFailing),
        ),
        (!open.is_empty(), (R::Progressing, Basis::WorkOpen)),
        (state == E::NotDerivable, (R::Unknown, Basis::NotDerivable)),
        (
            state == E::Failing && !work.is_empty(),
            (R::Failed, Basis::ClosedWork),
        ),
        (
            state == E::Failing && observed,
            (R::Failed, Basis::GapObservation),
        ),
        (state == E::Failing, (R::Failed, Basis::NoWork)),
        (!work.is_empty(), (R::NeedsEvidence, Basis::ClosedWork)),
        (observed, (R::NeedsEvidence, Basis::GapObservation)),
        (state == E::Stale, (R::NeedsEvidence, Basis::PassedBefore)),
        (true, (R::Exhausted, Basis::NoWork)),
    ];
    rows.into_iter().find(|(holds, _)| *holds).unwrap().1
}

/// Criterion `remains-and-outcome`, the table: every combination meets the row its own
/// predicate names, and every row is met by some combination.
#[test]
fn every_combination_meets_the_row_its_predicate_names() {
    let mut met = std::collections::BTreeSet::new();
    for state in UNMET {
        for (name, work) in works() {
            for strength in [C::Uncovered, C::Observed, C::Weak, C::Covered] {
                // coverage is observed only where no live issue serves the criterion
                if strength == C::Observed && !work.is_empty() {
                    continue;
                }
                let observed = strength == C::Observed;
                let want = expected(state, &work, observed);
                let got = remains(state, &work, strength);
                assert_eq!(got, want, "{state:?} with {name} work, {strength:?}");
                met.insert(format!("{:?}/{:?}", want.0, want.1));
            }
        }
    }
    assert_eq!(
        met.len(),
        11,
        "every row of the table is reachable: {met:?}"
    );
}

/// The answer is a function of its inputs: asked twice, it answers the same.
#[test]
fn the_same_inputs_answer_the_same() {
    for state in UNMET {
        for (_, work) in works() {
            assert_eq!(
                remains(state, &work, C::Covered),
                remains(state, &work, C::Covered)
            );
        }
    }
}

/// Criterion `remains-and-outcome`, the next action: each answer justifies the action the
/// ADR names for it, and none is left without one.
#[test]
fn every_remains_has_the_next_action_the_adr_names() {
    let reproduce = Some("bash test/run.sh x");
    let blocked = [issue("I1", "BLOCKED"), issue("I2", "DONE")];
    match next(
        "x",
        "a",
        (R::Blocked, Basis::WorkBlocked),
        &blocked,
        reproduce,
    ) {
        NextAction::Unblock { issues } => assert_eq!(issues, ["I1"]),
        other => panic!("{other:?}"),
    }
    let moving = [issue("I1", "ACTIVE"), issue("I2", "DONE")];
    match next(
        "x",
        "a",
        (R::Progressing, Basis::WorkOpen),
        &moving,
        reproduce,
    ) {
        NextAction::Work { issues } => assert_eq!(issues, [issue("I1", "ACTIVE")]),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        next("x", "a", (R::Failed, Basis::ClosedWork), &[], reproduce),
        NextAction::Repair {
            reproduce: reproduce.map(String::from)
        }
    );
    for basis in [Basis::GapObservation, Basis::NoWork] {
        assert_eq!(
            next("x", "a", (R::Failed, basis), &[], reproduce),
            NextAction::PlanWork {
                serves: "x#a".into()
            }
        );
    }
    assert_eq!(
        next("x", "a", (R::Exhausted, Basis::NoWork), &[], None),
        NextAction::PlanWork {
            serves: "x#a".into()
        }
    );
    for basis in [
        Basis::ClosedWork,
        Basis::GapObservation,
        Basis::PassedBefore,
    ] {
        assert_eq!(
            next("x", "a", (R::NeedsEvidence, basis), &[], reproduce),
            NextAction::RecordEvidence {
                reproduce: reproduce.map(String::from)
            }
        );
    }
    assert!(matches!(
        next("x", "a", (R::Unknown, Basis::NotDerivable), &[], None),
        NextAction::None { .. }
    ));
}

fn guard(violated: bool) -> IntentGuard {
    serde_json::from_value(serde_json::json!({
        "id": "g", "invariant": "stays true", "evidence": "test", "ref": "t.sh",
        "state": if violated { "failing" } else { "current" }, "violated": violated,
    }))
    .unwrap()
}

/// Criterion `remains-and-outcome`, the intent: its order, optional criteria, a violated guard,
/// and an intent with nothing required unmet.
#[test]
fn an_intents_outcome_follows_the_adr_order() {
    let all = [
        R::Blocked,
        R::Progressing,
        R::Unknown,
        R::NeedsEvidence,
        R::Exhausted,
        R::Failed,
    ];
    let words = [
        O::Blocked,
        O::Progressing,
        O::Unknown,
        O::NeedsEvidence,
        O::Exhausted,
        O::Failed,
    ];
    // each answer added to everything that ranks below it decides the outcome
    for n in 0..all.len() {
        let unmet: Vec<(R, bool)> = all[..=n].iter().map(|r| (*r, false)).collect();
        assert_eq!(outcome(V::Unsatisfied, &[], &unmet), words[n], "{unmet:?}");
    }
    // optional criteria decide nothing
    let unmet = [(R::Progressing, false), (R::Failed, true)];
    assert_eq!(outcome(V::Unsatisfied, &[], &unmet), O::Progressing);
    // nothing required is unmet and the verdict is not satisfied
    assert_eq!(outcome(V::Unknown, &[], &[(R::Failed, true)]), O::Unknown);
    assert_eq!(outcome(V::Unknown, &[], &[]), O::Unknown);
    // a violated guard fails the intent whatever its criteria; one that holds does not
    assert_eq!(outcome(V::Unsatisfied, &[guard(true)], &unmet), O::Failed);
    assert_eq!(
        outcome(V::Unsatisfied, &[guard(false)], &unmet),
        O::Progressing
    );
    // a satisfied verdict is satisfied
    assert_eq!(outcome(V::Satisfied, &[], &[]), O::Satisfied);
}
