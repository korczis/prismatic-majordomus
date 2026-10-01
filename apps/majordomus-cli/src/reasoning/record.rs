//! The records a session writes while it reasons: typed engineering artefacts, never a
//! transcript and never hidden reasoning. What is kept is what a later session needs to
//! avoid rediscovering a decision — facts, evidence references, assumptions, questions,
//! positions, conclusions, validation — and nothing an advisor said verbatim beyond the
//! summary its adapter normalised.
//!
//! Inputs are what a caller authors; the stored body adds what only the writer may
//! compute: a plan is computed from the availability at the moment it is recorded, a
//! conclusion's review count from the consultations it cites. A caller cannot supply
//! either, because a review count a caller can type is a review a caller can invent.
//!
//! The lifecycle of one record: a caller authors a [`ReasoningInput`] (usually as JSON,
//! tagged by `kind`), the store admits it into a [`ReasoningBody`], and the writer wraps
//! that body in a [`ReasoningRecord`] stamped with an id, a task and a time.
//!
//! ```
//! use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
//! use majordomus_cli::reasoning::record::{
//!     ReasoningBody, ReasoningInput, ReasoningRecord, RECORD_SCHEMA, RECORD_VERSION,
//! };
//! use majordomus_cli::reasoning::store::{admit, Admission, Loaded};
//!
//! let input: ReasoningInput = serde_json::from_value(serde_json::json!({
//!     "kind": "assessment",
//!     "subject": "whether the cache survives a restart",
//!     "materiality": "low",
//! }))
//! .unwrap();
//! let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
//! let admission = Admission { mode: &mode, availability: &[] };
//! let body: ReasoningBody = admit(input, &Loaded::default(), &admission).unwrap();
//! assert_eq!(body.kind(), "assessment");
//!
//! let record = ReasoningRecord {
//!     schema: RECORD_SCHEMA.into(),
//!     version: RECORD_VERSION,
//!     id: "assessment-20261001T000000Z-abcdef".into(),
//!     task: "untasked".into(),
//!     recorded_at: "2026-10-01T00:00:00Z".into(),
//!     head: None,
//!     redacted: vec![],
//!     body,
//! };
//! let json = serde_json::to_value(&record).unwrap();
//! assert_eq!(json["body"]["kind"], "assessment");
//! assert_eq!(json["schema"], "majordomus/reasoning-record");
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::availability::AdvisorState;
use super::policy::{Materiality, ReasoningConfidence, ReviewPlan};

/// The schema name every stored record carries.
pub const RECORD_SCHEMA: &str = "majordomus/reasoning-record";
/// The record format version.
pub const RECORD_VERSION: u32 = 1;

/// What a piece of evidence is.
///
/// The kind tells a reader how to follow the reference: open a file, run a test, read an
/// ADR or a rule, show a commit. A prior external opinion is a kind of its own so that an
/// advisor's earlier conclusion, handed to another advisor, is never read as a fact.
///
/// ```
/// use majordomus_cli::reasoning::record::ReasoningEvidenceKind;
///
/// let kind: ReasoningEvidenceKind = serde_json::from_str("\"prior_external_opinion\"").unwrap();
/// assert_eq!(kind, ReasoningEvidenceKind::PriorExternalOpinion);
/// assert_eq!(serde_json::to_string(&ReasoningEvidenceKind::Adr).unwrap(), "\"adr\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEvidenceKind {
    /// A file of the repository, optionally with a line: `src/x.rs:42`.
    File,
    /// A test case or test name.
    Test,
    /// An architecture decision record.
    Adr,
    /// A rule of the layer.
    Rule,
    /// A commit.
    Commit,
    /// A command that was run, and what it showed.
    Command,
    /// Another reasoning record.
    Record,
    /// Documentation.
    Document,
    /// A runtime observation: a log, a measurement, a diagnostic.
    Runtime,
    /// A prior external opinion: an advisor's conclusion handed to another advisor.
    /// Marked so it is never mistaken for a fact.
    PriorExternalOpinion,
}

