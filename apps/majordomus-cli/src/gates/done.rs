//! The "done" invariant: the questions a task must answer before anybody may call it
//! finished, each answered from the one place that already knows.
//!
//! # Why this is a projection and not a checklist
//!
//! Twenty-two questions get asked of every piece of work in this repository — is it tested,
//! is it documented, are the projections current, did the version move, is it pushed, is it
//! on the trunk, is it published, is CI green, was the deployment verified, is a stale
//! branch left behind, did the recorded debt grow, is the repository bigger than the work
//! found it. Written as a checklist they would be twenty-two new opinions, each able to
//! disagree with the subsystem that already holds the fact, and a checklist is exactly the
//! artifact a worker learns to tick.
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
//! - the **closing readings** of [`super::closing`], for the three questions a person asks
//!   when a session ends: the debt baselines at the commit the task started at and now, the
//!   branches and worktrees created since it started, and the pull requests opened since —
//!   each read from the record that holds it, never from what a worker recalls doing;
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

use super::closing::{Closing, DEBT_DIRECTORY, DEBT_SUFFIX};
use super::judge::{Gate, GateStatus};
use super::ImpliedObligation;
use crate::convergence::ConvergenceReport;

/// ```
/// use majordomus_cli::gates::{DoneQuestion, GateStatus};
///
/// let q = DoneQuestion {
///     id: "pushed".into(),
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
    /// The repository's convergence: whether any work is held where it can be lost.
    Convergence,
    /// The debt baselines, at the commit the task started at and now.
    Debt,
    /// The branches and worktrees created since the task started that remain.
    Accumulated,
    /// The pull requests opened since the task started that are still open.
    Backlog,
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
            Answers::Convergence,
            "majordomus convergence",
        ),
        (
            "no-new-debt",
            "Is the repository's recorded debt no larger than when this task started?",
            Answers::Debt,
            "take out what the change added to the baseline, or settle what the entry records",
        ),
        (
            "nothing-accumulated",
            "Has every branch and worktree created since this task started gone again?",
            Answers::Accumulated,
            "majordomus worktree cleanup",
        ),
        (
            "backlog-not-grown",
            "Has every pull request opened since this task started been closed?",
            Answers::Backlog,
            "majordomus prs status",
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

/// Answer the invariant.
///
/// Every argument is a judgement somebody else already made: `standing` is the obligation
/// closure's, `gates` is [`super::judge`]'s, `implied` is [`super::implied`]'s,
/// `changed` is the change set, `convergence` is [`crate::convergence`]'s verdict over the
/// repository's holdings and `closing` is [`super::closing`]'s readings of what the task
/// leaves behind. Nothing is measured here — this composes.
pub(crate) fn answer(
    standing: &std::collections::BTreeMap<String, ObligationStanding>,
    implied: &[ImpliedObligation],
    gates: &[Gate],
    changed: &[String],
    closure_reachable: bool,
    convergence: Option<&ConvergenceReport>,
    closing: Option<&Closing>,
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
                Answers::Convergence => match convergence {
                    // the verdict is one word over every holding; the evidence names the
                    // holdings themselves, because "3 at risk" is not something a person
                    // can act on and `feature/x, /tmp/wt` is
                    Some(report) => {
                        let at_risk: Vec<&str> = report
                            .holdings
                            .iter()
                            .filter(|h| h.at_risk)
                            .map(|h| h.identity.as_str())
                            .take(3)
                            .collect();
                        if report.converged {
                            (
                                GateStatus::Pass,
                                report.summary(),
                                "convergence.report".to_string(),
                            )
                        } else {
                            (
                                GateStatus::Fail,
                                format!("{}: {}", report.summary(), at_risk.join(", ")),
                                "convergence.report".to_string(),
                            )
                        }
                    }
                    None => (
                        GateStatus::Unknown,
                        "the repository's holdings could not be read in this checkout".into(),
                        "convergence.report".to_string(),
                    ),
                },
                Answers::Debt => debt_answer(closing),
                Answers::Accumulated => accumulated_answer(closing),
                Answers::Backlog => backlog_answer(closing),
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

/// The words for a closing question nothing could be measured for: there is no task, so
/// there is no start to measure from.
const NO_START: &str =
    "no task record says where this work started, so there is nothing to measure it from";

/// Did a debt baseline gain an entry since the task started?
///
/// Growth refuses: an entry added to a baseline is debt the change created and then
/// permitted itself. A baseline that appears for the first time is not growth — the debt it
/// lists was there unrecorded — and is named in the evidence so that it is not mistaken for
/// nothing having happened.
fn debt_answer(closing: Option<&Closing>) -> (GateStatus, String, String) {
    let source = format!(
        "the debt baselines ({DEBT_DIRECTORY}/*{DEBT_SUFFIX}) at the task's first commit and now"
    );
    let (status, evidence) = match closing.map(|c| &c.debt) {
        None => (GateStatus::Unknown, NO_START.to_string()),
        Some(Err(why)) => (
            GateStatus::Unknown,
            format!("the debt baselines could not be compared: {why}"),
        ),
        Some(Ok(baselines)) if baselines.is_empty() => (
            GateStatus::Exempt,
            "this repository keeps no debt baseline".to_string(),
        ),
        Some(Ok(baselines)) => {
            let grown: Vec<String> = baselines
                .iter()
                .filter(|b| b.grown_by() > 0)
                .map(|b| {
                    format!(
                        "{} {} -> {} (+{})",
                        b.path,
                        b.before.unwrap_or(0),
                        b.after,
                        b.grown_by()
                    )
                })
                .collect();
            let first: Vec<&str> = baselines
                .iter()
                .filter(|b| b.before.is_none())
                .map(|b| b.path.as_str())
                .collect();
            let recorded = if first.is_empty() {
                String::new()
            } else {
                format!("; recorded for the first time: {}", first.join(", "))
            };
            if grown.is_empty() {
                let before: usize = baselines.iter().filter_map(|b| b.before).sum();
                let after: usize = baselines
                    .iter()
                    .filter(|b| b.before.is_some())
                    .map(|b| b.after)
                    .sum();
                (
                    GateStatus::Pass,
                    format!(
                        "{} baseline(s) held {before} entries when the task started and hold {after} now{recorded}",
                        baselines.len() - first.len()
                    ),
                )
            } else {
                (
                    GateStatus::Fail,
                    format!(
                        "{} baseline(s) grew: {}{recorded}",
                        grown.len(),
                        grown.join("; ")
                    ),
                )
            }
        }
    };
    (status, evidence, source)
}

/// Is a branch or a worktree created since the task started still there?
///
/// One that is makes the question owed, not refused: the task's own branch is among them
/// until the work has landed and been cleaned up, which is exactly the state the question
/// exists to keep visible.
fn accumulated_answer(closing: Option<&Closing>) -> (GateStatus, String, String) {
    let source = "git's reflogs: when each local branch and each linked worktree began".to_string();
    let (status, evidence) = match closing.map(|c| (&c.accumulated, c.since.as_str())) {
        None => (GateStatus::Unknown, NO_START.to_string()),
        Some((Err(why), _)) => (
            GateStatus::Unknown,
            format!("the branches and worktrees could not be read: {why}"),
        ),
        Some((Ok(found), since)) => {
            let totals = format!(
                "{} linked worktree(s) and {} local branch(es) now",
                found.worktrees_now, found.branches_now
            );
            let undated = if found.undated == 0 {
                String::new()
            } else {
                format!(
                    "; {} have no creation record and are counted on neither side",
                    found.undated
                )
            };
            if found.worktrees.is_empty() && found.branches.is_empty() {
                (
                    GateStatus::Pass,
                    format!("nothing created since {since} remains; {totals}{undated}"),
                )
            } else {
                let named: Vec<&str> = found
                    .worktrees
                    .iter()
                    .chain(found.branches.iter())
                    .map(String::as_str)
                    .take(3)
                    .collect();
                (
                    GateStatus::Queued,
                    format!(
                        "{} worktree(s) and {} branch(es) created since {since} are still here, \
                         among them {}; {totals}{undated}",
                        found.worktrees.len(),
                        found.branches.len(),
                        named.join(", ")
                    ),
                )
            }
        }
    };
    (status, evidence, source)
}

/// Is a pull request opened since the task started still open?
fn backlog_answer(closing: Option<&Closing>) -> (GateStatus, String, String) {
    let source = "the recorded forge observation (majordomus prs refresh)".to_string();
    let (status, evidence) = match closing.map(|c| (&c.backlog, c.since.as_str())) {
        None => (GateStatus::Unknown, NO_START.to_string()),
        Some((Err(why), _)) => (
            GateStatus::Unknown,
            format!("the backlog could not be read: {why}"),
        ),
        Some((Ok(backlog), since)) => {
            let observed = format!(
                "{} open when the forge was observed at {}",
                backlog.open, backlog.observed_at
            );
            if backlog.opened_since.is_empty() {
                (
                    GateStatus::Pass,
                    format!("no pull request opened since {since} is open; {observed}"),
                )
            } else {
                let named: Vec<String> = backlog
                    .opened_since
                    .iter()
                    .take(5)
                    .map(|n| format!("#{n}"))
                    .collect();
                (
                    GateStatus::Queued,
                    format!(
                        "{} pull request(s) opened since {since} are still open, among them {}; {observed}",
                        backlog.opened_since.len(),
                        named.join(" ")
                    ),
                )
            }
        }
    };
    (status, evidence, source)
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

    use crate::gates::closing::{Accumulated, Backlog, DebtReading};

    /// Closing readings in which nothing was added: one baseline, unchanged.
    fn settled() -> Closing {
        Closing {
            since: "2026-10-10T12:00:00Z".into(),
            debt: Ok(vec![DebtReading {
                path: ".ai/repo/doc-command-baseline.txt".into(),
                before: Some(4),
                after: 3,
            }]),
            accumulated: Ok(Accumulated {
                worktrees_now: 2,
                branches_now: 5,
                ..Accumulated::default()
            }),
            backlog: Ok(Backlog {
                observed_at: "2026-10-10T15:00:00Z".into(),
                open: 7,
                opened_since: Default::default(),
            }),
        }
    }

    fn closing_answers(closing: Option<&Closing>) -> [DoneQuestion; 3] {
        let q = answer(&BTreeMap::new(), &[], &[], &[], true, None, closing);
        let by = |id: &str| q.iter().find(|q| q.id == id).unwrap().clone();
        [
            by("no-new-debt"),
            by("nothing-accumulated"),
            by("backlog-not-grown"),
        ]
    }

    #[test]
    fn a_task_that_added_nothing_passes_the_three_closing_questions_with_what_was_read() {
        let [debt, accumulated, backlog] = closing_answers(Some(&settled()));
        assert_eq!(debt.status, GateStatus::Pass);
        assert!(
            debt.evidence
                .contains("held 4 entries when the task started and hold 3 now"),
            "{}",
            debt.evidence
        );
        assert_eq!(accumulated.status, GateStatus::Pass);
        assert!(accumulated
            .evidence
            .contains("nothing created since 2026-10-10T12:00:00Z remains"));
        assert!(accumulated
            .evidence
            .contains("2 linked worktree(s) and 5 local branch(es) now"));
        assert_eq!(backlog.status, GateStatus::Pass);
        assert!(backlog
            .evidence
            .contains("7 open when the forge was observed at 2026-10-10T15:00:00Z"));
    }

    #[test]
    fn a_baseline_that_gained_an_entry_refuses_and_names_the_file_and_the_numbers() {
        let mut closing = settled();
        closing.debt = Ok(vec![
            DebtReading {
                path: ".ai/repo/a-baseline.txt".into(),
                before: Some(2),
                after: 5,
            },
            DebtReading {
                path: ".ai/repo/b-baseline.txt".into(),
                before: Some(9),
                after: 1,
            },
            DebtReading {
                path: ".ai/repo/new-baseline.txt".into(),
                before: None,
                after: 6,
            },
        ]);
        let [debt, ..] = closing_answers(Some(&closing));
        assert_eq!(
            debt.status,
            GateStatus::Fail,
            "shrinking one baseline does not pay for growing another"
        );
        assert!(
            debt.evidence
                .contains("1 baseline(s) grew: .ai/repo/a-baseline.txt 2 -> 5 (+3)"),
            "{}",
            debt.evidence
        );
        assert!(
            debt.evidence
                .contains("recorded for the first time: .ai/repo/new-baseline.txt"),
            "{}",
            debt.evidence
        );

        // a baseline that only appears is said, and is not growth
        closing.debt = Ok(vec![DebtReading {
            path: ".ai/repo/new-baseline.txt".into(),
            before: None,
            after: 6,
        }]);
        let [debt, ..] = closing_answers(Some(&closing));
        assert_eq!(debt.status, GateStatus::Pass);
        assert!(
            debt.evidence.contains("0 baseline(s) held 0 entries"),
            "{}",
            debt.evidence
        );
        assert!(
            debt.evidence.contains("recorded for the first time"),
            "{}",
            debt.evidence
        );

        // and a repository that keeps none is exempt rather than clean
        closing.debt = Ok(vec![]);
        assert_eq!(
            closing_answers(Some(&closing))[0].status,
            GateStatus::Exempt
        );
    }

    #[test]
    fn what_was_created_since_the_start_and_remains_is_owed_and_named() {
        let mut closing = settled();
        closing.accumulated = Ok(Accumulated {
            worktrees: ["/w/feature/a".to_string()].into(),
            branches: ["feature/a", "feature/b", "feature/c"]
                .map(String::from)
                .into(),
            worktrees_now: 3,
            branches_now: 8,
            undated: 2,
        });
        let [_, accumulated, _] = closing_answers(Some(&closing));
        assert_eq!(
            accumulated.status,
            GateStatus::Queued,
            "owed, not refused: the task's own branch is among them"
        );
        let said = &accumulated.evidence;
        assert!(
            said.contains(
                "1 worktree(s) and 3 branch(es) created since 2026-10-10T12:00:00Z are still here"
            ),
            "{said}"
        );
        assert!(
            said.contains("among them /w/feature/a, feature/a, feature/b;"),
            "three are named: {said}"
        );
        assert!(
            said.contains("2 have no creation record and are counted on neither side"),
            "{said}"
        );
    }

    #[test]
    fn a_pull_request_opened_since_the_start_and_still_open_is_owed_and_numbered() {
        let mut closing = settled();
        closing.backlog = Ok(Backlog {
            observed_at: "2026-10-10T15:00:00Z".into(),
            open: 52,
            opened_since: [897, 898, 899].into(),
        });
        let [.., backlog] = closing_answers(Some(&closing));
        assert_eq!(backlog.status, GateStatus::Queued);
        assert!(backlog.evidence.contains("3 pull request(s) opened since 2026-10-10T12:00:00Z are still open, among them #897 #898 #899; 52 open"), "{}", backlog.evidence);
    }

    #[test]
    fn a_closing_reading_that_was_not_taken_is_unknown_and_says_why() {
        // no task: nothing to measure from
        for q in closing_answers(None) {
            assert_eq!(q.status, GateStatus::Unknown, "{}", q.id);
            assert!(
                q.evidence
                    .contains("no task record says where this work started"),
                "{}",
                q.evidence
            );
        }
        // a task, and three readings that failed: each says its own reason
        let unread = Closing {
            since: "2026-10-10T12:00:00Z".into(),
            debt: Err("the commit the task started at (abc) is not in this repository".into()),
            accumulated: Err("git could not list the local branches".into()),
            backlog: Err("the forge has never been observed from this checkout".into()),
        };
        let [debt, accumulated, backlog] = closing_answers(Some(&unread));
        for q in [&debt, &accumulated, &backlog] {
            assert_eq!(
                q.status,
                GateStatus::Unknown,
                "{}: an unread reading is never a pass",
                q.id
            );
        }
        assert!(debt.evidence.contains("is not in this repository"));
        assert!(accumulated
            .evidence
            .contains("could not list the local branches"));
        assert!(backlog.evidence.contains("never been observed"));
    }

    #[test]
    fn every_question_carries_a_source_and_something_to_run() {
        let q = answer(&BTreeMap::new(), &[], &[], &[], false, None, None);
        assert_eq!(q.len(), 22, "the invariant is twenty-two questions");
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
        let q = answer(&s, &[], &[], &[], true, None, None);
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
        let q = answer(&BTreeMap::new(), &implied, &[], &[], true, None, None);
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
        let none = answer(&BTreeMap::new(), &[], &[], &[], true, None, None);
        assert_eq!(
            none.iter().find(|q| q.id == "ci").unwrap().status,
            GateStatus::Exempt
        );

        let silent = vec![
            gate("a", GateStatus::Queued, true),
            gate("b", GateStatus::Pass, true),
        ];
        let q = answer(&BTreeMap::new(), &[], &silent, &[], true, None, None);
        assert_eq!(
            q.iter().find(|q| q.id == "ci").unwrap().status,
            GateStatus::Queued
        );

        let red = vec![
            gate("a", GateStatus::Fail, true),
            gate("b", GateStatus::Queued, true),
        ];
        let q = answer(&BTreeMap::new(), &[], &red, &[], true, None, None);
        let ci = q.iter().find(|q| q.id == "ci").unwrap();
        assert_eq!(ci.status, GateStatus::Fail, "a refusal outranks a silence");
        assert!(ci.evidence.contains("1 refusing") && ci.evidence.contains("1 never reported"));

        let green = vec![
            gate("a", GateStatus::Pass, true),
            gate("b", GateStatus::Exempt, false),
        ];
        let q = answer(&BTreeMap::new(), &[], &green, &[], true, None, None);
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
            None,
            None,
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
        let q = answer(
            &BTreeMap::new(),
            &[],
            &[],
            &["lib/a.sh".to_string()],
            true,
            None,
            None,
        );
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
            None,
            None,
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

    /// The verdict is the answer: a converged repository passes, and an unconverged one
    /// fails with the holdings named. Before this, the question answered "nothing in this
    /// report reaches that fact" no matter what the repository held.
    #[test]
    fn no_stale_topology_is_answered_from_the_convergence_verdict() {
        use crate::convergence::{ConvergenceReport, Disposition, Holding, HoldingKind};

        let at_risk = ConvergenceReport {
            schema: "convergence/v1".into(),
            repository: "/tmp/r".into(),
            trunk: Some("master".into()),
            holdings: vec![Holding {
                kind: HoldingKind::Branch,
                identity: "feature/never-pushed".into(),
                disposition: Disposition::LocalOnly,
                evidence: "abc on no remote".into(),
                remedy: "git push -u origin feature/never-pushed".into(),
                at_risk: true,
            }],
            tallies: Default::default(),
            at_risk: 1,
            converged: false,
        };
        let q = answer(&BTreeMap::new(), &[], &[], &[], true, Some(&at_risk), None);
        let question = q.iter().find(|q| q.id == "no-stale-topology").unwrap();
        assert_eq!(question.status, GateStatus::Fail);
        assert!(
            question.evidence.contains("feature/never-pushed"),
            "the finding must name the holding, not only count it: {}",
            question.evidence
        );
        assert_eq!(question.source, "convergence.report");

        let converged = ConvergenceReport {
            holdings: vec![],
            at_risk: 0,
            converged: true,
            ..at_risk
        };
        let q = answer(
            &BTreeMap::new(),
            &[],
            &[],
            &[],
            true,
            Some(&converged),
            None,
        );
        assert_eq!(
            q.iter()
                .find(|q| q.id == "no-stale-topology")
                .unwrap()
                .status,
            GateStatus::Pass
        );
    }

    /// A verdict that could not be read is not a clean verdict: the question is unknown,
    /// never a pass. The rule about absence, applied to the one source that can be absent.
    #[test]
    fn an_unreadable_verdict_is_unknown_and_never_a_pass() {
        let q = answer(&BTreeMap::new(), &[], &[], &[], true, None, None);
        let question = q.iter().find(|q| q.id == "no-stale-topology").unwrap();
        assert_eq!(question.status, GateStatus::Unknown);
        assert!(!question.status.refuses());
        assert!(question.status.unverified());
    }

    #[test]
    fn a_question_nothing_here_reaches_says_so_and_names_the_command() {
        let q = answer(&BTreeMap::new(), &[], &[], &[], true, None, None);
        for id in ["parity", "handover", "issue"] {
            let question = q.iter().find(|q| q.id == id).unwrap();
            assert_eq!(question.status, GateStatus::Unknown, "{id}");
            assert!(
                question.evidence.contains("majordomus"),
                "{id} names no command"
            );
        }
    }
}
