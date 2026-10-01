//! The consultation policy: given what is uncertain and how much it matters, whether
//! independent review is worth having, how much, and from which available advisors.
//!
//! Pure over its inputs, like `models.route`: the same assessment against the same
//! availability gets the same plan, and every advisor that was not selected says why.
//! It names capabilities and never an advisor: a repository's preference ("ask this one
//! first") is the catalogue's declaration order, which reaches this function only as the
//! order of the states it is handed.
//!
//! The budget is the number of advisors, by materiality and mode:
//!
//! | mode      | trivial | low | material | high | critical |
//! |-----------|---------|-----|----------|------|----------|
//! | ci        | 0       | 0   | 0        | 0    | 0        |
//! | fast      | 0       | 0   | 0        | 1    | 1        |
//! | standard  | 0       | 0   | 1        | 2    | 2        |
//! | offline   | 0       | 0   | 1        | 2    | 2        |
//! | strict    | 0       | 0   | 1        | 2    | 3        |
//!
//! High confidence in the local conclusion lowers it by one. Selection is greedy and
//! explainable: each next advisor is the one covering the most requested capabilities not
//! yet covered, then one from a source not yet asked (independence: two answers from one
//! vendor are one opinion twice), then declaration order. Zero selected is a plan, not a
//! failure: the plan then carries the structured local review a session performs instead.
//!
//! What the policy never does is count. There is no quorum, no threshold of agreeing
//! advisors, no majority: how advice becomes a decision is the conclusion's business,
//! and a conclusion is refused while a disagreement it covers is unresolved (see
//! [`super::store`]).
//!
//! # Example
//!
//! High uncertainty under `standard` allows two advisors. Of three, one is rate limited
//! and is excluded with its reason; the other two are selected in preference order.
//!
//! ```
//! use majordomus_cli::reasoning::availability::{AdvisorState, ModeResolution, ReasoningMode};
//! use majordomus_cli::reasoning::policy::{plan, Materiality, PlanOutcome, PlanRequest};
//!
//! fn advisor(id: &str, adapter: &str, status: &str) -> AdvisorState {
//!     serde_json::from_value(serde_json::json!({
//!         "id": id, "title": id, "transport": "api", "adapter": adapter,
//!         "capabilities": ["independent_reasoning"], "status": status,
//!         "reason": "present", "probe": "",
//!     }))
//!     .unwrap()
//! }
//! let states = [
//!     advisor("first", "x-api", "available"),
//!     advisor("limited", "y-api", "rate_limited"),
//!     advisor("second", "z-cli", "available"),
//! ];
//! let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
//! let request = PlanRequest { materiality: Materiality::High, ..Default::default() };
//! let p = plan(&request, &mode, &states);
//! assert_eq!(p.outcome, PlanOutcome::Consult);
//! let selected: Vec<&str> = p.selected.iter().map(|s| s.advisor.as_str()).collect();
//! assert_eq!(selected, ["first", "second"]);
//! assert_eq!(p.excluded[0].advisor, "limited");
//! assert!(p.excluded[0].reason.starts_with("rate_limited"));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::availability::{AdvisorState, AdvisorStatus, ModeResolution, ReasoningMode};

/// How much the uncertain decision could matter. Ordered from `trivial` to `critical`:
/// with the mode it sets the review budget, and from `material` up a decision taken alone
/// carries the structured local review.
///
/// ```
/// use majordomus_cli::reasoning::policy::Materiality;
///
/// assert_eq!(Materiality::default(), Materiality::Low);
/// assert!(Materiality::Material < Materiality::Critical);
/// let m: Materiality = serde_json::from_str("\"high\"").unwrap();
/// assert_eq!(m, Materiality::High);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Materiality {
    /// A name, a format, an obvious convention: never reviewed.
    Trivial,
    /// Local and reversible.
    #[default]
    Low,
    /// Could affect correctness, an interface, persistent data, concurrency, operations.
    Material,
    /// Architecture, security, compatibility or a public contract.
    High,
    /// Irreversible or destructive, or repository governance itself.
    Critical,
}