/// A reference to something that supports a claim.
///
/// Evidence is a pointer, never a copy: a path, a case, an id or a command line, with an
/// optional one-line note of what it shows. Unknown fields are refused, so a misspelt
/// field is an error rather than a silently dropped claim.
///
/// ```
/// use majordomus_cli::reasoning::record::{ReasoningEvidence, ReasoningEvidenceKind};
///
/// let e: ReasoningEvidence = serde_json::from_value(serde_json::json!({
///     "kind": "file",
///     "reference": "src/worker.rs:42",
/// }))
/// .unwrap();
/// assert_eq!(e.kind, ReasoningEvidenceKind::File);
/// assert_eq!(e.note, None);
///
/// let typo = serde_json::from_value::<ReasoningEvidence>(serde_json::json!({
///     "kind": "file", "reference": "x", "notes": "misspelt",
/// }));
/// assert!(typo.is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReasoningEvidence {
    /// What it is.
    pub kind: ReasoningEvidenceKind,
    /// Where it is: a path, a case, an id, a command line.
    pub reference: String,
    /// What it shows, in one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A material uncertainty, stated with the evidence gathered before anyone was asked.
///
/// The assessment opens a chain: plans, consultations, disagreements and conclusions all
/// name it. Only `subject` is required; materiality defaults to low and confidence to
/// medium. The store refuses a material assessment that carries no evidence.
///
/// ```
/// use majordomus_cli::reasoning::record::AssessmentInput;
/// use majordomus_cli::reasoning::policy::{Materiality, ReasoningConfidence};
///
/// let a: AssessmentInput =
///     serde_json::from_value(serde_json::json!({ "subject": "retry policy" })).unwrap();
/// assert_eq!(a.materiality, Materiality::Low);
/// assert_eq!(a.confidence, ReasoningConfidence::Medium);
/// assert!(a.evidence.is_empty() && !a.new_evidence);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssessmentInput {
    /// What is uncertain, as one line. Later assessments of the same subject find the
    /// conclusions of earlier ones through it.
    pub subject: String,
    /// How much the decision could matter.
    #[serde(default)]
    pub materiality: Materiality,
    /// How sure the executor is of its hypothesis.
    #[serde(default)]
    pub confidence: ReasoningConfidence,
    /// The current hypothesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
    /// What the decision could affect: `public_api`, `persistent_data`, `security`,
    /// `concurrency`, `architecture`, ...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affects: Vec<String>,
    /// What is known.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
    /// What is assumed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// What is still open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<String>,
    /// The evidence gathered locally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ReasoningEvidence>,
    /// The advisory capabilities review would need; empty means `independent_reasoning`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// The assessment brings evidence a standing conclusion on the subject did not have,
    /// so that conclusion does not answer it.
    #[serde(default)]
    pub new_evidence: bool,
}

/// Ask the writer to make and record the plan for an assessment.
///
/// The input names only the assessment: the plan itself is computed by the store from
/// the mode and the availability at the moment it is admitted, so a caller cannot write
/// a plan that selects an advisor nobody could ask.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::record::{PlanInput, ReasoningInput};
/// use majordomus_cli::reasoning::store::{admit, Admission, Loaded};
///
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let admission = Admission { mode: &mode, availability: &[] };
/// let input = ReasoningInput::Plan(PlanInput { assessment: "assessment-1".into() });
/// let refused = admit(input, &Loaded::default(), &admission).unwrap_err();
/// assert_eq!(refused.0, "no assessment assessment-1 is recorded");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanInput {
    /// The assessment the plan answers.
    pub assessment: String,
}

