//! The "done" invariant: the questions a task must answer before anybody may call it
//! finished, each answered from the one place that already knows.
//!
//! # Why this is a projection and not a checklist
//!
//! Eighteen questions get asked of every piece of work in this repository — is it tested,
//! is it documented, are the projections current, did the version move, is it pushed, is it
//! on the trunk, is it published, is CI green, was the deployment verified. Written as a
//! checklist they would be eighteen new opinions, each able to disagree with the subsystem
//! that already holds the fact, and a checklist is exactly the artifact a worker learns to
//! tick.
//!
//! So the questions are **declared once**, in `share/completion.yaml` ([`super::policy`]),
//! and every question there names a **source**. This module takes each answer from that
//! source and from nowhere else:
//!
//! - an **obligation** of `share/obligations.yaml`, judged by
//!   [`crate::capability::builtin::obligations`] — which is where `implemented`, `tested`,
//!   `documented`, `generated`, `committed`, `pushed`, `integrated`, `published`,
//!   `deployed` and `deployment-verified` come from. Nothing is re-judged here: the state
//!   word comes back from that capability's own execution;
//! - a **gate** of the CI model, judged by [`super::judge`] — the CI question and the
//!   changelog and parity questions;
//! - the **structural release analysis** (`release.analysis`, ADR 0051), for the version
//!   question — computed from the tree against the last release, never recorded;
//! - the **change set** itself, for the one question it settles: whether a test path is
//!   among the files this task touched;
//! - the **task record**, for the issue it names, resolved against the plan;
//! - the **continuity store**, for whether a handover exists for this task;
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

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::judge::{Gate, GateStatus};
use super::policy::{CompletionPolicy, QuestionSource};
use super::ImpliedObligation;

/// ```
/// use majordomus_cli::gates::{DoneQuestion, GateStatus};
///
/// let q = DoneQuestion {
///     id: "pushed".into(),
///     stage: "commit".into(),
///     question: "Has the commit reached the remote?".into(),
///     status: GateStatus::Queued,
///     evidence: "the obligation is owed".into(),
///     source: "obligation push (obligations.closure)".into(),
///     remediation: "git push".into(),
/// };
/// // every question names what answered it and what would settle it: a question with
/// // neither is a checklist line, which is the artifact this type exists instead of
/// assert!(!q.source.is_empty() && !q.remediation.is_empty());
/// assert!(q.status.unverified());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One question of the done invariant, and what answered it.
pub struct DoneQuestion {
    /// The question's identity, stable across reports.
    pub id: String,
    /// The lifecycle stage the policy places it in.
    pub stage: String,
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

/// ```
/// use majordomus_cli::gates::ObligationStanding;
///
/// // the closure's own word, carried verbatim; nothing here re-judges it
/// let standing = ObligationStanding {
///     state: "owed".into(),
///     detail: "no evidence covers it".into(),
///     reproduce: "majordomus evidence --covers tests --command 'just test'".into(),
/// };
/// assert_eq!(standing.state, "owed");
/// ```
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

/// What the structural release analysis said, reduced to what the version question needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseStanding {
    /// The analysis could not be made; why.
    Unknown(String),
    /// There is nothing to measure against: the repository has published no release, so
    /// no baseline exists and no version is owed yet.
    NotApplicable(String),
    /// It was: whether the declared version is at least what the contract requires, and
    /// the sentence a reader is shown.
    Measured {
        /// Whether the declared version satisfies the contract.
        ok: bool,
        /// The sentence a reader is shown.
        detail: String,
    },
}

/// What the task record says about the issue this work belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueStanding {
    /// No task is active, or the record could not be read.
    Unknown(String),
    /// The task names no issue.
    Undeclared,
    /// The task states `issue: none`: trivial work, declared as such.
    DeclaredNone,
    /// The task names an issue and the plan holds it.
    Resolved {
        /// The issue's id.
        id: String,
        /// Its title, from the record.
        title: String,
    },
    /// The task names an issue the plan does not hold.
    Unresolved {
        /// The id the task named.
        id: String,
    },
}

/// Whether a continuation record exists for this task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoverStanding {
    /// The store could not be read; why.
    Unknown(String),
    /// A handover for this task exists; its file name.
    Present(String),
    /// None does.
    Absent,
}

/// Everything the invariant is answered from. Every field is a judgement somebody else
/// already made; nothing is measured by [`answer`], which composes.
pub struct DoneInputs<'a> {
    /// The obligation closure's word per token.
    pub standing: &'a BTreeMap<String, ObligationStanding>,
    /// Which obligations the change implies and whether the task declared them.
    pub implied: &'a [ImpliedObligation],
    /// The judged gates.
    pub gates: &'a [Gate],
    /// The change set.
    pub changed: &'a [String],
    /// Whether the closure could be read at all.
    pub closure_reachable: bool,
    /// The release analysis.
    pub release: ReleaseStanding,
    /// The issue the task names.
    pub issue: IssueStanding,
    /// The continuation record.
    pub handover: HandoverStanding,
}

