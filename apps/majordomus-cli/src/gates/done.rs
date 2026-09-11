//! The "done" invariant: the questions a task must answer before anybody may call it
//! finished, each answered from the one place that already knows.
//!
//! # Why this is a projection and not a checklist
//!
//! Nineteen questions get asked of every piece of work in this repository — is it tested,
//! is it documented, are the projections current, did the version move, is it pushed, is it
//! on the trunk, is it published, is CI green, was the deployment verified, is a stale
//! branch left behind. Written as a checklist they would be nineteen new opinions, each
//! able to disagree with the subsystem that already holds the fact, and a checklist is
//! exactly the artifact a worker learns to tick.
//!
//! So every question here names a **source** and takes its answer from it:
//!
//! - an **obligation** of `share/obligations.yaml`, judged by
//!   [`crate::capability::builtin::obligations`] — which is where `implemented`, `tested`,
//!   `documented`, `generated`, `committed`, `pushed`, `integrated`, `published`,
//!   `deployed` and `deployment-verified` come from. Nothing is re-judged here: the state
//!   word comes back from that capability's own execution;
//! - a **gate** of the CI model, judged by [`super::judge`] — which is where the CI verdict
//!   and the release questions come from, when the model declares gates for them;
//! - the **change set** itself, for the one question it settles: whether a test path is
//!   among the files this task touched;
//! - or **nothing here**, in which case the answer is [`super::GateStatus::Unknown`] and
//!   the question carries the command that would answer it. An honest gap beats a boolean
//!   conjured by a report, and this is where the gaps are written down rather than left for
//!   a reader to infer.
//!
//! # The rule about absence
//!
//! No question here invents a `pass`. A question whose source has not spoken is `queued`
//! (it was asked and nothing answered) or `unknown` (it could not be asked), never `pass`;
//! a question the change makes irrelevant is `exempt`. Those three are different findings
//! and a reader is entitled to all three.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::judge::{Gate, GateStatus};
use super::ImpliedObligation;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One question of the done invariant, and what answered it.
pub struct DoneQuestion {
    /// The question's identity, stable across reports.
    pub id: String,
    /// The question, as a person would ask it.
    pub question: String,
    /// The answer, in the same vocabulary every gate uses.
    pub status: GateStatus,
    /// What was actually read — never a restatement of the status.
    pub evidence: String,
    /// What answered it: an obligation token, a gate id, or the capability that would.
    pub source: String,
    /// What would settle it, as a command.
    pub remediation: String,
}