/// A plan as stored: the decision and the availability it was made against.
///
/// The store builds it when it admits a [`PlanInput`]; nothing else does. The snapshot of
/// availability is what a later consultation is admitted against, so an answer that
/// arrives after the advisor's circuit opened still belongs to the plan that asked for it.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::PlanOutcome;
/// use majordomus_cli::reasoning::record::*;
/// use majordomus_cli::reasoning::store::{admit, Admission, Loaded};
///
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let admission = Admission { mode: &mode, availability: &[] };
/// let mut loaded = Loaded::default();
/// loaded.records.push(ReasoningRecord {
///     schema: RECORD_SCHEMA.into(),
///     version: RECORD_VERSION,
///     id: "assessment-1".into(),
///     task: "t".into(),
///     recorded_at: "2026-10-01T00:00:01Z".into(),
///     head: None,
///     redacted: vec![],
///     body: ReasoningBody::Assessment(AssessmentInput {
///         subject: "a name".into(),
///         ..Default::default()
///     }),
/// });
/// let input = ReasoningInput::Plan(PlanInput { assessment: "assessment-1".into() });
/// let ReasoningBody::Plan(stored) = admit(input, &loaded, &admission).unwrap() else {
///     panic!("a plan input is stored as a plan");
/// };
/// let stored: StoredPlan = stored;
/// assert_eq!(stored.assessment, "assessment-1");
/// assert_eq!(stored.plan.outcome, PlanOutcome::DecideLocally);
/// assert!(stored.availability.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StoredPlan {
    /// The assessment it answers.
    pub assessment: String,
    /// The decision.
    pub plan: ReviewPlan,
    /// Every advisor's standing at the moment of the plan: the snapshot a later
    /// consultation is admitted against.
    pub availability: Vec<AdvisorState>,
}

/// How a consultation ended, as the adapter classified it.
///
/// Only a completed consultation carries advice and counts as review; every other status
/// is a degradation that is recorded so the state can report it, never hidden. The wire
/// form is the snake_case word that [`ConsultationStatus::as_str`] returns.
///
/// ```
/// use majordomus_cli::reasoning::record::ConsultationStatus;
///
/// let s: ConsultationStatus = serde_json::from_str("\"rate_limited\"").unwrap();
/// assert_eq!(s, ConsultationStatus::RateLimited);
/// assert!(!s.is_transient());
/// assert!(ConsultationStatus::Timeout.is_transient());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConsultationStatus {
    /// An answer was received and normalised.
    Completed,
    /// No answer within the time allowed.
    Timeout,
    /// The advisor refused for rate.
    RateLimited,
    /// The credential was refused.
    AuthFailed,
    /// An answer arrived that could not be normalised.
    Malformed,
    /// An answer arrived with nothing in it.
    Empty,
    /// The advisor could not be reached: not running, not answering, gone.
    Unavailable,
    /// The session withdrew the request.
    Cancelled,
    /// Anything else the adapter could not classify.
    Error,
}

impl ConsultationStatus {
    /// A failure that says the advisor may not be askable right now.
    ///
    /// A timeout, an unreachable advisor and an unclassified error are transient: they
    /// count towards opening the advisor's circuit. A refused credential, a rate limit or
    /// a malformed answer say something more specific and are not.
    ///
    /// ```
    /// use majordomus_cli::reasoning::record::ConsultationStatus;
    ///
    /// assert!(ConsultationStatus::Unavailable.is_transient());
    /// assert!(!ConsultationStatus::Completed.is_transient());
    /// assert!(!ConsultationStatus::AuthFailed.is_transient());
    /// ```
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Timeout | Self::Unavailable | Self::Error)
    }

    /// The status as the snake_case word the records and refusals spell it with.
    ///
    /// It is the same word serde writes, so a refusal message and a stored record never
    /// disagree about how a status is named.
    ///
    /// ```
    /// use majordomus_cli::reasoning::record::ConsultationStatus;
    ///
    /// let s = ConsultationStatus::AuthFailed;
    /// assert_eq!(s.as_str(), "auth_failed");
    /// assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.as_str()));
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::AuthFailed => "auth_failed",
            Self::Malformed => "malformed",
            Self::Empty => "empty",
            Self::Unavailable => "unavailable",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }
}

