//! The reasoning state of a session, derived from its records and nothing else: what is
//! uncertain, what was asked and what came back, where advice disagreed and what settled
//! it, what was concluded and whether it was proven — plus the timeline, the degradation
//! that happened along the way, and the report a handover or a final summary carries.
//!
//! Deterministic: the records are read in `(recorded_at, id)` order, every list here keeps
//! that order, and the report is rendered from these lists alone. The same records give
//! the same state on every surface, which is what lets the CLI, HTTP, MCP, the Cockpit,
//! `majordomus context` and a handover all say the same thing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::policy::{Materiality, PlanOutcome};
use super::record::*;
use super::store::{assessment_of, resolution_of, standing_conclusion, Loaded};

/// Where an assessment stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStanding {
    /// Stated; no plan yet.
    Open,
    /// Planned; no conclusion yet.
    Planned,
    /// A disagreement on it is unsettled.
    Disputed,
    /// Concluded; not yet validated.
    Concluded,
    /// Concluded, and every validation recorded passed.
    Validated,
    /// Concluded, and a validation failed.
    ValidationFailed,
}

/// One assessment and where it stands.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentView {
    /// Record id.
    pub id: String,
    /// The task it was recorded under.
    pub task: String,
    /// What is uncertain.
    pub subject: String,
    /// How much it matters.
    pub materiality: Materiality,
    /// Where it stands.
    pub standing: AssessmentStanding,
    /// Its standing conclusion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// Questions it left open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<String>,
}

/// One consultation, as the state reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsultationView {
    /// Record id.
    pub id: String,
    /// The plan that selected it.
    pub plan: String,
    /// The advisor.
    pub advisor: String,
    /// How it ended.
    pub status: ConsultationStatus,
    /// Where the advice stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<Stance>,
    /// The advice, summarised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// The adapter's diagnostic, for a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

/// One conclusion, as the state reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConclusionView {
    /// Record id.
    pub id: String,
    /// The assessment it concludes.
    pub assessment: String,
    /// The decision.
    pub decision: String,
    /// Advisors whose answers it weighed.
    pub reviewed_by: Vec<String>,
    /// How many.
    pub independent_review_count: usize,
    /// Validations recorded: passed, failed.
    pub validations_passed: usize,
    /// Validations failed.
    pub validations_failed: usize,
    /// Superseded by a later conclusion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// Its validation plan.
    pub validation_plan: Vec<String>,
    /// Risks that remain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
}

/// One disagreement, as the state reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DisagreementView {
    /// Record id.
    pub id: String,
    /// The assessment.
    pub assessment: String,
    /// The question that decides it.
    pub discriminating_question: String,
    /// The resolution, when settled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// The position the evidence favoured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favours: Option<String>,
}

/// A phase of the reasoning timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningPhase {
    /// Uncertainty stated with evidence.
    Uncertainty,
    /// Review planned.
    Consulting,
    /// Decided locally, by plan.
    Local,
    /// A standing conclusion reused.
    Reuse,
    /// An advisor answered.
    AdvisorResponse,
    /// An advisor could not answer.
    Degradation,
    /// Positions diverged.
    Disagreement,
    /// An experiment settled a disagreement.
    Experiment,
    /// A conclusion.
    Conclusion,
    /// A validation step.
    Validation,
    /// A failed attempt.
    Attempt,
}

/// One step of the timeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningStep {
    /// When.
    pub at: String,
    /// The record.
    pub id: String,
    /// The phase.
    pub phase: ReasoningPhase,
    /// One line.
    pub summary: String,
}

/// Totals over the records.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningTotals {
    /// Records.
    pub records: usize,
    /// Consultations that completed.
    pub consultations_completed: usize,
    /// Consultations that did not.
    pub consultations_failed: usize,
    /// Distinct advisors that answered.
    pub independent_reviewers: usize,
    /// Input tokens the adapters reported.
    pub input_tokens: u64,
    /// Output tokens the adapters reported.
    pub output_tokens: u64,
}

