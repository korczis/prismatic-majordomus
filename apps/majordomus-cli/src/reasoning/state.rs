//! The reasoning state of a session, derived from its records and nothing else: what is
//! uncertain, what was asked and what came back, where advice disagreed and what settled
//! it, what was concluded and whether it was proven — plus the timeline, the degradation
//! that happened along the way, and the report a handover or a final summary carries.
//!
//! Deterministic: the records are read in `(recorded_at, id)` order, every list here keeps
//! that order, and the report is rendered from these lists alone. The same records give
//! the same state on every surface, which is what lets the CLI, HTTP, MCP, the Cockpit,
//! `majordomus context` and a handover all say the same thing.
//!
//! The lifecycle: records are admitted and stored, [`derive()`] turns them into a
//! [`ReasoningState`], and [`render_markdown`] turns that state into the paragraph a
//! handover carries. [`explain`] answers for one record with its whole chain.
//!
//! ```
//! use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
//! use majordomus_cli::reasoning::record::*;
//! use majordomus_cli::reasoning::state::{derive, explain, render_markdown, AssessmentStanding};
//! use majordomus_cli::reasoning::store::{admit, Admission, Loaded};
//!
//! let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
//! let admission = Admission { mode: &mode, availability: &[] };
//! let mut loaded = Loaded::default();
//! let add = |loaded: &mut Loaded, input: ReasoningInput| {
//!     let body = admit(input, loaded, &admission).unwrap();
//!     let n = loaded.records.len() + 1;
//!     let id = format!("{}-{n}", body.kind());
//!     loaded.records.push(ReasoningRecord {
//!         schema: RECORD_SCHEMA.into(),
//!         version: RECORD_VERSION,
//!         id: id.clone(),
//!         task: "t".into(),
//!         recorded_at: format!("2026-10-01T00:00:{n:02}Z"),
//!         head: None,
//!         redacted: vec![],
//!         body,
//!     });
//!     id
//! };
//!
//! let a = add(&mut loaded, ReasoningInput::Assessment(AssessmentInput {
//!     subject: "whether the cache survives a restart".into(),
//!     ..Default::default()
//! }));
//! add(&mut loaded, ReasoningInput::Plan(PlanInput { assessment: a.clone() }));
//! let k = add(&mut loaded, ReasoningInput::Conclusion(ConclusionInput {
//!     assessment: a.clone(),
//!     decision: "rebuild the cache on start".into(),
//!     rationale: "nothing persists it".into(),
//!     ..Default::default()
//! }));
//! add(&mut loaded, ReasoningInput::Validation(ValidationInput {
//!     conclusion: k.clone(),
//!     check: "a restarted worker rebuilds it".into(),
//!     command: None,
//!     outcome: ValidationOutcome::Pass,
//!     evidence: vec![],
//! }));
//!
//! let state = derive(&loaded, Some("t"));
//! assert_eq!(state.assessments[0].standing, AssessmentStanding::Validated);
//! assert_eq!(state.timeline.len(), 4);
//! let report = render_markdown(&state);
//! assert!(report.contains("- Decision conclusion-3: rebuild the cache on start"));
//! assert_eq!(explain(&loaded, &k).unwrap().chain.len(), 4);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::policy::{Materiality, PlanOutcome};
use super::record::*;
use super::store::{assessment_of, resolution_of, standing_conclusion, Loaded};