/// How sure the primary executor is of its current hypothesis. `high` lowers the review
/// budget by one; the other levels leave it as the mode and materiality set it.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, Materiality, PlanRequest, ReasoningConfidence};
///
/// let mode = ModeResolution { mode: ReasoningMode::Strict, source: "default".into() };
/// let budget = |confidence: ReasoningConfidence| {
///     let request = PlanRequest { materiality: Materiality::Critical, confidence, ..Default::default() };
///     plan(&request, &mode, &[]).budget
/// };
/// assert_eq!(budget(ReasoningConfidence::Medium), 3);
/// assert_eq!(budget(ReasoningConfidence::High), 2);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningConfidence {
    /// A guess the evidence does not yet support.
    Low,
    /// Supported, with open questions.
    #[default]
    Medium,
    /// Supported by evidence that would be hard to overturn.
    High,
}

/// What the policy is asked about: the materiality and confidence of one uncertainty,
/// the advisory capabilities review needs, and any standing conclusion on the subject.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, PlanOutcome, PlanRequest};
///
/// let request: PlanRequest = serde_json::from_str(
///     r#"{"materiality":"high","confidence":"low","prior_conclusion":"conclusion-1"}"#,
/// ).unwrap();
/// assert!(request.capabilities.is_empty() && !request.new_evidence);
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// // a standing conclusion and no new evidence: reuse it rather than ask again
/// assert_eq!(plan(&request, &mode, &[]).outcome, PlanOutcome::ReusePrior);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PlanRequest {
    /// Materiality of the uncertainty.
    pub materiality: Materiality,
    /// Confidence in the local hypothesis.
    pub confidence: ReasoningConfidence,
    /// The advisory capabilities review needs; empty means `independent_reasoning`.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// A standing conclusion on the same subject, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_conclusion: Option<String>,
    /// The assessment brings evidence the prior conclusion did not have.
    #[serde(default)]
    pub new_evidence: bool,
}

/// What the plan decided: consult the selected advisors, decide on local evidence, or
/// reuse a standing conclusion. Deciding locally is an ordinary outcome, not a failure.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, Materiality, PlanOutcome, PlanRequest};
///
/// let mode = ModeResolution { mode: ReasoningMode::Ci, source: "CI".into() };
/// let request = PlanRequest { materiality: Materiality::Critical, ..Default::default() };
/// let outcome: PlanOutcome = plan(&request, &mode, &[]).outcome;
/// assert_eq!(outcome, PlanOutcome::DecideLocally);
/// assert_eq!(serde_json::to_value(outcome).unwrap(), "decide_locally");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanOutcome {
    /// Consult the selected advisors, then conclude.
    Consult,
    /// Decide on local evidence; no independent review is called for or none is available.
    DecideLocally,
    /// A standing conclusion already answers this; reuse it rather than ask again.
    ReusePrior,
}

/// One selected advisor, the requested capabilities it covers, and why the greedy
/// selection chose it at its position.
///
/// ```
/// use majordomus_cli::reasoning::availability::{AdvisorState, ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, Materiality, PlanRequest, SelectedAdvisor};
///
/// fn advisor(id: &str, adapter: &str, status: &str) -> AdvisorState {
///     serde_json::from_value(serde_json::json!({
///         "id": id, "title": id, "transport": "api", "adapter": adapter,
///         "capabilities": ["independent_reasoning"], "status": status,
///         "reason": "present", "probe": "",
///     }))
///     .unwrap()
/// }
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let request = PlanRequest { materiality: Materiality::Material, ..Default::default() };
/// let p = plan(&request, &mode, &[advisor("a", "x-api", "available")]);
/// let chosen: &SelectedAdvisor = &p.selected[0];
/// assert_eq!(chosen.advisor, "a");
/// assert_eq!(chosen.covers, ["independent_reasoning"]);
/// assert!(chosen.why.starts_with("the first available advisor"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SelectedAdvisor {
    /// The advisor id.
    pub advisor: String,
    /// The requested capabilities it covers.
    pub covers: Vec<String>,
    /// Why it was chosen.
    pub why: String,
}