/// Where an advisor's answer stands relative to the executor's hypothesis.
///
/// A completed consultation must state its stance; the store refuses one without it. The
/// stance is a label for the timeline, not a vote: agreement settles nothing.
///
/// ```
/// use majordomus_cli::reasoning::record::Stance;
///
/// let s: Stance = serde_json::from_str("\"alternative\"").unwrap();
/// assert_eq!(s, Stance::Alternative);
/// assert!(serde_json::from_str::<Stance>("\"agrees\"").is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// It supports the hypothesis.
    Supports,
    /// It argues against it.
    Opposes,
    /// It proposes something else.
    Alternative,
    /// It does not decide.
    Inconclusive,
}

/// Token counts an adapter reported. Absent when the transport reports none; never
/// estimated.
///
/// The state sums these into its totals. A missing count reads as zero, so an adapter
/// that reports only one of the two still records what it reported.
///
/// ```
/// use majordomus_cli::reasoning::record::AdvisorUsage;
///
/// let u: AdvisorUsage = serde_json::from_str(r#"{"input_tokens": 120}"#).unwrap();
/// assert_eq!(u, AdvisorUsage { input_tokens: 120, output_tokens: 0 });
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisorUsage {
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: u64,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: u64,
}

/// One normalised advisor exchange: what was asked, how it ended, and what came back.
///
/// The store admits it only when the plan it names selected this advisor. A completed
/// consultation carries a conclusion and a stance; a failed one carries neither, only its
/// status and the adapter's diagnostic. The full prompt and answer are never kept.
///
/// ```
/// use majordomus_cli::reasoning::record::{ConsultationInput, ConsultationStatus};
///
/// let c: ConsultationInput = serde_json::from_value(serde_json::json!({
///     "plan": "plan-1",
///     "advisor": "alpha",
///     "status": "timeout",
///     "diagnostic": "timeout: no answer in 60s",
/// }))
/// .unwrap();
/// assert_eq!(c.status, ConsultationStatus::Timeout);
/// assert!(c.conclusion.is_none() && c.stance.is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsultationInput {
    /// The recorded plan that selected this advisor.
    pub plan: String,
    /// The advisor asked.
    pub advisor: String,
    /// How it ended.
    pub status: ConsultationStatus,
    /// The question asked, in one line (the full prompt is not kept).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// The evidence the question carried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ReasoningEvidence>,
    /// The advisor's conclusion, summarised. Required when completed, refused otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// Where it stands relative to the hypothesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<Stance>,
    /// Assumptions it identified.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// Risks it identified.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    /// Actions it recommended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// What would falsify its conclusion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub falsifiers: Vec<String>,
    /// Its stated confidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ReasoningConfidence>,
    /// How long it took, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// What the transport reported it cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AdvisorUsage>,
    /// The adapter's diagnostic for a failure: a class and a sentence, never a payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    /// A rate limit's retry-after, seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

/// One side of a disagreement: whose position it is, and the position in one line.
///
/// The source is `primary` for the executor, or the id of a completed consultation; the
/// store refuses a position sourced from a consultation that did not complete.
///
/// ```
/// use majordomus_cli::reasoning::record::DisagreementPosition;
///
/// let p = DisagreementPosition { source: "primary".into(), position: "keep the lock".into() };
/// let json = serde_json::to_value(&p).unwrap();
/// assert_eq!(json, serde_json::json!({ "source": "primary", "position": "keep the lock" }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisagreementPosition {
    /// Whose position: `primary`, or a completed consultation's record id.
    pub source: String,
    /// The position, in one line.
    pub position: String,
}