/// Answer the invariant: every question of the policy, in the policy's order.
pub(crate) fn answer(policy: &CompletionPolicy, inputs: &DoneInputs<'_>) -> Vec<DoneQuestion> {
    policy
        .questions
        .iter()
        .map(|decl| {
            let (status, evidence, source) = match &decl.kind() {
                QuestionSource::Obligation(token) => obligation(token, inputs),
                QuestionSource::Gates => gates(inputs.gates),
                QuestionSource::Gate(gate) => match inputs.gates.iter().find(|g| &g.id == gate) {
                    Some(g) => (
                        g.status,
                        g.reason.clone(),
                        format!("gate {gate} (.ai/repo/ci/gates.yaml)"),
                    ),
                    // a gate the model does not declare is not a gate that never reported:
                    // the repository asks no such question, and the answer says so by name
                    // rather than leaving a reader to guess between "not asked" and "silent"
                    None => (
                        GateStatus::Exempt,
                        format!(
                            "not applicable: this repository's CI model declares no gate '{gate}'"
                        ),
                        format!("gate {gate}"),
                    ),
                },
                QuestionSource::ChangeTestPath => change(inputs.changed),
                QuestionSource::ReleaseImpact => match &inputs.release {
                    ReleaseStanding::Unknown(why) => (
                        GateStatus::Unknown,
                        format!("the release analysis could not be made: {why}"),
                        "release.analysis".to_string(),
                    ),
                    ReleaseStanding::NotApplicable(why) => (
                        GateStatus::Exempt,
                        why.clone(),
                        "release.analysis".to_string(),
                    ),
                    ReleaseStanding::Measured { ok: true, detail } => (
                        GateStatus::Pass,
                        detail.clone(),
                        "release.analysis (ADR 0051)".into(),
                    ),
                    ReleaseStanding::Measured { ok: false, detail } => (
                        GateStatus::Fail,
                        detail.clone(),
                        "release.analysis (ADR 0051)".into(),
                    ),
                },
                QuestionSource::TaskIssue => match &inputs.issue {
                    IssueStanding::Unknown(why) => {
                        (GateStatus::Unknown, why.clone(), "the task record".into())
                    }
                    IssueStanding::Undeclared => (
                        GateStatus::Queued,
                        "the task names no issue; name the one it serves, or `none` for work the \
                         plan does not track"
                            .into(),
                        "the task record (issue)".into(),
                    ),
                    IssueStanding::DeclaredNone => (
                        GateStatus::Exempt,
                        "the task states `issue: none`: work the plan does not track, declared as \
                         such"
                            .into(),
                        "the task record (issue)".into(),
                    ),
                    IssueStanding::Resolved { id, title } => (
                        GateStatus::Pass,
                        format!("{id} — {title}"),
                        "the task record (issue), resolved against the plan".into(),
                    ),
                    IssueStanding::Unresolved { id } => (
                        GateStatus::Fail,
                        format!("the task names issue {id}, which the plan does not hold"),
                        "the task record (issue), resolved against the plan".into(),
                    ),
                },
                QuestionSource::SessionHandover => match &inputs.handover {
                    HandoverStanding::Unknown(why) => (
                        GateStatus::Unknown,
                        why.clone(),
                        "the continuity store".into(),
                    ),
                    HandoverStanding::Present(name) => (
                        GateStatus::Pass,
                        format!("handover {name}"),
                        ".ai/local/state/handovers/".into(),
                    ),
                    HandoverStanding::Absent => (
                        GateStatus::Queued,
                        "no handover for this task; `finish` also accepts a completion note".into(),
                        ".ai/local/state/handovers/".into(),
                    ),
                },
                QuestionSource::Elsewhere(command) => (
                    GateStatus::Unknown,
                    format!(
                        "nothing in this report reaches that fact; `{command}` is what answers it"
                    ),
                    command.clone(),
                ),
            };
            DoneQuestion {
                id: decl.id.clone(),
                stage: decl.stage.clone(),
                question: decl.question.clone(),
                status,
                evidence,
                source,
                remediation: decl.remediation.clone(),
            }
        })
        .collect()
}