/// One advisor left out of the plan, its standing when the plan was made, and why: its
/// status, a capability it does not offer, or a budget already spent.
///
/// ```
/// use majordomus_cli::reasoning::availability::{AdvisorState, AdvisorStatus, ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, ExcludedAdvisor, Materiality, PlanRequest};
///
/// fn advisor(id: &str, adapter: &str, status: &str) -> AdvisorState {
///     serde_json::from_value(serde_json::json!({
///         "id": id, "title": id, "transport": "api", "adapter": adapter,
///         "capabilities": ["independent_reasoning"], "status": status,
///         "reason": "present", "probe": "",
///     }))
///     .unwrap()
/// }
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let request = PlanRequest { materiality: Materiality::Material, ..Default::default() };
/// let states = [advisor("a", "x-api", "available"), advisor("b", "y-api", "available")];
/// let p = plan(&request, &mode, &states);
/// let left: &ExcludedAdvisor = &p.excluded[0];
/// assert_eq!((left.advisor.as_str(), left.status), ("b", AdvisorStatus::Available));
/// assert!(left.reason.contains("budget was spent"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExcludedAdvisor {
    /// The advisor id.
    pub advisor: String,
    /// Its standing when the plan was made.
    pub status: AdvisorStatus,
    /// Why it is not in the plan.
    pub reason: String,
}

/// The plan: the decision with its reasons attached. It carries what it answered (mode,
/// materiality, confidence, capabilities), the budget, the outcome with one sentence of
/// why, every advisor selected or excluded, and the local review when one is due.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::{plan, Materiality, PlanRequest, ReviewPlan, LOCAL_REVIEW};
///
/// let mode = ModeResolution { mode: ReasoningMode::Offline, source: "MAJORDOMUS_REASONING_MODE".into() };
/// let request = PlanRequest { materiality: Materiality::Material, ..Default::default() };
/// let p: ReviewPlan = plan(&request, &mode, &[]);
/// assert_eq!(p.mode_source, "MAJORDOMUS_REASONING_MODE");
/// assert_eq!(p.capabilities, ["independent_reasoning"]);
/// assert_eq!((p.budget, p.available), (1, 0));
/// assert_eq!(p.local_review.len(), LOCAL_REVIEW.len());
/// assert!(p.reason.starts_with("no available advisor offers"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewPlan {
    /// The mode it was made under.
    pub mode: ReasoningMode,
    /// What decided the mode.
    pub mode_source: String,
    /// The materiality it answered.
    pub materiality: Materiality,
    /// The confidence it answered.
    pub confidence: ReasoningConfidence,
    /// The capabilities review was asked for.
    pub capabilities: Vec<String>,
    /// How many advisors the mode and materiality allow.
    pub budget: usize,
    /// What to do.
    pub outcome: PlanOutcome,
    /// Why, in one sentence.
    pub reason: String,
    /// The advisors to consult, in selection order.
    pub selected: Vec<SelectedAdvisor>,
    /// Every advisor not selected, with its reason.
    pub excluded: Vec<ExcludedAdvisor>,
    /// Advisors available when the plan was made, whatever was selected.
    pub available: usize,
    /// The structured local review to perform, when material uncertainty meets no advisor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_review: Vec<String>,
}

/// The capability review defaults to.
pub const DEFAULT_CAPABILITY: &str = "independent_reasoning";

/// The adversarial self-review a session performs when it must decide alone.
pub const LOCAL_REVIEW: &[&str] = &[
    "state the hypothesis and the strongest alternative to it",
    "list each assumption with the evidence that supports it, and mark the unsupported ones",
    "name what observation would falsify the hypothesis",
    "run the smallest experiment that discriminates between hypothesis and alternative",
    "record the conclusion with its evidence, rejected alternatives, risks and validation plan",
];

/// The budget of a mode and materiality, before confidence: how many advisors a plan may
/// select, as the table in the module header states it.
///
/// ```
/// use majordomus_cli::reasoning::availability::ReasoningMode;
/// use majordomus_cli::reasoning::policy::{budget, Materiality};
///
/// assert_eq!(budget(ReasoningMode::Ci, Materiality::Critical), 0);
/// assert_eq!(budget(ReasoningMode::Fast, Materiality::Material), 0);
/// assert_eq!(budget(ReasoningMode::Standard, Materiality::High), 2);
/// assert_eq!(budget(ReasoningMode::Strict, Materiality::Critical), 3);
/// ```
pub fn budget(mode: ReasoningMode, materiality: Materiality) -> usize {
    use Materiality::*;
    use ReasoningMode::*;
    match (mode, materiality) {
        (Ci, _) | (_, Trivial) | (_, Low) => 0,
        (Fast, Material) => 0,
        (Fast, _) => 1,
        (Standard | Offline | Strict, Material) => 1,
        (Standard | Offline, High | Critical) => 2,
        (Strict, High) => 2,
        (Strict, Critical) => 3,
    }
}