/// Advisors, or an advisor and the executor, disagree.
///
/// A disagreement names the question whose answer decides between the positions; until
/// a [`ResolutionInput`] settles it with evidence, the store refuses every conclusion on
/// its assessment. It needs at least two positions, each from its own source.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::record::*;
/// use majordomus_cli::reasoning::store::{admit, Admission, Loaded};
///
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let admission = Admission { mode: &mode, availability: &[] };
/// let mut loaded = Loaded::default();
/// loaded.records.push(ReasoningRecord {
///     schema: RECORD_SCHEMA.into(),
///     version: RECORD_VERSION,
///     id: "assessment-1".into(),
///     task: "t".into(),
///     recorded_at: "2026-10-01T00:00:01Z".into(),
///     head: None,
///     redacted: vec![],
///     body: ReasoningBody::Assessment(AssessmentInput {
///         subject: "lock scope".into(),
///         ..Default::default()
///     }),
/// });
/// let lonely = DisagreementInput {
///     assessment: "assessment-1".into(),
///     positions: vec![DisagreementPosition {
///         source: "primary".into(),
///         position: "keep the lock".into(),
///     }],
///     divergent_assumptions: vec![],
///     discriminating_question: "does any caller hold it across an await?".into(),
///     experiment: None,
/// };
/// let refused = admit(ReasoningInput::Disagreement(lonely), &loaded, &admission).unwrap_err();
/// assert_eq!(refused.0, "a disagreement has at least two positions");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisagreementInput {
    /// The assessment it is about.
    pub assessment: String,
    /// The positions; at least two.
    pub positions: Vec<DisagreementPosition>,
    /// The assumptions the positions divide on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub divergent_assumptions: Vec<String>,
    /// The question whose answer decides between them.
    pub discriminating_question: String,
    /// The experiment proposed to answer it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<String>,
}

/// A disagreement settled by evidence: what was run or read, and which position it favours.
///
/// The resolution is the only way past a disagreement. It must cite at least one piece of
/// evidence, because a disagreement is settled by an experiment, never by counting who
/// agrees; it favours one of the disagreement's sources, or `neither`.
///
/// ```
/// use majordomus_cli::reasoning::record::ResolutionInput;
///
/// let r: ResolutionInput = serde_json::from_value(serde_json::json!({
///     "disagreement": "disagreement-1",
///     "experiment": "run the supervision test",
///     "outcome": "no caller retries",
///     "favours": "neither",
///     "evidence": [{ "kind": "test", "reference": "test/supervision.rs" }],
/// }))
/// .unwrap();
/// assert_eq!(r.favours, "neither");
/// assert_eq!(r.evidence.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolutionInput {
    /// The disagreement it settles.
    pub disagreement: String,
    /// The experiment performed.
    pub experiment: String,
    /// What it showed.
    pub outcome: String,
    /// The position it favours: a `source` of the disagreement, or `neither`.
    pub favours: String,
    /// The evidence it produced; at least one reference.
    pub evidence: Vec<ReasoningEvidence>,
}

/// The engineering conclusion an assessment reaches.
///
/// A conclusion names its decision and why. On material uncertainty it must also carry
/// evidence and a validation plan, and it must weigh every completed consultation of its
/// assessment. The review count is not part of the input: the store computes it.
///
/// ```
/// use majordomus_cli::reasoning::policy::ReasoningConfidence;
/// use majordomus_cli::reasoning::record::ConclusionInput;
///
/// let c = ConclusionInput {
///     assessment: "assessment-1".into(),
///     decision: "keep the lock".into(),
///     rationale: "no caller holds it across an await".into(),
///     ..Default::default()
/// };
/// assert_eq!(c.confidence, ReasoningConfidence::Medium);
/// assert!(c.consultations.is_empty() && c.supersedes.is_none());
/// // a review count is computed by the store, so the input has no field for one
/// let typed = serde_json::json!({
///     "assessment": "a", "decision": "d", "rationale": "r", "independent_review_count": 3,
/// });
/// assert!(serde_json::from_value::<ConclusionInput>(typed).is_err());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConclusionInput {
    /// The assessment it concludes.
    pub assessment: String,
    /// The decision.
    pub decision: String,
    /// Why, from the evidence.
    pub rationale: String,
    /// The evidence it rests on. Required for material uncertainty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ReasoningEvidence>,
    /// The completed consultations it weighed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consultations: Vec<String>,
    /// The resolved disagreements it rests on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disagreements: Vec<String>,
    /// Alternatives considered and rejected, each with why.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<String>,
    /// Assumptions it still depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// Risks that remain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    /// How the decision will be proven. Required for material uncertainty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_plan: Vec<String>,
    /// Confidence in it.
    #[serde(default)]
    pub confidence: ReasoningConfidence,
    /// An earlier conclusion this one replaces, because new evidence overturned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