fn obligation(token: &str, inputs: &DoneInputs<'_>) -> (GateStatus, String, String) {
    let applicable = inputs
        .implied
        .iter()
        .find(|o| o.id == token)
        .map(|o| (o.applicable, o.declared, o.reason.clone()));
    match (inputs.standing.get(token), applicable) {
        // neither the task nor the change asks for it: not applicable, whatever the closure
        // may have read for a token it judged on the change's behalf
        (_, Some((false, false, why))) => (
            GateStatus::Exempt,
            why,
            format!("obligation {token} (derived from its own inputs)"),
        ),
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
        // the change implies it and the task never promised it: owed by the change and
        // held by nothing, which is a debt and not a pass
        (None, Some((true, false, why))) => (
            GateStatus::Queued,
            format!(
                "the change implies this obligation and the task does not declare it, so \
                 nothing holds it to it: {why}"
            ),
            format!("obligation {token} (derived from its own inputs)"),
        ),
        (None, Some((false, _, why))) => (
            GateStatus::Exempt,
            why,
            format!("obligation {token} (derived from its own inputs)"),
        ),
        _ if !inputs.closure_reachable => (
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

fn gates(gates: &[Gate]) -> (GateStatus, String, String) {
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
            "{} gate(s) selected: {passing} passing, {refused} refusing, {silent} never reported",
            required.len()
        ),
        ".ai/repo/ci/gates.yaml".to_string(),
    )
}

fn change(changed: &[String]) -> (GateStatus, String, String) {
    let tests: Vec<&String> = changed.iter().filter(|p| is_test_path(p)).take(3).collect();
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
            "the change set (profile verification.regression_test_required)".to_string(),
        )
    } else {
        (
            GateStatus::Pass,
            format!(
                "{} test path(s) touched, including {}",
                tests.len(),
                tests[0]
            ),
            "the change set (profile verification.regression_test_required)".to_string(),
        )
    }
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

    fn policy() -> CompletionPolicy {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        CompletionPolicy::load(&root).unwrap()
    }

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

    struct Fx {
        standing: BTreeMap<String, ObligationStanding>,
        implied: Vec<ImpliedObligation>,
        gates: Vec<Gate>,
        changed: Vec<String>,
        release: ReleaseStanding,
        issue: IssueStanding,
        handover: HandoverStanding,
        reachable: bool,
    }

    impl Fx {
        fn new() -> Self {
            Fx {
                standing: BTreeMap::new(),
                implied: vec![],
                gates: vec![],
                changed: vec![],
                release: ReleaseStanding::Unknown("not asked".into()),
                issue: IssueStanding::Unknown("no task".into()),
                handover: HandoverStanding::Unknown("no store".into()),
                reachable: true,
            }
        }
        fn answer(&self) -> Vec<DoneQuestion> {
            answer(
                &policy(),
                &DoneInputs {
                    standing: &self.standing,
                    implied: &self.implied,
                    gates: &self.gates,
                    changed: &self.changed,
                    closure_reachable: self.reachable,
                    release: self.release.clone(),
                    issue: self.issue.clone(),
                    handover: self.handover.clone(),
                },
            )
        }
        fn by(&self, id: &str) -> DoneQuestion {
            self.answer().into_iter().find(|q| q.id == id).unwrap()
        }
    }

    #[test]
    fn every_question_carries_a_source_a_stage_and_something_to_run() {
        let fx = Fx::new();
        let q = fx.answer();
        assert_eq!(
            q.len(),
            policy().questions.len(),
            "one answer per declared question"
        );
        for question in &q {
            assert!(!question.source.is_empty(), "{} has no source", question.id);
            assert!(!question.remediation.is_empty(), "{}", question.id);
            assert!(!question.evidence.is_empty(), "{}", question.id);
            assert!(!question.stage.is_empty(), "{}", question.id);
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
        let mut fx = Fx::new();
        fx.standing
            .insert("tests".to_string(), standing("discharged"));
        fx.standing.insert("docs".to_string(), standing("stale"));
        fx.standing.insert("push".to_string(), standing("owed"));
        assert_eq!(fx.by("tested").status, GateStatus::Pass);
        assert_eq!(fx.by("documented").status, GateStatus::Stale);
        assert_eq!(fx.by("pushed").status, GateStatus::Queued);
        assert!(fx
            .by("tested")
            .evidence
            .contains("the closure said discharged"));
        assert!(fx.by("tested").source.contains("obligations.closure"));
    }

    #[test]
    fn an_obligation_the_change_implies_and_nobody_declared_is_a_debt_not_a_pass() {
        let mut fx = Fx::new();
        fx.implied = vec![
            implied_of("tests", true, false),
            implied_of("deploy", false, false),
        ];
        assert_eq!(fx.by("tested").status, GateStatus::Queued);
        assert!(fx.by("tested").evidence.contains("does not declare it"));
        assert_eq!(
            fx.by("deployed").status,
            GateStatus::Exempt,
            "a task that deploys nothing is not owed a deployment"
        );
    }

    #[test]
    fn the_ci_question_separates_refused_from_never_reported() {
        let mut fx = Fx::new();
        assert_eq!(fx.by("ci").status, GateStatus::Exempt);

        fx.gates = vec![
            gate("a", GateStatus::Queued, true),
            gate("b", GateStatus::Pass, true),
        ];
        assert_eq!(fx.by("ci").status, GateStatus::Queued);

        fx.gates = vec![
            gate("a", GateStatus::Fail, true),
            gate("b", GateStatus::Queued, true),
        ];
        let ci = fx.by("ci");
        assert_eq!(ci.status, GateStatus::Fail, "a refusal outranks a silence");
        assert!(ci.evidence.contains("1 refusing") && ci.evidence.contains("1 never reported"));

        fx.gates = vec![
            gate("a", GateStatus::Pass, true),
            gate("b", GateStatus::Exempt, false),
        ];
        assert_eq!(fx.by("ci").status, GateStatus::Pass);
    }

    #[test]
    fn a_gate_question_a_model_declares_no_gate_for_is_exempt_and_says_why() {
        let mut fx = Fx::new();
        fx.gates = vec![gate("projection-closure", GateStatus::Pass, true)];
        assert_eq!(fx.by("parity").status, GateStatus::Pass);
        let missing = fx.by("changelog");
        assert_eq!(missing.status, GateStatus::Exempt);
        assert!(missing.evidence.contains("declares no gate"));
        assert!(missing.evidence.starts_with("not applicable"));
    }

    #[test]
    fn a_repository_with_no_release_owes_no_version_and_says_so() {
        let mut fx = Fx::new();
        fx.release = ReleaseStanding::NotApplicable("no release record".into());
        let v = fx.by("version");
        assert_eq!(v.status, GateStatus::Exempt);
        assert!(v.evidence.contains("no release record"));
    }

    #[test]
    fn the_version_question_is_the_release_analysis_and_refuses_an_understated_version() {
        let mut fx = Fx::new();
        assert_eq!(fx.by("version").status, GateStatus::Unknown);
        fx.release = ReleaseStanding::Measured {
            ok: false,
            detail: "0.6.0 owed, 0.5.0 declared".into(),
        };
        let v = fx.by("version");
        assert_eq!(v.status, GateStatus::Fail);
        assert!(v.evidence.contains("owed"));
        fx.release = ReleaseStanding::Measured {
            ok: true,
            detail: "minor, 0.6.0".into(),
        };
        assert_eq!(fx.by("version").status, GateStatus::Pass);
    }

    #[test]
    fn the_issue_question_reads_the_task_record_and_the_plan() {
        let mut fx = Fx::new();
        fx.issue = IssueStanding::Undeclared;
        assert_eq!(fx.by("issue").status, GateStatus::Queued);
        fx.issue = IssueStanding::DeclaredNone;
        assert_eq!(fx.by("issue").status, GateStatus::Exempt);
        fx.issue = IssueStanding::Resolved {
            id: "I0001".into(),
            title: "t".into(),
        };
        assert_eq!(fx.by("issue").status, GateStatus::Pass);
        fx.issue = IssueStanding::Unresolved { id: "I9999".into() };
        assert_eq!(
            fx.by("issue").status,
            GateStatus::Fail,
            "a dangling reference refuses"
        );
    }

    #[test]
    fn the_handover_question_reads_the_store() {
        let mut fx = Fx::new();
        fx.handover = HandoverStanding::Absent;
        assert_eq!(fx.by("handover").status, GateStatus::Queued);
        fx.handover = HandoverStanding::Present("x.md".into());
        assert_eq!(fx.by("handover").status, GateStatus::Pass);
    }

    #[test]
    fn regression_is_answered_from_the_change_set_alone() {
        let mut fx = Fx::new();
        fx.changed = vec!["lib/a.sh".to_string()];
        let r = fx.by("regression-tested");
        assert_eq!(r.status, GateStatus::Queued);
        assert!(r.evidence.contains("no test path"));
        fx.changed
            .push("test/cases/131_completion_gates.sh".to_string());
        assert_eq!(fx.by("regression-tested").status, GateStatus::Pass);
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
        let text = "version: 1\nstages:\n  - id: a\n    title: A\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: x\n    source: elsewhere:majordomus doctor\n    remediation: r\n";
        let p = CompletionPolicy::parse(text, "t").unwrap();
        let fx = Fx::new();
        let q = answer(
            &p,
            &DoneInputs {
                standing: &fx.standing,
                implied: &fx.implied,
                gates: &fx.gates,
                changed: &fx.changed,
                closure_reachable: true,
                release: fx.release.clone(),
                issue: fx.issue.clone(),
                handover: fx.handover.clone(),
            },
        );
        assert_eq!(q[0].status, GateStatus::Unknown);
        assert!(q[0].evidence.contains("majordomus doctor"));
    }
}