/// How one question is answered.
enum Answers {
    /// An obligation token of `share/obligations.yaml`.
    Obligation(&'static str),
    /// The aggregate of the gates the change selects.
    Gates,
    /// A gate of the CI model, by id, when the model declares one.
    Gate(&'static str),
    /// The change set itself.
    Change,
    /// Nothing reachable from here; the command that would answer it.
    Elsewhere(&'static str),
}

/// The invariant, declared once: the question, what answers it, and what a worker runs.
///
/// The order is the order the work happens in, so a reader meets the questions in the order
/// they become answerable rather than in alphabetical order.
#[allow(clippy::type_complexity)]
fn invariant() -> Vec<(&'static str, &'static str, Answers, &'static str)> {
    vec![
        (
            "implemented",
            "Is all the work this task promised present in the tree?",
            Answers::Obligation("implementation"),
            "majordomus check",
        ),
        (
            "tested",
            "Were the cases the change obliges run, and did they pass?",
            Answers::Obligation("tests"),
            "majordomus usecase impact",
        ),
        (
            "regression-tested",
            "Does the change touch a test of its own?",
            Answers::Change,
            "add a case under test/cases/ or a test beside the code",
        ),
        (
            "documented",
            "Were the documents a reader needs updated?",
            Answers::Obligation("docs"),
            "scripts/ci/reference-check",
        ),
        (
            "generated",
            "Is every derived artifact current with its canonical source?",
            Answers::Obligation("generated"),
            "majordomus generate --check",
        ),
        (
            "openapi",
            "Are the OpenAPI document and the transport registries current?",
            Answers::Obligation("generated"),
            "majordomus generate --check",
        ),
        (
            "parity",
            "Does every surface — command line, HTTP, MCP, Cockpit — agree with the registry?",
            Answers::Elsewhere("majordomus quality report"),
            "majordomus quality report",
        ),
        (
            "committed",
            "Are the changes in the branch's history rather than in the working tree?",
            Answers::Obligation("commit"),
            "git commit",
        ),
        (
            "pushed",
            "Has the commit reached the remote?",
            Answers::Obligation("push"),
            "git push",
        ),
        (
            "ci",
            "Has every gate the change selects reported, and did it pass?",
            Answers::Gates,
            "majordomus evidence --gate <id> --exit <status>",
        ),
        (
            "version",
            "Has the declared version moved as far as the public surface has?",
            Answers::Gate("version-surface"),
            "majordomus release version",
        ),
        (
            "changelog",
            "Is the release's own record current with the tree?",
            Answers::Gate("release-check"),
            "scripts/ci/release-check",
        ),
        (
            "integrated",
            "Does the trunk reach this commit?",
            Answers::Obligation("target"),
            "open a pull request and land it",
        ),
        (
            "published",
            "Does the published site serve this commit?",
            Answers::Obligation("pages"),
            "scripts/pages verify --commit HEAD",
        ),
        (
            "deployed",
            "Is the environment this change targets running it?",
            Answers::Obligation("deploy"),
            "majordomus deployment",
        ),
        (
            "deployment-verified",
            "Was the deployed result looked at after it was deployed?",
            Answers::Obligation("verify"),
            "majordomus evidence --covers verify --command '<the live check>'",
        ),
        (
            "no-stale-topology",
            "Is a branch, worktree or pull request left behind?",
            Answers::Elsewhere("majordomus worktree && majordomus doctor"),
            "majordomus worktree repair",
        ),
        (
            "handover",
            "Is the session's continuation record written?",
            Answers::Elsewhere("majordomus handover"),
            "majordomus handover < note.md",
        ),
        (
            "issue",
            "Do the issue and milestone this work belongs to say so?",
            Answers::Elsewhere("majordomus plan status"),
            "majordomus plan record",
        ),
    ]
}

/// The state of one obligation as [`crate::capability::builtin::obligations`] judged it:
/// the token, its state word, and the line the validator would refuse with.
#[derive(Debug, Clone, Default)]
pub struct ObligationStanding {
    /// The state word: `discharged`, `owed`, `stale` or `undeclared`.
    pub state: String,
    /// What the closure said about it, verbatim.
    pub detail: String,
    /// The command that would discharge it, as the closure gave it.
    pub reproduce: String,
}

/// Answer the invariant.
///
/// Every argument is a judgement somebody else already made: `standing` is the obligation
/// closure's, `gates` is [`super::judge`]'s, `implied` is [`super::implied`]'s, and
/// `changed` is the change set. Nothing is measured here — this composes.
pub fn answer(
    standing: &std::collections::BTreeMap<String, ObligationStanding>,
    implied: &[ImpliedObligation],
    gates: &[Gate],
    changed: &[String],
    closure_reachable: bool,
) -> Vec<DoneQuestion> {
    invariant()
        .into_iter()
        .map(|(id, question, from, remediation)| {
            let (status, evidence, source) = match from {
                Answers::Obligation(token) => {
                    let applicable = implied
                        .iter()
                        .find(|o| o.id == token)
                        .map(|o| (o.applicable, o.declared, o.reason.clone()));
                    match (standing.get(token), applicable) {
                        // the closure judged it: that word is the answer, translated once
                        (Some(s), _) => (
                            match s.state.as_str() {
                                "discharged" => GateStatus::Pass,
                                "stale" => GateStatus::Stale,
                                "owed" => GateStatus::Queued,
                                _ => GateStatus::Unknown,
                            },
                            s.detail.clone(),
                            format!("obligation {token} (obligations.closure)"),
                        ),
                        // the change implies it and the task never promised it: owed by the
                        // change and held by nothing, which is a debt and not a pass
                        (None, Some((true, false, why))) => (
                            GateStatus::Queued,
                            format!(
                                "the change implies this obligation and the task does not declare it, \
                                 so nothing holds it to it: {why}"
                            ),
                            format!("obligation {token} (derived from its own inputs)"),
                        ),
                        (None, Some((false, _, why))) => (
                            GateStatus::Exempt,
                            why,
                            format!("obligation {token} (derived from its own inputs)"),
                        ),
                        _ if !closure_reachable => (
                            GateStatus::Unknown,
                            "the obligation closure could not be read in this checkout".into(),
                            format!("obligation {token}"),
                        ),
                        _ => (
                            GateStatus::Unknown,
                            format!("share/obligations.yaml declares no token '{token}' here"),
                            format!("obligation {token}"),
                        ),
                    }
                }
                Answers::Gates => {
                    let required: Vec<&Gate> = gates.iter().filter(|g| g.required).collect();
                    let refused = required.iter().filter(|g| g.status.refuses()).count();
                    let silent = required.iter().filter(|g| g.status.unverified()).count();
                    let passing = required
                        .iter()
                        .filter(|g| g.status == GateStatus::Pass)
                        .count();
                    let status = if required.is_empty() {
                        GateStatus::Exempt
                    } else if refused > 0 {
                        GateStatus::Fail
                    } else if silent > 0 {
                        GateStatus::Queued
                    } else {
                        GateStatus::Pass
                    };
                    (
                        status,
                        format!(
                            "{} gate(s) selected: {passing} passing, {refused} refusing, \
                             {silent} never reported",
                            required.len()
                        ),
                        ".ai/repo/ci/gates.yaml".to_string(),
                    )
                }
                Answers::Gate(gate) => match gates.iter().find(|g| g.id == gate) {
                    Some(g) => (
                        g.status,
                        g.reason.clone(),
                        format!("gate {gate} (.ai/repo/ci/gates.yaml)"),
                    ),
                    None => (
                        GateStatus::Unknown,
                        format!("this repository's CI model declares no gate '{gate}'"),
                        format!("gate {gate}"),
                    ),
                },
                Answers::Change => {
                    let tests: Vec<&String> = changed
                        .iter()
                        .filter(|p| is_test_path(p))
                        .take(3)
                        .collect();
                    if changed.is_empty() {
                        (
                            GateStatus::Exempt,
                            "the task has changed nothing".into(),
                            "the change set".to_string(),
                        )
                    } else if tests.is_empty() {
                        (
                            GateStatus::Queued,
                            format!(
                                "no test path among the {} file(s) this task touched",
                                changed.len()
                            ),
                            "the change set (profile verification.regression_test_required)"
                                .to_string(),
                        )
                    } else {
                        (
                            GateStatus::Pass,
                            format!(
                                "{} test path(s) touched, including {}",
                                tests.len(),
                                tests[0]
                            ),
                            "the change set (profile verification.regression_test_required)"
                                .to_string(),
                        )
                    }
                }
                Answers::Elsewhere(command) => (
                    GateStatus::Unknown,
                    format!(
                        "nothing in this report reaches that fact; `{command}` is what answers it"
                    ),
                    command.to_string(),
                ),
            };
            DoneQuestion {
                id: id.to_string(),
                question: question.to_string(),
                status,
                evidence,
                source,
                remediation: remediation.to_string(),
            }
        })
        .collect()
}

/// Is this a path a test lives at? `mj_validate_profile_requirements`' predicate, which is
/// the one this repository already refuses over; nothing stricter is invented here.
fn is_test_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/')
        .any(|seg| matches!(seg, "test" | "tests" | "spec" | "specs"))
        || name.contains("_test.")
        || name.contains(".test.")
        || name.contains("_spec.")
        || name.contains(".spec.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn implied_of(id: &str, applicable: bool, declared: bool) -> ImpliedObligation {
        ImpliedObligation {
            id: id.into(),
            title: id.into(),
            applicable,
            declared,
            reason: "because".into(),
            because: vec![],
            remediation: "run it".into(),
        }
    }

    fn gate(id: &str, status: GateStatus, required: bool) -> Gate {
        Gate {
            id: id.into(),
            title: id.into(),
            scope: "structure".into(),
            required,
            status,
            reason: "r".into(),
            remediation: "m".into(),
            source: "s".into(),
            evidence: None,
            inputs: vec![],
            inputs_hash: String::new(),
            evaluated_at: "now".into(),
        }
    }

    fn standing(state: &str) -> ObligationStanding {
        ObligationStanding {
            state: state.into(),
            detail: format!("the closure said {state}"),
            reproduce: "x".into(),
        }
    }

    #[test]
    fn every_question_carries_a_source_and_something_to_run() {
        let q = answer(&BTreeMap::new(), &[], &[], &[], false);
        assert_eq!(q.len(), 19, "the invariant is nineteen questions");
        for question in &q {
            assert!(!question.source.is_empty(), "{} has no source", question.id);
            assert!(!question.remediation.is_empty(), "{}", question.id);
            assert!(!question.evidence.is_empty(), "{}", question.id);
            assert_ne!(
                question.status,
                GateStatus::Pass,
                "{} passed with nothing behind it",
                question.id
            );
        }
    }

    #[test]
    fn an_obligation_answer_is_the_closures_word_and_not_a_second_opinion() {
        let mut s = BTreeMap::new();
        s.insert("tests".to_string(), standing("discharged"));
        s.insert("docs".to_string(), standing("stale"));
        s.insert("push".to_string(), standing("owed"));
        let q = answer(&s, &[], &[], &[], true);
        let by = |id: &str| q.iter().find(|q| q.id == id).unwrap().clone();
        assert_eq!(by("tested").status, GateStatus::Pass);
        assert_eq!(by("documented").status, GateStatus::Stale);
        assert_eq!(by("pushed").status, GateStatus::Queued);
        assert!(by("tested")
            .evidence
            .contains("the closure said discharged"));
        assert!(by("tested").source.contains("obligations.closure"));
    }

    #[test]
    fn an_obligation_the_change_implies_and_nobody_declared_is_a_debt_not_a_pass() {
        let implied = vec![
            implied_of("tests", true, false),
            implied_of("deploy", false, false),
        ];
        let q = answer(&BTreeMap::new(), &implied, &[], &[], true);
        let by = |id: &str| q.iter().find(|q| q.id == id).unwrap().clone();
        assert_eq!(by("tested").status, GateStatus::Queued);
        assert!(by("tested").evidence.contains("does not declare it"));
        assert_eq!(
            by("deployed").status,
            GateStatus::Exempt,
            "a task that deploys nothing is not owed a deployment"
        );
    }

    #[test]
    fn the_ci_question_separates_refused_from_never_reported() {
        let none = answer(&BTreeMap::new(), &[], &[], &[], true);
        assert_eq!(
            none.iter().find(|q| q.id == "ci").unwrap().status,
            GateStatus::Exempt
        );

        let silent = vec![
            gate("a", GateStatus::Queued, true),
            gate("b", GateStatus::Pass, true),
        ];
        let q = answer(&BTreeMap::new(), &[], &silent, &[], true);
        assert_eq!(
            q.iter().find(|q| q.id == "ci").unwrap().status,
            GateStatus::Queued
        );

        let red = vec![
            gate("a", GateStatus::Fail, true),
            gate("b", GateStatus::Queued, true),
        ];
        let q = answer(&BTreeMap::new(), &[], &red, &[], true);
        let ci = q.iter().find(|q| q.id == "ci").unwrap();
        assert_eq!(ci.status, GateStatus::Fail, "a refusal outranks a silence");
        assert!(ci.evidence.contains("1 refusing") && ci.evidence.contains("1 never reported"));

        let green = vec![
            gate("a", GateStatus::Pass, true),
            gate("b", GateStatus::Exempt, false),
        ];
        let q = answer(&BTreeMap::new(), &[], &green, &[], true);
        assert_eq!(
            q.iter().find(|q| q.id == "ci").unwrap().status,
            GateStatus::Pass
        );
    }

    #[test]
    fn a_release_question_a_model_declares_no_gate_for_is_unknown_not_exempt() {
        let q = answer(
            &BTreeMap::new(),
            &[],
            &[gate("version-surface", GateStatus::Pass, true)],
            &[],
            true,
        );
        assert_eq!(
            q.iter().find(|q| q.id == "version").unwrap().status,
            GateStatus::Pass
        );
        let missing = q.iter().find(|q| q.id == "changelog").unwrap();
        assert_eq!(missing.status, GateStatus::Unknown);
        assert!(missing.evidence.contains("declares no gate"));
    }

    #[test]
    fn regression_is_answered_from_the_change_set_alone() {
        let q = answer(&BTreeMap::new(), &[], &[], &["lib/a.sh".to_string()], true);
        let r = q.iter().find(|q| q.id == "regression-tested").unwrap();
        assert_eq!(r.status, GateStatus::Queued);
        assert!(r.evidence.contains("no test path"));

        let q = answer(
            &BTreeMap::new(),
            &[],
            &[],
            &[
                "lib/a.sh".to_string(),
                "test/cases/131_completion_gates.sh".to_string(),
            ],
            true,
        );
        assert_eq!(
            q.iter()
                .find(|q| q.id == "regression-tested")
                .unwrap()
                .status,
            GateStatus::Pass
        );
    }

    #[test]
    fn a_test_path_is_recognised_the_way_the_validator_recognises_one() {
        assert!(is_test_path("test/cases/131.sh"));
        assert!(is_test_path("apps/majordomus-cli/tests/cli.rs"));
        assert!(is_test_path("src/foo_test.rs"));
        assert!(is_test_path("web/a.spec.ts"));
        assert!(!is_test_path("lib/check.sh"));
        assert!(
            !is_test_path("docs/latest.md"),
            "a substring is not a segment"
        );
    }

    #[test]
    fn a_question_nothing_here_reaches_says_so_and_names_the_command() {
        let q = answer(&BTreeMap::new(), &[], &[], &[], true);
        for id in ["parity", "no-stale-topology", "handover", "issue"] {
            let question = q.iter().find(|q| q.id == id).unwrap();
            assert_eq!(question.status, GateStatus::Unknown, "{id}");
            assert!(
                question.evidence.contains("majordomus"),
                "{id} names no command"
            );
        }
    }
}