/// A conclusion as stored: the input and what only the writer computes.
///
/// The input is flattened, so a stored conclusion reads as the input plus two fields:
/// the advisors whose completed consultations it cites, each once and in canonical
/// order, and their count. Both are computed at admission and checked again by
/// `reasoning.check`, so a hand-edited count is reported.
///
/// ```
/// use majordomus_cli::reasoning::record::{ConclusionInput, StoredConclusion};
///
/// let stored = StoredConclusion {
///     input: ConclusionInput {
///         assessment: "assessment-1".into(),
///         decision: "keep the lock".into(),
///         rationale: "measured".into(),
///         ..Default::default()
///     },
///     reviewed_by: vec!["alpha".into()],
///     independent_review_count: 1,
/// };
/// let json = serde_json::to_value(&stored).unwrap();
/// assert_eq!(json["decision"], "keep the lock");
/// assert_eq!(json["independent_review_count"], 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StoredConclusion {
    /// What the executor concluded.
    #[serde(flatten)]
    pub input: ConclusionInput,
    /// The advisors whose completed consultations it cites, each once. Computed.
    pub reviewed_by: Vec<String>,
    /// How many independent advisors reviewed it: `reviewed_by`'s length. Computed.
    pub independent_review_count: usize,
}

/// Whether a performed validation step passed or failed.
///
/// One failed validation is enough to mark the assessment's standing as
/// `validation_failed` in the derived state, whatever else passed.
///
/// ```
/// use majordomus_cli::reasoning::record::ValidationOutcome;
///
/// let o: ValidationOutcome = serde_json::from_str("\"fail\"").unwrap();
/// assert_eq!(o, ValidationOutcome::Fail);
/// assert_eq!(serde_json::to_string(&ValidationOutcome::Pass).unwrap(), "\"pass\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOutcome {
    /// It passed.
    Pass,
    /// It failed.
    Fail,
}

/// A step of a conclusion's validation plan, performed.
///
/// It names the conclusion it validates, what was checked, optionally the command, and
/// how it came out. The store refuses one whose conclusion is not recorded or whose check
/// is empty.
///
/// ```
/// use majordomus_cli::reasoning::record::{ValidationInput, ValidationOutcome};
///
/// let v: ValidationInput = serde_json::from_value(serde_json::json!({
///     "conclusion": "conclusion-1",
///     "check": "the worker test passes",
///     "command": "cargo test worker",
///     "outcome": "pass",
/// }))
/// .unwrap();
/// assert_eq!(v.outcome, ValidationOutcome::Pass);
/// assert_eq!(v.command.as_deref(), Some("cargo test worker"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationInput {
    /// The conclusion validated.
    pub conclusion: String,
    /// What was checked, in one line.
    pub check: String,
    /// The command that checked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// How it came out.
    pub outcome: ValidationOutcome,
    /// What it produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ReasoningEvidence>,
}

/// A failed implementation attempt and what it taught.
///
/// An attempt may stand alone or bear on an assessment. It records the observed failure
/// and, when known, the assumption the failure invalidated, so that a later session does
/// not try the same thing again.
///
/// ```
/// use majordomus_cli::reasoning::record::AttemptInput;
///
/// let a: AttemptInput = serde_json::from_value(serde_json::json!({
///     "attempt": "share the client across threads",
///     "observed_failure": "the borrow checker refused the handle",
///     "assumption_invalidated": "the client is Sync",
/// }))
/// .unwrap();
/// assert!(a.assessment.is_none());
/// assert_eq!(a.assumption_invalidated.as_deref(), Some("the client is Sync"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttemptInput {
    /// The assessment it bears on, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<String>,
    /// What was tried.
    pub attempt: String,
    /// What failed, observed.
    pub observed_failure: String,
    /// The assumption the failure invalidated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assumption_invalidated: Option<String>,
    /// Evidence the failure produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ReasoningEvidence>,
    /// The hypothesis now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_hypothesis: Option<String>,
}