/// The reasoning state of a task (or of every task).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningState {
    /// The task reported, or `all`.
    pub task: String,
    /// The assessments.
    pub assessments: Vec<AssessmentView>,
    /// The consultations.
    pub consultations: Vec<ConsultationView>,
    /// The disagreements.
    pub disagreements: Vec<DisagreementView>,
    /// The conclusions.
    pub conclusions: Vec<ConclusionView>,
    /// The timeline.
    pub timeline: Vec<ReasoningStep>,
    /// Totals.
    pub totals: ReasoningTotals,
    /// Store files that could not be read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
}

fn one_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or_default().trim();
    if line.chars().count() > 140 {
        format!("{}…", line.chars().take(139).collect::<String>())
    } else {
        line.to_string()
    }
}

/// Derive the state of `task` (`None` for every task).
pub fn derive(loaded: &Loaded, task: Option<&str>) -> ReasoningState {
    let records: Vec<&ReasoningRecord> = loaded
        .records
        .iter()
        .filter(|r| task.is_none_or(|t| r.task == t))
        .collect();
    let mut state = ReasoningState {
        task: task.unwrap_or("all").to_string(),
        assessments: Vec::new(),
        consultations: Vec::new(),
        disagreements: Vec::new(),
        conclusions: Vec::new(),
        timeline: Vec::new(),
        totals: ReasoningTotals {
            records: records.len(),
            ..Default::default()
        },
        unreadable: loaded.unreadable.clone(),
    };
    let mut reviewers: Vec<&str> = Vec::new();
    for r in &records {
        let (phase, summary) = match &r.body {
            ReasoningBody::Assessment(a) => {
                let conclusion = standing_conclusion(loaded, &r.id);
                let disputed = loaded.records.iter().any(|d| {
                    matches!(&d.body, ReasoningBody::Disagreement(x) if x.assessment == r.id)
                        && resolution_of(loaded, &d.id).is_none()
                });
                let planned = loaded
                    .records
                    .iter()
                    .any(|p| matches!(&p.body, ReasoningBody::Plan(x) if x.assessment == r.id));
                let standing = match conclusion {
                    _ if disputed => AssessmentStanding::Disputed,
                    Some(c) => {
                        let (pass, fail) = validations(loaded, &c.id);
                        if fail > 0 {
                            AssessmentStanding::ValidationFailed
                        } else if pass > 0 {
                            AssessmentStanding::Validated
                        } else {
                            AssessmentStanding::Concluded
                        }
                    }
                    None if planned => AssessmentStanding::Planned,
                    None => AssessmentStanding::Open,
                };
                state.assessments.push(AssessmentView {
                    id: r.id.clone(),
                    task: r.task.clone(),
                    subject: a.subject.clone(),
                    materiality: a.materiality,
                    standing,
                    conclusion: conclusion.map(|c| c.id.clone()),
                    questions: a.questions.clone(),
                });
                (
                    ReasoningPhase::Uncertainty,
                    format!(
                        "{} ({}, {} evidence)",
                        one_line(&a.subject),
                        super::policy::materiality_word(a.materiality),
                        a.evidence.len()
                    ),
                )
            }
            ReasoningBody::Plan(p) => {
                let chosen: Vec<&str> =
                    p.plan.selected.iter().map(|s| s.advisor.as_str()).collect();
                match p.plan.outcome {
                    PlanOutcome::Consult => (
                        ReasoningPhase::Consulting,
                        format!(
                            "consult {} ({})",
                            chosen.join(", "),
                            one_line(&p.plan.reason)
                        ),
                    ),
                    PlanOutcome::DecideLocally => (ReasoningPhase::Local, one_line(&p.plan.reason)),
                    PlanOutcome::ReusePrior => (ReasoningPhase::Reuse, one_line(&p.plan.reason)),
                }
            }
            ReasoningBody::Consultation(c) => {
                state.consultations.push(ConsultationView {
                    id: r.id.clone(),
                    plan: c.plan.clone(),
                    advisor: c.advisor.clone(),
                    status: c.status,
                    stance: c.stance,
                    conclusion: c.conclusion.clone(),
                    diagnostic: c.diagnostic.clone(),
                });
                if let Some(u) = c.usage {
                    state.totals.input_tokens += u.input_tokens;
                    state.totals.output_tokens += u.output_tokens;
                }
                if c.status == ConsultationStatus::Completed {
                    state.totals.consultations_completed += 1;
                    if !reviewers.contains(&c.advisor.as_str()) {
                        reviewers.push(&c.advisor);
                    }
                    (
                        ReasoningPhase::AdvisorResponse,
                        format!(
                            "{} {}: {}",
                            c.advisor,
                            c.stance.map(stance_word).unwrap_or("answered"),
                            one_line(c.conclusion.as_deref().unwrap_or_default())
                        ),
                    )
                } else {
                    state.totals.consultations_failed += 1;
                    (
                        ReasoningPhase::Degradation,
                        format!(
                            "{} {}{}",
                            c.advisor,
                            c.status.as_str(),
                            c.diagnostic
                                .as_deref()
                                .map(|d| format!(": {}", one_line(d)))
                                .unwrap_or_default()
                        ),
                    )
                }
            }
            ReasoningBody::Disagreement(d) => {
                let res = resolution_of(loaded, &r.id);
                state.disagreements.push(DisagreementView {
                    id: r.id.clone(),
                    assessment: d.assessment.clone(),
                    discriminating_question: d.discriminating_question.clone(),
                    resolution: res.map(|x| x.id.clone()),
                    favours: res.and_then(|x| match &x.body {
                        ReasoningBody::Resolution(y) => Some(y.favours.clone()),
                        _ => None,
                    }),
                });
                (
                    ReasoningPhase::Disagreement,
                    format!(
                        "{} positions; decided by: {}",
                        d.positions.len(),
                        one_line(&d.discriminating_question)
                    ),
                )
            }
            ReasoningBody::Resolution(x) => (
                ReasoningPhase::Experiment,
                format!("{} → favours {}", one_line(&x.experiment), x.favours),
            ),
            ReasoningBody::Conclusion(c) => {
                let (pass, fail) = validations(loaded, &r.id);
                let superseded_by = loaded.records.iter().find_map(|o| match &o.body {
                    ReasoningBody::Conclusion(x)
                        if x.input.supersedes.as_deref() == Some(&r.id) =>
                    {
                        Some(o.id.clone())
                    }
                    _ => None,
                });
                state.conclusions.push(ConclusionView {
                    id: r.id.clone(),
                    assessment: c.input.assessment.clone(),
                    decision: c.input.decision.clone(),
                    reviewed_by: c.reviewed_by.clone(),
                    independent_review_count: c.independent_review_count,
                    validations_passed: pass,
                    validations_failed: fail,
                    superseded_by,
                    validation_plan: c.input.validation_plan.clone(),
                    risks: c.input.risks.clone(),
                });
                (
                    ReasoningPhase::Conclusion,
                    format!(
                        "{} ({} independent review(s))",
                        one_line(&c.input.decision),
                        c.independent_review_count
                    ),
                )
            }
            ReasoningBody::Validation(v) => (
                ReasoningPhase::Validation,
                format!(
                    "{}: {}",
                    match v.outcome {
                        ValidationOutcome::Pass => "pass",
                        ValidationOutcome::Fail => "fail",
                    },
                    one_line(&v.check)
                ),
            ),
            ReasoningBody::Attempt(a) => (
                ReasoningPhase::Attempt,
                format!(
                    "{} failed: {}",
                    one_line(&a.attempt),
                    one_line(&a.observed_failure)
                ),
            ),
        };
        state.timeline.push(ReasoningStep {
            at: r.recorded_at.clone(),
            id: r.id.clone(),
            phase,
            summary,
        });
    }
    state.totals.independent_reviewers = reviewers.len();
    state
}