/// Where an assessment stands, from open through validated.
///
/// The standing is derived, never stored: an unresolved disagreement makes it disputed
/// whatever else is recorded; otherwise its standing conclusion and that conclusion's
/// validations decide it, and without a conclusion a recorded plan makes it planned.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, AssessmentStanding};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("assessment-1", 1, json!({ "kind": "assessment", "subject": "s" })));
/// assert_eq!(derive(&loaded, None).assessments[0].standing, AssessmentStanding::Open);
///
/// loaded.records.push(rec("disagreement-2", 2, json!({
///     "kind": "disagreement", "assessment": "assessment-1",
///     "positions": [
///         { "source": "primary", "position": "A" },
///         { "source": "consultation-9", "position": "B" },
///     ],
///     "discriminating_question": "do callers retry?",
/// })));
/// assert_eq!(derive(&loaded, None).assessments[0].standing, AssessmentStanding::Disputed);
/// ```
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
///
/// The view carries what a reader needs to pick the work up: the subject, its
/// materiality, its derived standing, the conclusion that stands for it and the
/// questions it left open.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::policy::Materiality;
/// use majordomus_cli::reasoning::state::{derive, AssessmentView};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("assessment-1", 1, json!({
///     "kind": "assessment", "subject": "lock scope", "materiality": "trivial",
///     "questions": ["is it held across an await?"],
/// })));
/// let state = derive(&loaded, None);
/// let view: &AssessmentView = &state.assessments[0];
/// assert_eq!((view.id.as_str(), view.task.as_str()), ("assessment-1", "t"));
/// assert_eq!(view.materiality, Materiality::Trivial);
/// assert_eq!(view.conclusion, None);
/// assert_eq!(view.questions, ["is it held across an await?"]);
/// ```
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
///
/// A failed consultation is reported as plainly as a completed one: its status and the
/// adapter's diagnostic stand where the advice would, so degradation is visible rather
/// than silently absent.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::record::ConsultationStatus;
/// use majordomus_cli::reasoning::state::{derive, ConsultationView};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("consultation-1", 1, json!({
///     "kind": "consultation", "plan": "plan-0", "advisor": "alpha",
///     "status": "timeout", "diagnostic": "timeout: no answer in 60s",
/// })));
/// let state = derive(&loaded, None);
/// let view: &ConsultationView = &state.consultations[0];
/// assert_eq!(view.status, ConsultationStatus::Timeout);
/// assert_eq!(view.conclusion, None);
/// assert_eq!(view.diagnostic.as_deref(), Some("timeout: no answer in 60s"));
/// assert_eq!(state.totals.consultations_failed, 1);
/// ```
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
///
/// Besides the decision and its computed review, the view counts the validations
/// recorded against the conclusion and names the later conclusion that superseded it,
/// if any. A superseded conclusion stays in the list; the report leaves it out.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, ConclusionView};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let conclusion = |decision: &str, supersedes: Option<&str>| {
///     json!({
///         "kind": "conclusion", "assessment": "assessment-0", "decision": decision,
///         "rationale": "r", "supersedes": supersedes,
///         "reviewed_by": [], "independent_review_count": 0,
///     })
/// };
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("conclusion-1", 1, conclusion("A", None)));
/// loaded.records.push(rec("validation-2", 2, json!({
///     "kind": "validation", "conclusion": "conclusion-1", "check": "x", "outcome": "fail",
/// })));
/// loaded.records.push(rec("conclusion-3", 3, conclusion("B", Some("conclusion-1"))));
///
/// let state = derive(&loaded, None);
/// let first: &ConclusionView = &state.conclusions[0];
/// assert_eq!((first.validations_passed, first.validations_failed), (0, 1));
/// assert_eq!(first.superseded_by.as_deref(), Some("conclusion-3"));
/// assert_eq!(state.conclusions[1].superseded_by, None);
/// ```
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
///
/// The view names the question that decides the disagreement and, once a resolution is
/// recorded, the resolution and the position its evidence favoured. A view without a
/// resolution is an unsettled disagreement that blocks its assessment.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, DisagreementView};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("disagreement-1", 1, json!({
///     "kind": "disagreement", "assessment": "assessment-0",
///     "positions": [
///         { "source": "primary", "position": "A" },
///         { "source": "consultation-9", "position": "B" },
///     ],
///     "discriminating_question": "do callers retry?",
/// })));
/// loaded.records.push(rec("resolution-2", 2, json!({
///     "kind": "resolution", "disagreement": "disagreement-1",
///     "experiment": "read the callers", "outcome": "none retries",
///     "favours": "consultation-9",
///     "evidence": [{ "kind": "file", "reference": "src/caller.rs" }],
/// })));
/// let state = derive(&loaded, None);
/// let view: &DisagreementView = &state.disagreements[0];
/// assert_eq!(view.discriminating_question, "do callers retry?");
/// assert_eq!(view.resolution.as_deref(), Some("resolution-2"));
/// assert_eq!(view.favours.as_deref(), Some("consultation-9"));
/// ```
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
///
/// Each record maps to one phase: a plan to consulting, local or reuse by its outcome, a
/// consultation to an advisor response or a degradation by its status, and every other
/// kind to the phase of the same name.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, ReasoningPhase};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("consultation-1", 1, json!({
///     "kind": "consultation", "plan": "plan-0", "advisor": "alpha", "status": "unavailable",
/// })));
/// let phases: Vec<ReasoningPhase> =
///     derive(&loaded, None).timeline.iter().map(|s| s.phase).collect();
/// assert_eq!(phases, [ReasoningPhase::Degradation]);
/// let word = serde_json::to_value(ReasoningPhase::AdvisorResponse).unwrap();
/// assert_eq!(word, "advisor_response");
/// ```
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

/// One step of the timeline: when, which record, which phase, and one line saying what
/// happened.
///
/// The summary is cut to a single line of at most 140 characters, so a timeline renders
/// the same on a terminal, a page and a handover.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, ReasoningPhase, ReasoningStep};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("attempt-1", 1, json!({
///     "kind": "attempt", "attempt": "share the client\nacross threads",
///     "observed_failure": "the handle is not Sync",
/// })));
/// let state = derive(&loaded, None);
/// let step: &ReasoningStep = &state.timeline[0];
/// assert_eq!(step.at, "2026-10-01T00:00:01Z");
/// assert_eq!(step.phase, ReasoningPhase::Attempt);
/// assert_eq!(step.summary, "share the client failed: the handle is not Sync");
/// ```
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