/// The source an advisor's answer comes from, for independence: its adapter family is
/// what the states carry, so two advisors through one adapter count as one source.
fn source_of(state: &AdvisorState) -> &str {
    &state.adapter
}

/// Make the plan for one request under one mode, over the advisors' states in preference
/// order. Pure: the same inputs give the same plan, and every advisor not selected is in
/// `excluded` with its reason.
///
/// ```
/// use majordomus_cli::reasoning::availability::{ModeResolution, ReasoningMode};
/// use majordomus_cli::reasoning::policy::*;
///
/// let mode = ModeResolution { mode: ReasoningMode::Standard, source: "default".into() };
/// let request = PlanRequest { materiality: Materiality::High, ..Default::default() };
/// // No advisor at all: a plan, not an error.
/// let p = plan(&request, &mode, &[]);
/// assert_eq!(p.outcome, PlanOutcome::DecideLocally);
/// assert_eq!(p.budget, 2);
/// assert!(p.selected.is_empty());
/// assert!(!p.local_review.is_empty(), "the local path is spelled out");
/// ```
pub fn plan(request: &PlanRequest, mode: &ModeResolution, states: &[AdvisorState]) -> ReviewPlan {
    let wanted: Vec<String> = if request.capabilities.is_empty() {
        vec![DEFAULT_CAPABILITY.to_string()]
    } else {
        request.capabilities.clone()
    };
    let mut allowed = budget(mode.mode, request.materiality);
    if request.confidence == ReasoningConfidence::High {
        allowed = allowed.saturating_sub(1);
    }
    let available = states
        .iter()
        .filter(|s| s.status == AdvisorStatus::Available)
        .count();
    let mut out = ReviewPlan {
        mode: mode.mode,
        mode_source: mode.source.clone(),
        materiality: request.materiality,
        confidence: request.confidence,
        capabilities: wanted.clone(),
        budget: allowed,
        outcome: PlanOutcome::DecideLocally,
        reason: String::new(),
        selected: Vec::new(),
        excluded: Vec::new(),
        available,
        local_review: Vec::new(),
    };
    let material = request.materiality >= Materiality::Material;

    if let (Some(prior), false) = (&request.prior_conclusion, request.new_evidence) {
        out.outcome = PlanOutcome::ReusePrior;
        out.budget = 0;
        out.reason = format!(
            "conclusion {prior} already answers this subject and the assessment brings no new evidence"
        );
        out.excluded = states
            .iter()
            .map(|s| ExcludedAdvisor {
                advisor: s.id.clone(),
                status: s.status,
                reason: "a standing conclusion answers the subject".into(),
            })
            .collect();
        return out;
    }

    let mut pool: Vec<&AdvisorState> = Vec::new();
    for s in states {
        if s.status != AdvisorStatus::Available {
            out.excluded.push(ExcludedAdvisor {
                advisor: s.id.clone(),
                status: s.status,
                reason: format!("{}: {}", s.status.as_str(), s.reason),
            });
        } else if !s.capabilities.iter().any(|c| wanted.contains(c)) {
            out.excluded.push(ExcludedAdvisor {
                advisor: s.id.clone(),
                status: s.status,
                reason: format!("offers none of {}", wanted.join(", ")),
            });
        } else {
            pool.push(s);
        }
    }

    let mut covered: Vec<String> = Vec::new();
    let mut sources: Vec<&str> = Vec::new();
    while out.selected.len() < allowed && !pool.is_empty() {
        let score = |s: &AdvisorState| {
            let fresh = s
                .capabilities
                .iter()
                .filter(|c| wanted.contains(c) && !covered.contains(c))
                .count();
            let independent = usize::from(!sources.contains(&source_of(s)));
            (fresh, independent)
        };
        // max by score; declaration order breaks ties (the first of equals wins)
        let mut best = 0;
        for (i, s) in pool.iter().enumerate() {
            if score(s) > score(pool[best]) {
                best = i;
            }
        }
        let chosen = pool.remove(best);
        let covers: Vec<String> = chosen
            .capabilities
            .iter()
            .filter(|c| wanted.contains(c))
            .cloned()
            .collect();
        let (fresh, independent) = score(chosen);
        let why = if out.selected.is_empty() {
            "the first available advisor in preference order covering the most requested capabilities".to_string()
        } else if fresh > 0 {
            format!("covers {fresh} requested capability(ies) the selection did not")
        } else if independent == 1 {
            "an independent source for a second opinion".to_string()
        } else {
            "the next available advisor in preference order".to_string()
        };
        for c in &covers {
            if !covered.contains(c) {
                covered.push(c.clone());
            }
        }
        sources.push(source_of(chosen));
        out.selected.push(SelectedAdvisor {
            advisor: chosen.id.clone(),
            covers,
            why,
        });
    }
    for s in pool {
        out.excluded.push(ExcludedAdvisor {
            advisor: s.id.clone(),
            status: s.status,
            reason: "the budget was spent on advisors earlier in the selection".into(),
        });
    }
    // Excluded in the states' order, so a reader finds each advisor where it is declared.
    let mut left_out = std::mem::take(&mut out.excluded);
    for s in states {
        if let Some(i) = left_out.iter().position(|e| e.advisor == s.id) {
            out.excluded.push(left_out.swap_remove(i));
        }
    }

    if !out.selected.is_empty() {
        out.outcome = PlanOutcome::Consult;
        out.reason = format!(
            "{} uncertainty under mode {}: {} independent review(s) within a budget of {allowed}",
            materiality_word(request.materiality),
            mode.mode.as_str(),
            out.selected.len()
        );
    } else {
        out.reason = if !material {
            format!(
                "{} uncertainty is decided on local evidence; review would cost more than it could change",
                materiality_word(request.materiality)
            )
        } else if allowed == 0 {
            format!(
                "mode {} with {} confidence allows no independent review for {} uncertainty",
                mode.mode.as_str(),
                confidence_word(request.confidence),
                materiality_word(request.materiality)
            )
        } else {
            format!(
                "no available advisor offers {}; decide on local evidence after a structured local review",
                wanted.join(", ")
            )
        };
        if material {
            out.local_review = LOCAL_REVIEW.iter().map(|s| s.to_string()).collect();
        }
    }
    out
}