/// Anything a session records, as authored.
///
/// The JSON form is tagged by `kind`, with the variant's fields beside the tag. This is
/// what the CLI, HTTP and MCP surfaces accept; [`crate::reasoning::store::admit`] turns
/// it into the [`ReasoningBody`] that is stored, or refuses it.
///
/// ```
/// use majordomus_cli::reasoning::record::ReasoningInput;
///
/// let input: ReasoningInput = serde_json::from_value(serde_json::json!({
///     "kind": "plan",
///     "assessment": "assessment-1",
/// }))
/// .unwrap();
/// assert!(matches!(input, ReasoningInput::Plan(ref p) if p.assessment == "assessment-1"));
/// let unknown = serde_json::json!({ "kind": "opinion", "text": "x" });
/// assert!(serde_json::from_value::<ReasoningInput>(unknown).is_err());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningInput {
    /// A material uncertainty.
    Assessment(AssessmentInput),
    /// Make and record the plan for an assessment.
    Plan(PlanInput),
    /// One advisor exchange.
    Consultation(ConsultationInput),
    /// A disagreement.
    Disagreement(DisagreementInput),
    /// A disagreement settled by evidence.
    Resolution(ResolutionInput),
    /// An engineering conclusion.
    Conclusion(ConclusionInput),
    /// A validation step performed.
    Validation(ValidationInput),
    /// A failed attempt.
    Attempt(AttemptInput),
}

/// A stored record's body: the admitted input, with whatever the writer computed.
///
/// It mirrors [`ReasoningInput`] variant for variant, except that a plan is stored as a
/// [`StoredPlan`] and a conclusion as a [`StoredConclusion`]. Only the store produces one,
/// by admitting an input; its JSON form is tagged by `kind` like the input's.
///
/// ```
/// use majordomus_cli::reasoning::record::{AssessmentInput, ReasoningBody};
///
/// let body = ReasoningBody::Assessment(AssessmentInput {
///     subject: "lock scope".into(),
///     ..Default::default()
/// });
/// assert_eq!(body.kind(), "assessment");
/// assert_eq!(serde_json::to_value(&body).unwrap()["kind"], "assessment");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningBody {
    /// A material uncertainty.
    Assessment(AssessmentInput),
    /// A plan, with its availability snapshot.
    Plan(StoredPlan),
    /// One advisor exchange.
    Consultation(ConsultationInput),
    /// A disagreement.
    Disagreement(DisagreementInput),
    /// A disagreement settled by evidence.
    Resolution(ResolutionInput),
    /// An engineering conclusion, with its computed review.
    Conclusion(StoredConclusion),
    /// A validation step performed.
    Validation(ValidationInput),
    /// A failed attempt.
    Attempt(AttemptInput),
}

impl ReasoningBody {
    /// The kind word, which is also the id prefix.
    ///
    /// It is the same word as the serde `kind` tag, so a record's id, its JSON and every
    /// refusal that names its kind agree.
    ///
    /// ```
    /// use majordomus_cli::reasoning::record::{AttemptInput, ReasoningBody};
    ///
    /// let body = ReasoningBody::Attempt(AttemptInput {
    ///     assessment: None,
    ///     attempt: "inline the call".into(),
    ///     observed_failure: "the test still fails".into(),
    ///     assumption_invalidated: None,
    ///     evidence: vec![],
    ///     updated_hypothesis: None,
    /// });
    /// assert_eq!(body.kind(), "attempt");
    /// assert_eq!(serde_json::to_value(&body).unwrap()["kind"], body.kind());
    /// ```
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Assessment(_) => "assessment",
            Self::Plan(_) => "plan",
            Self::Consultation(_) => "consultation",
            Self::Disagreement(_) => "disagreement",
            Self::Resolution(_) => "resolution",
            Self::Conclusion(_) => "conclusion",
            Self::Validation(_) => "validation",
            Self::Attempt(_) => "attempt",
        }
    }
}

