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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::availability::AdvisorState;
use super::policy::{Materiality, ReasoningConfidence, ReviewPlan};

/// The schema name every stored record carries.
pub const RECORD_SCHEMA: &str = "majordomus/reasoning-record";
/// The record format version.
pub const RECORD_VERSION: u32 = 1;

/// What a piece of evidence is.
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanInput {
    /// The assessment the plan answers.
    pub assessment: String,
}

/// A plan as stored: the decision and the availability it was made against.
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

/// How a consultation ended.
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
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Timeout | Self::Unavailable | Self::Error)
    }

    /// The word.
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

/// One normalised advisor exchange.
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

/// One side of a disagreement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisagreementPosition {
    /// Whose position: `primary`, or a completed consultation's record id.
    pub source: String,
    /// The position, in one line.
    pub position: String,
}

/// Advisors, or an advisor and the executor, disagree.
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

/// Whether a validation passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOutcome {
    /// It passed.
    Pass,
    /// It failed.
    Fail,
}

/// A step of a conclusion's validation plan, performed.
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

/// A stored record's body.
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

/// A stored record.
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