/// The word that names a materiality, the same one its JSON form carries: what a plan's
/// reason sentence prints.
///
/// ```
/// use majordomus_cli::reasoning::policy::{materiality_word, Materiality};
///
/// assert_eq!(materiality_word(Materiality::Critical), "critical");
/// assert_eq!(serde_json::to_value(Materiality::Low).unwrap(), materiality_word(Materiality::Low));
/// ```
pub fn materiality_word(m: Materiality) -> &'static str {
    match m {
        Materiality::Trivial => "trivial",
        Materiality::Low => "low",
        Materiality::Material => "material",
        Materiality::High => "high",
        Materiality::Critical => "critical",
    }
}

fn confidence_word(c: ReasoningConfidence) -> &'static str {
    match c {
        ReasoningConfidence::Low => "low",
        ReasoningConfidence::Medium => "medium",
        ReasoningConfidence::High => "high",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::catalogue::AdvisorTransport;

    fn state(id: &str, adapter: &str, caps: &[&str], status: AdvisorStatus) -> AdvisorState {
        AdvisorState {
            id: id.into(),
            title: id.into(),
            transport: AdvisorTransport::Api,
            adapter: adapter.into(),
            capabilities: caps.iter().map(|c| c.to_string()).collect(),
            model: None,
            executable: None,
            credential: None,
            status,
            reason: "present".into(),
            probe: String::new(),
            recovering: false,
            circuit: None,
        }
    }

    fn standard() -> ModeResolution {
        ModeResolution {
            mode: ReasoningMode::Standard,
            source: "default".into(),
        }
    }

    fn request(m: Materiality, caps: &[&str]) -> PlanRequest {
        PlanRequest {
            materiality: m,
            capabilities: caps.iter().map(|c| c.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn trivial_uncertainty_never_consults() {
        let s = [state(
            "a",
            "x",
            &["independent_reasoning"],
            AdvisorStatus::Available,
        )];
        for m in [Materiality::Trivial, Materiality::Low] {
            let p = plan(&request(m, &[]), &standard(), &s);
            assert_eq!(p.outcome, PlanOutcome::DecideLocally);
            assert!(p.selected.is_empty());
            assert!(p.local_review.is_empty(), "no ceremony for trivia either");
        }
    }

    #[test]
    fn material_uncertainty_consults_the_first_preferred_advisor() {
        let s = [
            state(
                "a",
                "x",
                &["independent_reasoning"],
                AdvisorStatus::Available,
            ),
            state(
                "b",
                "y",
                &["independent_reasoning"],
                AdvisorStatus::Available,
            ),
        ];
        let p = plan(&request(Materiality::Material, &[]), &standard(), &s);
        assert_eq!(p.outcome, PlanOutcome::Consult);
        assert_eq!(p.selected.len(), 1);
        assert_eq!(
            p.selected[0].advisor, "a",
            "declaration order is the preference"
        );
        assert_eq!(p.excluded[0].advisor, "b");
    }

    #[test]
    fn an_unavailable_preference_falls_through_to_the_next_suitable_advisor() {
        let s = [
            state(
                "a",
                "x",
                &["independent_reasoning"],
                AdvisorStatus::NotConfigured,
            ),
            state("b", "y", &["code_review"], AdvisorStatus::Available),
            state(
                "c",
                "z",
                &["independent_reasoning"],
                AdvisorStatus::Available,
            ),
        ];
        let p = plan(&request(Materiality::Material, &[]), &standard(), &s);
        assert_eq!(p.selected[0].advisor, "c");
        let reasons: Vec<(&str, &str)> = p
            .excluded
            .iter()
            .map(|e| (e.advisor.as_str(), e.reason.as_str()))
            .collect();
        assert_eq!(reasons[0], ("a", "not_configured: present"));
        assert!(reasons[1].1.starts_with("offers none of"));
    }

    #[test]
    fn high_blast_radius_asks_for_complementary_and_independent_review() {
        let s = [
            state(
                "a",
                "x",
                &["independent_reasoning", "architecture_review"],
                AdvisorStatus::Available,
            ),
            state(
                "a2",
                "x",
                &["independent_reasoning", "architecture_review"],
                AdvisorStatus::Available,
            ),
            state(
                "b",
                "y",
                &["code_review", "independent_reasoning"],
                AdvisorStatus::Available,
            ),
        ];
        let p = plan(
            &request(Materiality::High, &["independent_reasoning", "code_review"]),
            &standard(),
            &s,
        );
        let chosen: Vec<&str> = p.selected.iter().map(|s| s.advisor.as_str()).collect();
        // `b` covers both requested capabilities, then `a` is an independent source; `a2`
        // shares `a`'s adapter and is left for the budget.
        assert_eq!(chosen, ["b", "a"]);
    }

    #[test]
    fn zero_advisors_is_a_plan_with_the_local_path_spelled_out() {
        let p = plan(&request(Materiality::Critical, &[]), &standard(), &[]);
        assert_eq!(p.outcome, PlanOutcome::DecideLocally);
        assert_eq!(p.available, 0);
        assert_eq!(p.local_review.len(), LOCAL_REVIEW.len());
        assert!(p.reason.contains("no available advisor"));
    }

    #[test]
    fn ci_mode_allows_nothing_and_says_so() {
        let ci = ModeResolution {
            mode: ReasoningMode::Ci,
            source: "CI".into(),
        };
        let p = plan(&request(Materiality::Critical, &[]), &ci, &[]);
        assert_eq!(p.budget, 0);
        assert!(p.reason.contains("mode ci"));
    }

    #[test]
    fn a_standing_conclusion_is_reused_unless_new_evidence_arrives() {
        let s = [state(
            "a",
            "x",
            &["independent_reasoning"],
            AdvisorStatus::Available,
        )];
        let mut r = request(Materiality::High, &[]);
        r.prior_conclusion = Some("conclusion-1".into());
        let p = plan(&r, &standard(), &s);
        assert_eq!(p.outcome, PlanOutcome::ReusePrior);
        assert!(p.selected.is_empty());
        r.new_evidence = true;
        assert_eq!(plan(&r, &standard(), &s).outcome, PlanOutcome::Consult);
    }

    #[test]
    fn the_budget_table_is_the_documented_one() {
        use Materiality::*;
        let rows = [
            (ReasoningMode::Ci, [0, 0, 0, 0, 0]),
            (ReasoningMode::Fast, [0, 0, 0, 1, 1]),
            (ReasoningMode::Standard, [0, 0, 1, 2, 2]),
            (ReasoningMode::Offline, [0, 0, 1, 2, 2]),
            (ReasoningMode::Strict, [0, 0, 1, 2, 3]),
        ];
        for (mode, want) in rows {
            let got: Vec<usize> = [Trivial, Low, Material, High, Critical]
                .iter()
                .map(|m| budget(mode, *m))
                .collect();
            assert_eq!(got, want, "{mode:?}");
        }
    }
}