/// A stored record: a body with the envelope only the writer stamps.
///
/// The envelope carries the schema and version, a unique id, the task, the time it was
/// recorded, the commit the checkout was on and the secret shapes redacted from its text.
/// One record is one JSON file in the store; the store reads back only records whose
/// schema and version match.
///
/// ```
/// use majordomus_cli::reasoning::record::*;
///
/// let record = ReasoningRecord {
///     schema: RECORD_SCHEMA.into(),
///     version: RECORD_VERSION,
///     id: "attempt-20261001T000000Z-abcdef".into(),
///     task: "untasked".into(),
///     recorded_at: "2026-10-01T00:00:00Z".into(),
///     head: None,
///     redacted: vec![],
///     body: ReasoningBody::Attempt(AttemptInput {
///         assessment: None,
///         attempt: "inline the call".into(),
///         observed_failure: "the test still fails".into(),
///         assumption_invalidated: None,
///         evidence: vec![],
///         updated_hypothesis: None,
///     }),
/// };
/// let text = serde_json::to_string(&record).unwrap();
/// assert!(!text.contains("\"head\""), "an absent head is not written");
/// let back: ReasoningRecord = serde_json::from_str(&text).unwrap();
/// assert_eq!(back.id, record.id);
/// assert_eq!(back.body.kind(), "attempt");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningRecord {
    /// [`RECORD_SCHEMA`].
    pub schema: String,
    /// [`RECORD_VERSION`].
    pub version: u32,
    /// `<kind>-<yyyymmddThhmmssZ>-<hex>`, unique in the store.
    pub id: String,
    /// The task it was recorded under, or `untasked`.
    pub task: String,
    /// When it was recorded, RFC 3339; stamped by the writer, never supplied.
    pub recorded_at: String,
    /// The commit the checkout was on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Secret shapes the writer removed from the text, by shape name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redacted: Vec<String>,
    /// The record.
    pub body: ReasoningBody,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_body_kind_is_its_serde_tag() {
        let bodies = vec![
            ReasoningBody::Assessment(AssessmentInput::default()),
            ReasoningBody::Conclusion(StoredConclusion {
                input: ConclusionInput::default(),
                reviewed_by: vec![],
                independent_review_count: 0,
            }),
            ReasoningBody::Validation(ValidationInput {
                conclusion: "c".into(),
                check: "x".into(),
                command: None,
                outcome: ValidationOutcome::Pass,
                evidence: vec![],
            }),
        ];
        for body in bodies {
            let json = serde_json::to_value(&body).unwrap();
            assert_eq!(json["kind"], body.kind());
        }
    }

    #[test]
    fn a_status_word_is_its_serde_word() {
        for s in [
            ConsultationStatus::Completed,
            ConsultationStatus::Timeout,
            ConsultationStatus::RateLimited,
            ConsultationStatus::AuthFailed,
            ConsultationStatus::Malformed,
            ConsultationStatus::Empty,
            ConsultationStatus::Unavailable,
            ConsultationStatus::Cancelled,
            ConsultationStatus::Error,
        ] {
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                serde_json::Value::String(s.as_str().into())
            );
        }
    }

    #[test]
    fn a_caller_cannot_type_a_review_count() {
        let typed = serde_json::json!({
            "kind": "conclusion",
            "assessment": "a",
            "decision": "d",
            "rationale": "r",
            "reviewed_by": ["anyone"],
        });
        assert!(serde_json::from_value::<ReasoningInput>(typed).is_err());
    }
}