/// Totals over the records of the derived task: counts, reviewers and reported tokens.
///
/// Independent reviewers are distinct advisors with at least one completed consultation;
/// token counts are summed only from what adapters reported, never estimated.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, ReasoningTotals};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let answer = |plan: &str| {
///     json!({
///         "kind": "consultation", "plan": plan, "advisor": "alpha", "status": "completed",
///         "conclusion": "A", "stance": "supports",
///         "usage": { "input_tokens": 100, "output_tokens": 20 },
///     })
/// };
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("consultation-1", 1, answer("plan-a")));
/// loaded.records.push(rec("consultation-2", 2, answer("plan-b")));
/// let totals: ReasoningTotals = derive(&loaded, None).totals;
/// assert_eq!(totals.consultations_completed, 2);
/// assert_eq!(totals.independent_reviewers, 1, "one advisor, however often asked");
/// assert_eq!((totals.input_tokens, totals.output_tokens), (200, 40));
/// ```
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
///
/// It is what every surface reports: the assessments, consultations, disagreements and
/// conclusions of the task, the timeline of every record, the totals, and the store
/// files that could not be read. Derived by [`derive()`], never stored.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, task: &str, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": task,
/// #         "recorded_at": "2026-10-01T00:00:00Z", "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, ReasoningState};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// let assessment = |subject: &str| json!({ "kind": "assessment", "subject": subject });
/// loaded.records.push(rec("assessment-1", "t1", assessment("a")));
/// loaded.records.push(rec("assessment-2", "t2", assessment("b")));
/// loaded.unreadable.push("t2/broken.json".into());
///
/// let one: ReasoningState = derive(&loaded, Some("t1"));
/// assert_eq!((one.task.as_str(), one.assessments.len()), ("t1", 1));
/// let all = derive(&loaded, None);
/// assert_eq!((all.task.as_str(), all.totals.records), ("all", 2));
/// assert_eq!(all.unreadable, ["t2/broken.json"]);
/// ```
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
///
/// Only the records of the task are reported, but references are resolved against every
/// loaded record, so a conclusion recorded under another task still settles an
/// assessment. The same records always give the same state.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, task: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": task,
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, AssessmentStanding};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// let assessment = json!({ "kind": "assessment", "subject": "s" });
/// loaded.records.push(rec("assessment-1", "t1", 1, assessment));
/// loaded.records.push(rec("conclusion-2", "t2", 2, json!({
///     "kind": "conclusion", "assessment": "assessment-1", "decision": "A", "rationale": "r",
///     "reviewed_by": [], "independent_review_count": 0,
/// })));
/// let state = derive(&loaded, Some("t1"));
/// assert_eq!(state.assessments[0].standing, AssessmentStanding::Concluded);
/// assert_eq!(state.assessments[0].conclusion.as_deref(), Some("conclusion-2"));
/// assert!(state.conclusions.is_empty(), "the conclusion is reported under its own task");
/// ```
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
///
/// The paragraph opens with the totals, then lists every standing decision with its
/// review and validation, every assessment still open, planned or disputed with its open
/// questions, and every consultation that degraded.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{derive, render_markdown};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// assert_eq!(render_markdown(&derive(&loaded, None)), "");
///
/// loaded.records.push(rec("assessment-1", 1, json!({
///     "kind": "assessment", "subject": "lock scope", "questions": ["held across an await?"],
/// })));
/// let report = render_markdown(&derive(&loaded, None));
/// let lines: Vec<&str> = report.lines().collect();
/// assert_eq!(
///     lines,
///     [
///         "Reasoning: 1 record(s); 0 consultation(s) completed, 0 failed; \
///          0 independent reviewer(s).",
///         "- Unresolved assessment-1: lock scope (Open)",
///         "  - open question: held across an await?",
///     ]
/// );
/// ```
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
///
/// It carries the record itself, the assessment it belongs to, and that assessment's
/// whole provenance chain as timeline steps in order. A record that belongs to no
/// assessment has a chain of itself alone.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{explain, ReasoningExplanation};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("attempt-1", 1, json!({
///     "kind": "attempt", "attempt": "inline it", "observed_failure": "still slow",
/// })));
/// let e: ReasoningExplanation = explain(&loaded, "attempt-1").unwrap();
/// assert_eq!(e.record.id, "attempt-1");
/// assert_eq!(e.assessment, None);
/// assert_eq!(e.chain.len(), 1);
/// ```
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
///
/// The chain is derived over every task, so a record is explained with everything it
/// rests on wherever that was recorded. An unknown id explains nothing.
///
/// ```
/// # use majordomus_cli::reasoning::record::ReasoningRecord;
/// # use serde_json::json;
/// # fn rec(id: &str, n: u32, body: serde_json::Value) -> ReasoningRecord {
/// #     serde_json::from_value(json!({
/// #         "schema": "majordomus/reasoning-record", "version": 1, "id": id, "task": "t",
/// #         "recorded_at": format!("2026-10-01T00:00:{n:02}Z"), "body": body,
/// #     }))
/// #     .unwrap()
/// # }
/// use majordomus_cli::reasoning::state::{explain, ReasoningPhase};
/// use majordomus_cli::reasoning::store::Loaded;
///
/// let mut loaded = Loaded::default();
/// loaded.records.push(rec("assessment-1", 1, json!({ "kind": "assessment", "subject": "s" })));
/// loaded.records.push(rec("attempt-2", 2, json!({
///     "kind": "attempt", "attempt": "unrelated", "observed_failure": "x",
/// })));
/// loaded.records.push(rec("conclusion-3", 3, json!({
///     "kind": "conclusion", "assessment": "assessment-1", "decision": "A", "rationale": "r",
///     "reviewed_by": [], "independent_review_count": 0,
/// })));
///
/// let e = explain(&loaded, "conclusion-3").unwrap();
/// assert_eq!(e.assessment.as_deref(), Some("assessment-1"));
/// let phases: Vec<ReasoningPhase> = e.chain.iter().map(|s| s.phase).collect();
/// assert_eq!(phases, [ReasoningPhase::Uncertainty, ReasoningPhase::Conclusion]);
/// assert!(explain(&loaded, "conclusion-9").is_none());
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, n: u32, body: ReasoningBody) -> ReasoningRecord {
        ReasoningRecord {
            schema: RECORD_SCHEMA.into(),
            version: RECORD_VERSION,
            id: id.into(),
            task: "t".into(),
            recorded_at: format!("2026-10-01T00:00:{n:02}Z"),
            head: None,
            redacted: vec![],
            body,
        }
    }

    fn conclusion(assessment: &str) -> ReasoningBody {
        ReasoningBody::Conclusion(StoredConclusion {
            input: ConclusionInput {
                assessment: assessment.into(),
                decision: "A".into(),
                rationale: "r".into(),
                ..Default::default()
            },
            reviewed_by: vec![],
            independent_review_count: 0,
        })
    }

    fn validation(conclusion: &str, outcome: ValidationOutcome) -> ReasoningBody {
        ReasoningBody::Validation(ValidationInput {
            conclusion: conclusion.into(),
            check: "x".into(),
            command: None,
            outcome,
            evidence: vec![],
        })
    }

    #[test]
    fn one_failed_validation_outweighs_any_number_that_passed() {
        let mut loaded = Loaded::default();
        loaded.records.push(rec(
            "assessment-1",
            1,
            ReasoningBody::Assessment(AssessmentInput {
                subject: "s".into(),
                ..Default::default()
            }),
        ));
        loaded
            .records
            .push(rec("conclusion-2", 2, conclusion("assessment-1")));
        loaded.records.push(rec(
            "validation-3",
            3,
            validation("conclusion-2", ValidationOutcome::Pass),
        ));
        assert_eq!(
            derive(&loaded, None).assessments[0].standing,
            AssessmentStanding::Validated
        );
        loaded.records.push(rec(
            "validation-4",
            4,
            validation("conclusion-2", ValidationOutcome::Fail),
        ));
        assert_eq!(
            derive(&loaded, None).assessments[0].standing,
            AssessmentStanding::ValidationFailed
        );
    }

    #[test]
    fn a_long_summary_is_cut_to_one_line() {
        let long = "x".repeat(200);
        assert_eq!(one_line(&long).chars().count(), 140);
        assert!(one_line(&long).ends_with('…'));
        assert_eq!(one_line("first\nsecond"), "first");
    }

    #[test]
    fn a_degraded_consultation_is_reported() {
        let mut loaded = Loaded::default();
        loaded.records.push(rec(
            "consultation-1",
            1,
            ReasoningBody::Consultation(ConsultationInput {
                plan: "plan-0".into(),
                advisor: "alpha".into(),
                status: ConsultationStatus::RateLimited,
                question: None,
                evidence: vec![],
                conclusion: None,
                stance: None,
                assumptions: vec![],
                risks: vec![],
                actions: vec![],
                falsifiers: vec![],
                confidence: None,
                duration_ms: None,
                usage: None,
                diagnostic: None,
                retry_after_seconds: Some(30),
            }),
        ));
        let report = render_markdown(&derive(&loaded, None));
        assert!(
            report.contains("- Degradation: alpha rate_limited (consultation-1)"),
            "{report}"
        );
    }
}