fn stance_word(s: Stance) -> &'static str {
    match s {
        Stance::Supports => "supports",
        Stance::Opposes => "opposes",
        Stance::Alternative => "proposes an alternative",
        Stance::Inconclusive => "is inconclusive",
    }
}

fn validations(loaded: &Loaded, conclusion: &str) -> (usize, usize) {
    loaded
        .records
        .iter()
        .fold((0, 0), |(p, f), r| match &r.body {
            ReasoningBody::Validation(v) if v.conclusion == conclusion => match v.outcome {
                ValidationOutcome::Pass => (p + 1, f),
                ValidationOutcome::Fail => (p, f + 1),
            },
            _ => (p, f),
        })
}

/// The state as Markdown: what a handover carries and a final report ends with. Empty
/// when there is nothing to say.
pub fn render_markdown(state: &ReasoningState) -> String {
    if state.totals.records == 0 {
        return String::new();
    }
    let mut out = String::new();
    let t = &state.totals;
    out.push_str(&format!(
        "Reasoning: {} record(s); {} consultation(s) completed, {} failed; {} independent reviewer(s).\n",
        t.records, t.consultations_completed, t.consultations_failed, t.independent_reviewers
    ));
    for c in &state.conclusions {
        if c.superseded_by.is_some() {
            continue;
        }
        out.push_str(&format!(
            "- Decision {}: {} — reviewed by {}; validation {} passed, {} failed.\n",
            c.id,
            one_line(&c.decision),
            if c.reviewed_by.is_empty() {
                "no independent advisor (local reasoning)".to_string()
            } else {
                c.reviewed_by.join(", ")
            },
            c.validations_passed,
            c.validations_failed
        ));
        for r in &c.risks {
            out.push_str(&format!("  - risk: {}\n", one_line(r)));
        }
    }
    for a in &state.assessments {
        if matches!(
            a.standing,
            AssessmentStanding::Open | AssessmentStanding::Planned | AssessmentStanding::Disputed
        ) {
            out.push_str(&format!(
                "- Unresolved {}: {} ({:?})\n",
                a.id,
                one_line(&a.subject),
                a.standing
            ));
            for q in &a.questions {
                out.push_str(&format!("  - open question: {}\n", one_line(q)));
            }
        }
    }
    for c in &state.consultations {
        if c.status != ConsultationStatus::Completed {
            out.push_str(&format!(
                "- Degradation: {} {} ({})\n",
                c.advisor,
                c.status.as_str(),
                c.id
            ));
        }
    }
    out
}

/// A record with everything it rests on and everything that rests on it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningExplanation {
    /// The record asked about.
    pub record: ReasoningRecord,
    /// The assessment it belongs to, when it belongs to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<String>,
    /// Every record of that assessment, in order: its provenance chain.
    pub chain: Vec<ReasoningStep>,
}

/// Explain one record: its assessment and the whole chain of that assessment.
pub fn explain(loaded: &Loaded, id: &str) -> Option<ReasoningExplanation> {
    let record = loaded.get(id)?.clone();
    let assessment = assessment_of(loaded, &record);
    // Derived over every record, so references resolve against everything; then the
    // steps of this assessment (or the record alone, when it belongs to none).
    let chain = derive(loaded, None)
        .timeline
        .into_iter()
        .filter(|e| match &assessment {
            Some(a) => loaded
                .get(&e.id)
                .and_then(|r| assessment_of(loaded, r))
                .is_some_and(|x| &x == a),
            None => e.id == id,
        })
        .collect();
    Some(ReasoningExplanation {
        record,
        assessment,
        chain,
    })
}
