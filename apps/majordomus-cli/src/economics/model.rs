//! The typed records of the economics subsystem: the declarations it reads (methodology,
//! suites, tasks), the raw facts it records (runs), and the answer it derives (summary).
//!
//! Raw facts and derived answers are different types on purpose. A run holds what the
//! provider reported and what the success gates found, and nothing computed from them; a
//! pair, a metric and a summary are recomputed from runs on every read, so a change to how
//! a number is derived never needs the runs rewritten.
//!
//! Every public type is prefixed `Economics`: the OpenAPI document keeps one flat namespace
//! of schema names, and `Token*` and `Evidence*` already belong to other subsystems.
//!
//! ```
//! use majordomus_cli::economics::model::EconomicsClass;
//! assert!(EconomicsClass::Observed.is_measurement());
//! assert!(!EconomicsClass::Counterfactual.is_measurement());
//! assert_eq!(EconomicsClass::Derived.weakest(EconomicsClass::Estimated), EconomicsClass::Estimated);
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- measurement classes

/// How a number came to be. The classes are ordered from strongest to weakest, and a value
/// computed from several inputs is never stronger than the weakest of them.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsClass;
/// assert_eq!(serde_json::to_value(EconomicsClass::Counterfactual).unwrap(), "counterfactual");
/// let parsed: EconomicsClass = serde_json::from_value("observed".into()).unwrap();
/// assert!(parsed < EconomicsClass::Estimated, "a stronger class sorts first");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EconomicsClass {
    /// Returned by the provider or its harness at run time and recorded unmodified.
    Observed,
    /// Computed deterministically from bytes on disk with a named, pinned tokenizer.
    Counted,
    /// Arithmetic over observed or counted values.
    Derived,
    /// A number standing in for one nobody measured. Never compared, never published.
    Estimated,
    /// What a mechanism would have avoided, modelled rather than observed.
    Counterfactual,
}

impl EconomicsClass {
    /// The weaker of two classes: what a value computed from both may call itself.
    ///
    /// ```
    /// use majordomus_cli::economics::model::EconomicsClass::*;
    /// assert_eq!(Observed.weakest(Counted), Counted);
    /// assert_eq!(Counterfactual.weakest(Observed), Counterfactual);
    /// ```
    pub fn weakest(self, other: EconomicsClass) -> EconomicsClass {
        self.max(other)
    }

    /// Whether a value of this class is a measurement a comparison may rest on:
    /// observed, counted, or derived from those. Estimates and counterfactuals are not.
    ///
    /// ```
    /// use majordomus_cli::economics::model::EconomicsClass;
    /// assert!(EconomicsClass::Counted.is_measurement());
    /// assert!(!EconomicsClass::Estimated.is_measurement());
    /// ```
    pub fn is_measurement(self) -> bool {
        matches!(self, EconomicsClass::Observed | EconomicsClass::Counted | EconomicsClass::Derived)
    }

    /// The word a surface prints for this class.
    ///
    /// ```
    /// assert_eq!(majordomus_cli::economics::model::EconomicsClass::Counted.word(), "counted");
    /// ```
    pub fn word(self) -> &'static str {
        match self {
            EconomicsClass::Observed => "observed",
            EconomicsClass::Counted => "counted",
            EconomicsClass::Derived => "derived",
            EconomicsClass::Estimated => "estimated",
            EconomicsClass::Counterfactual => "counterfactual",
        }
    }
}

// ---------------------------------------------------------------- declarations

/// One measurement class as the methodology defines it.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsClass, EconomicsClassDefinition};
/// let d: EconomicsClassDefinition = serde_json::from_value(serde_json::json!({
///     "id": "counted",
///     "meaning": "Computed deterministically from bytes on disk with a pinned tokenizer."
/// }))
/// .unwrap();
/// assert_eq!(d.id, EconomicsClass::Counted);
/// let unknown = serde_json::json!({ "id": "guessed", "meaning": "A class nobody declared." });
/// assert!(serde_json::from_value::<EconomicsClassDefinition>(unknown).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsClassDefinition {
    /// The class.
    pub id: EconomicsClass,
    /// What it means, in the methodology's words.
    pub meaning: String,
}

/// One arm of the experiment: what the session runs with, and what it runs without.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsVariant;
/// let v: EconomicsVariant = serde_json::from_value(serde_json::json!({
///     "id": "baseline", "role": "control", "title": "Without Majordomus",
///     "description": "The fixture repository as a competent team keeps it."
/// }))
/// .unwrap();
/// assert_eq!(v.role, "control");
/// assert!(v.excludes.is_empty(), "an arm that names no exclusion excludes nothing");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsVariant {
    /// `baseline`, `majordomus`, ...
    pub id: String,
    /// `control` or `treatment`.
    pub role: String,
    /// A short name.
    pub title: String,
    /// Exactly what the arm consists of.
    pub description: String,
    /// What the arm deliberately does not include.
    #[serde(default)]
    pub excludes: Vec<String>,
}

/// One gate a run must pass to count as a success.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsGateDefinition;
/// let g: EconomicsGateDefinition = serde_json::from_value(serde_json::json!({
///     "id": "tests_kept", "meaning": "No test file the fixture started with was deleted."
/// }))
/// .unwrap();
/// assert_eq!(g.id, "tests_kept");
/// let unexplained = serde_json::json!({ "id": "visible_tests" });
/// assert!(serde_json::from_value::<EconomicsGateDefinition>(unexplained).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsGateDefinition {
    /// `visible_tests`, `acceptance_tests`, `tests_kept`.
    pub id: String,
    /// What passing it means.
    pub meaning: String,
}

/// How runs are paired and when a pair is comparable and valid.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsPairing;
/// let p: EconomicsPairing = serde_json::from_value(serde_json::json!({
///     "key": ["suite", "task", "repetition"],
///     "comparable": ["methodology version", "requested model", "fixture digest"],
///     "valid": "Both runs succeeded and both report provider usage."
/// }))
/// .unwrap();
/// assert!(p.key.iter().any(|k| k == "repetition"), "a repetition pairs with its own number");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsPairing {
    /// The fields two runs share to be one pair.
    pub key: Vec<String>,
    /// The fields two runs must agree on to be compared at all.
    pub comparable: Vec<String>,
    /// When a comparable pair is valid.
    pub valid: String,
}

/// How per-pair results are aggregated into a headline value and its interval. Every
/// parameter of the bootstrap is declared here, its generator seed included, so the same
/// runs always give the same interval.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsStatisticsPolicy;
/// let s: EconomicsStatisticsPolicy = serde_json::from_value(serde_json::json!({
///     "per_pair": "1 - treatment / control on total tokens", "location": "median",
///     "interval": "bootstrap percentile", "confidence_bp": 9500, "resamples": 10000,
///     "seed": 20260924, "min_pairs_for_interval": 5
/// }))
/// .unwrap();
/// assert_eq!(f64::from(s.confidence_bp) / 100.0, 95.0, "basis points, not percent");
/// assert_eq!(serde_json::to_value(&s).unwrap()["seed"], 20260924);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsStatisticsPolicy {
    /// The per-pair formula, in words.
    pub per_pair: String,
    /// The location statistic reported as the headline.
    pub location: String,
    /// How the interval is computed.
    pub interval: String,
    /// Confidence level in basis points: 9500 is 95 %.
    pub confidence_bp: u32,
    /// Bootstrap resamples.
    pub resamples: u32,
    /// The seed of the bootstrap's generator, so that the interval is reproducible.
    pub seed: u64,
    /// Below this many valid pairs no interval is computed at all.
    pub min_pairs_for_interval: usize,
}

/// When a quantitative total-token claim may be published. Every threshold must hold.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsPublicationPolicy;
/// let rule = serde_json::json!({
///     "min_valid_pairs": 30, "min_categories": 4, "min_pairs_per_category": 5,
///     "min_repetitions": 3, "min_valid_pair_rate_bp": 8000, "max_interval_width_bp": 2000,
///     "require_current": true
/// });
/// let p: EconomicsPublicationPolicy = serde_json::from_value(rule.clone()).unwrap();
/// assert!(p.require_current && p.min_valid_pairs == 30);
/// let mut lax = rule;
/// lax.as_object_mut().unwrap().remove("require_current");
/// assert!(serde_json::from_value::<EconomicsPublicationPolicy>(lax).is_err(), "no default");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsPublicationPolicy {
    /// Valid matched pairs, at least.
    pub min_valid_pairs: usize,
    /// Task categories with enough pairs, at least.
    pub min_categories: usize,
    /// Pairs a category needs to count towards `min_categories`.
    pub min_pairs_per_category: usize,
    /// Repetitions of every task and variant, at least.
    pub min_repetitions: u32,
    /// Valid pairs over attempted pairs, in basis points, at least.
    pub min_valid_pair_rate_bp: u32,
    /// Width of the interval, in basis points of reduction, at most.
    pub max_interval_width_bp: u32,
    /// Whether the evidence must be current (its mechanism unchanged since it was recorded).
    pub require_current: bool,
}

/// The rule for excluding runs, and the runs it excluded.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsOutlierPolicy;
/// let o: EconomicsOutlierPolicy =
///     serde_json::from_value(serde_json::json!({ "rule": "No run is excluded." })).unwrap();
/// assert!(o.excluded.is_empty(), "a rule with no list excludes nothing");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsOutlierPolicy {
    /// The rule, stated before any analysis.
    pub rule: String,
    /// Every excluded run, with its reason.
    #[serde(default)]
    pub excluded: Vec<EconomicsExclusion>,
}

/// One run excluded under the outlier rule.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsExclusion, EconomicsOutlierPolicy};
/// let o: EconomicsOutlierPolicy = serde_json::from_value(serde_json::json!({
///     "rule": "A run is excluded only by naming it here with a reason.",
///     "excluded": [{ "run": "vat-rounding--baseline--r2", "reason": "provider outage" }]
/// }))
/// .unwrap();
/// let first: &EconomicsExclusion = &o.excluded[0];
/// assert_eq!(first.run, "vat-rounding--baseline--r2");
/// let unexplained = serde_json::json!({ "run": "vat-rounding--baseline--r1" });
/// assert!(serde_json::from_value::<EconomicsExclusion>(unexplained).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsExclusion {
    /// The run's id.
    pub run: String,
    /// Why.
    pub reason: String,
}

/// A statement the benchmark exists to test. Never a result.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsHypothesis;
/// let h: EconomicsHypothesis = serde_json::from_value(serde_json::json!({
///     "id": "continuity",
///     "statement": "Work that spans sessions gains the most.",
///     "status": "untested"
/// }))
/// .unwrap();
/// assert_eq!(h.status, "untested");
/// assert_eq!(serde_json::to_value(&h).unwrap()["id"], "continuity");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsHypothesis {
    /// A slug.
    pub id: String,
    /// What is hypothesised.
    pub statement: String,
    /// `untested`, `supported`, `refuted` — set by a person reading the evidence.
    pub status: String,
}

/// A published claim bound to the metric that must support it.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsClaimBinding;
/// let b: EconomicsClaimBinding = serde_json::from_value(serde_json::json!({
///     "claim": "token-savings-measured",
///     "metric": "effective_token_reduction",
///     "requires": "verified"
/// }))
/// .unwrap();
/// assert_eq!((b.metric.as_str(), b.requires.as_str()), ("effective_token_reduction", "verified"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsClaimBinding {
    /// The claim's id in `docs/CLAIMS.yaml`.
    pub claim: String,
    /// The metric's id.
    pub metric: String,
    /// `verified` (sampled evidence meeting the publication rule) or `measured`
    /// (deterministic evidence, current).
    pub requires: String,
}

/// One entry of the methodology's change log.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsMethodologyChange;
/// let first = serde_json::json!({ "version": 1, "why": "The first methodology." });
/// let c: EconomicsMethodologyChange = serde_json::from_value(first).unwrap();
/// assert_eq!(c.version, 1);
/// let silent = serde_json::json!({ "version": 2 });
/// let refused = serde_json::from_value::<EconomicsMethodologyChange>(silent);
/// assert!(refused.is_err(), "a change says why");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsMethodologyChange {
    /// The version it introduced.
    pub version: u32,
    /// Why.
    pub why: String,
}

/// `.ai/repo/benchmarks/economics/methodology.yaml`: what is measured, how, and when a
/// result may be published.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsMethodology;
/// let m: EconomicsMethodology = serde_json::from_value(serde_json::json!({
///     "schema": "economics-methodology/v1", "version": 1, "title": "Token economics",
///     "question": "How many tokens does a session consume with and without Majordomus?",
///     "unit": "tokens per completed task", "primary_metric": "effective_token_reduction",
///     "classes": [], "variants": [], "success": [],
///     "pairing": { "key": ["task"], "comparable": [], "valid": "both succeeded" },
///     "statistics": { "per_pair": "1 - t / c", "location": "median", "interval": "bootstrap",
///         "confidence_bp": 9500, "resamples": 1000, "seed": 1, "min_pairs_for_interval": 5 },
///     "publication": { "min_valid_pairs": 30, "min_categories": 4, "min_pairs_per_category": 5,
///         "min_repetitions": 3, "min_valid_pair_rate_bp": 8000, "max_interval_width_bp": 2000,
///         "require_current": true },
///     "outliers": { "rule": "No run is excluded." }
/// }))
/// .unwrap();
/// assert_eq!(m.statistics.confidence_bp, 9500);
/// assert!(m.hypotheses.is_empty() && m.claims.is_empty() && m.changes.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsMethodology {
    /// `economics-methodology/v1`.
    pub schema: String,
    /// The methodology version. Evidence under another version is never pooled.
    pub version: u32,
    /// A name.
    pub title: String,
    /// The question the benchmark answers.
    pub question: String,
    /// The unit results are stated in.
    pub unit: String,
    /// The metric a headline is about.
    pub primary_metric: String,
    /// The measurement classes.
    pub classes: Vec<EconomicsClassDefinition>,
    /// The arms of the experiment.
    pub variants: Vec<EconomicsVariant>,
    /// The success gates.
    pub success: Vec<EconomicsGateDefinition>,
    /// Pairing.
    pub pairing: EconomicsPairing,
    /// Aggregation.
    pub statistics: EconomicsStatisticsPolicy,
    /// The publication threshold.
    pub publication: EconomicsPublicationPolicy,
    /// Outliers.
    pub outliers: EconomicsOutlierPolicy,
    /// What the benchmark tests.
    #[serde(default)]
    pub hypotheses: Vec<EconomicsHypothesis>,
    /// The claims of `docs/CLAIMS.yaml` that rest on a metric, and the standing the metric
    /// must have before the claim may be `guaranteed`.
    #[serde(default)]
    pub claims: Vec<EconomicsClaimBinding>,
    /// The change log.
    #[serde(default)]
    pub changes: Vec<EconomicsMethodologyChange>,
}

/// A suite, as `.ai/repo/benchmarks/economics/suites/<id>.yaml` declares it: what is run,
/// with which model and arms, and which tracked paths decide whether its evidence is still
/// current. A `live` suite fills the session fields and a `context` suite the seed and
/// tokenizer fields; the other kind's fields stay absent and are not written back.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsSuite;
/// let s: EconomicsSuite = serde_json::from_value(serde_json::json!({
///     "schema": "economics-suite/v1", "id": "context", "version": 1, "kind": "context",
///     "title": "Context selection, counted", "tokenizer": "o200k_base",
///     "freshness_inputs": ["apps/majordomus-cli/src/devcontext"]
/// }))
/// .unwrap();
/// assert!(s.model.is_none() && s.tasks.is_empty(), "a context suite runs no model");
/// let back = serde_json::to_value(&s).unwrap();
/// assert!(back.get("model").is_none(), "an absent live field is not written as null");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsSuite {
    /// `economics-suite/v1`.
    pub schema: String,
    /// The suite's id.
    pub id: String,
    /// The suite's own version.
    pub version: u32,
    /// `live` (model sessions) or `context` (deterministic, no model).
    pub kind: String,
    /// A name.
    pub title: String,
    /// The provider, for a live suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The harness that runs sessions, for a live suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// The model requested, for a live suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Repetitions of every task and variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetitions: Option<u32>,
    /// The control variant's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// The treatment variant's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment: Option<String>,
    /// The spending cap the harness is given per session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_budget_usd_per_session: Option<u32>,
    /// The task ids, for a live suite.
    #[serde(default)]
    pub tasks: Vec<String>,
    /// Where the seeds come from, for a context suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeds: Option<String>,
    /// The tokenizer, for a context suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    /// The tracked paths whose content decides whether recorded evidence is still current.
    pub freshness_inputs: Vec<String>,
}

/// One session of a task: the prompt file it starts from.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsTaskSession;
/// let s: EconomicsTaskSession =
///     serde_json::from_value(serde_json::json!({ "prompt": "prompt-1.md" })).unwrap();
/// assert_eq!(s.prompt, "prompt-1.md");
/// assert!(serde_json::from_value::<EconomicsTaskSession>(serde_json::json!({})).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsTaskSession {
    /// The prompt file, relative to the task's directory.
    pub prompt: String,
}

/// A task, as `.ai/repo/benchmarks/economics/tasks/<id>/task.yaml` declares it: the fixture
/// a run starts from, the prompt of each session in order, and the hidden acceptance tests
/// and verify command that decide whether the run succeeded.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsTask;
/// let t: EconomicsTask = serde_json::from_value(serde_json::json!({
///     "schema": "economics-task/v1", "id": "recurring-invoices", "title": "Recurring invoices",
///     "category": "multi-session", "complexity": "medium", "source": "Work across sessions.",
///     "fixture": "test/fixtures/economics/ledgerlite",
///     "acceptance": "test/fixtures/economics/acceptance/recurring-invoices",
///     "reference": "test/fixtures/economics/reference/recurring-invoices.patch",
///     "verify": "python3 -m unittest discover -s tests -q", "scope": ["ledgerlite", "tests"],
///     "sessions": [{ "prompt": "prompt-1.md" }, { "prompt": "prompt-2.md" }]
/// }))
/// .unwrap();
/// assert_eq!(t.sessions.len(), 2);
/// assert!(t.setup.is_none(), "a task with no setup patch starts from the bare fixture");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsTask {
    /// `economics-task/v1`.
    pub schema: String,
    /// The task's id.
    pub id: String,
    /// A name.
    pub title: String,
    /// The category results are segmented by.
    pub category: String,
    /// `small`, `medium`, `large`.
    pub complexity: String,
    /// The development pattern it stands for.
    pub source: String,
    /// The fixture repository the task starts from.
    pub fixture: String,
    /// A patch applied to the fixture before the first session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// The hidden acceptance tests, copied in after the last session.
    pub acceptance: String,
    /// A reference solution, which proves the acceptance tests can be satisfied.
    pub reference: String,
    /// The command whose exit decides the test gates.
    pub verify: String,
    /// Where the task is expected to change files.
    pub scope: Vec<String>,
    /// The sessions, in order.
    pub sessions: Vec<EconomicsTaskSession>,
}

// ---------------------------------------------------------------- raw facts

/// The harness that ran a session, as it identified itself.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsHarness;
/// let a = EconomicsHarness { name: "claude-code".into(), version: "2.0.1".into() };
/// let b = EconomicsHarness { version: "2.0.2".into(), ..a.clone() };
/// assert_ne!(a, b, "another version of the harness is another harness");
/// assert_eq!(serde_json::to_value(&a).unwrap()["name"], "claude-code");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsHarness {
    /// `claude-code`.
    pub name: String,
    /// Its version, as `--version` printed it.
    pub version: String,
}

/// The token usage of one provider request, provider-native field names kept.
///
/// A request is one model call. Claude Code writes one event per content block of a
/// message and repeats the message's usage on each, so a request is identified by its
/// message id and counted once.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRequest;
/// let r: EconomicsRequest = serde_json::from_value(serde_json::json!({
///     "model": "claude-sonnet-5", "input_tokens": 12, "cache_creation_input_tokens": 3000,
///     "cache_read_input_tokens": 18000, "output_tokens": 400
/// }))
/// .unwrap();
/// assert_eq!(r.input_tokens + r.cache_creation_input_tokens + r.cache_read_input_tokens, 21012);
/// assert!(serde_json::to_value(&r).unwrap().get("thinking_tokens").is_none(), "unreported");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsRequest {
    /// The model that answered.
    pub model: String,
    /// `input_tokens`: input not read from or written to the cache.
    pub input_tokens: u64,
    /// `cache_creation_input_tokens`: input written to the cache.
    pub cache_creation_input_tokens: u64,
    /// `cache_read_input_tokens`: input read from the cache.
    pub cache_read_input_tokens: u64,
    /// `output_tokens`, reasoning included.
    pub output_tokens: u64,
    /// `output_tokens_details.thinking_tokens`, when the provider reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u64>,
}

/// A model's usage over a whole session, as the harness totalled it (`modelUsage`),
/// including models the harness called on its own account.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsModelTotal;
/// let t: EconomicsModelTotal = serde_json::from_value(serde_json::json!({
///     "model": "claude-haiku-5", "input_tokens": 900, "cache_creation_input_tokens": 0,
///     "cache_read_input_tokens": 0, "output_tokens": 60, "cost_microusd": 1250
/// }))
/// .unwrap();
/// assert_eq!(t.cost_microusd, Some(1250), "0.00125 USD, kept as an integer");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsModelTotal {
    /// The model.
    pub model: String,
    /// `inputTokens`.
    pub input_tokens: u64,
    /// `cacheCreationInputTokens`.
    pub cache_creation_input_tokens: u64,
    /// `cacheReadInputTokens`.
    pub cache_read_input_tokens: u64,
    /// `outputTokens`.
    pub output_tokens: u64,
    /// `costUSD`, in millionths of a dollar: the harness's own projection from its price
    /// table at run time, not a bill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
}

/// What a session did before its first edit: the cost of getting oriented.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsOrientation;
/// let none = EconomicsOrientation::default();
/// assert!(!none.edited && none.requests == 0 && none.tokens == 0);
/// let o = EconomicsOrientation {
///     edited: true, requests: 4, tool_calls: 6, reads: 4, searches: 1, tokens: 52_000,
/// };
/// assert!(o.reads + o.searches <= o.tool_calls, "reads and searches are tool calls");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsOrientation {
    /// Whether the session edited anything at all.
    pub edited: bool,
    /// Provider requests before the first edit.
    pub requests: u64,
    /// Tool calls before the first edit.
    pub tool_calls: u64,
    /// File reads before the first edit.
    pub reads: u64,
    /// Searches (grep, glob, find) before the first edit.
    pub searches: u64,
    /// Total tokens of the requests before the first edit.
    pub tokens: u64,
}

/// One session of a run: one harness invocation on one prompt, with every provider request
/// it made, the harness's own totals and the tools it called. A count the harness did not
/// report is absent rather than zero.
///
/// ```
/// use std::collections::BTreeMap;
/// use majordomus_cli::economics::model::{EconomicsOrientation, EconomicsSession};
/// let s = EconomicsSession {
///     index: 1, prompt_sha256: "0".repeat(64), duration_ms: 42_000, turns: None,
///     ended: "success".into(), is_error: false, requests: vec![], models: vec![],
///     reported_cost_microusd: None, tools: BTreeMap::from([("Read".into(), 3)]),
///     orientation: EconomicsOrientation::default(),
/// };
/// let json = serde_json::to_value(&s).unwrap();
/// assert!(json.get("turns").is_none(), "an unreported count is absent, not zero");
/// assert_eq!(json["tools"]["Read"], 3);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsSession {
    /// 1-based.
    pub index: u32,
    /// SHA-256 of the prompt text. The text is in the task's prompt file, not here.
    pub prompt_sha256: String,
    /// Wall-clock milliseconds.
    pub duration_ms: u64,
    /// Turns, as the harness counted them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<u64>,
    /// How the session ended, as the harness reported it (`success`, `error_max_budget_usd`, ...).
    pub ended: String,
    /// Whether the harness reported an error.
    pub is_error: bool,
    /// Every request, deduplicated by message id, in order.
    pub requests: Vec<EconomicsRequest>,
    /// The harness's per-model totals.
    pub models: Vec<EconomicsModelTotal>,
    /// The harness's `total_cost_usd`, in millionths of a dollar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_cost_microusd: Option<u64>,
    /// Tool calls by tool name.
    pub tools: BTreeMap<String, u64>,
    /// The session's orientation cost.
    pub orientation: EconomicsOrientation,
}

/// One success gate's verdict on a run.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsCheck;
/// let c = EconomicsCheck { id: "tests_kept".into(), passed: true, detail: "0 deleted".into() };
/// let back: EconomicsCheck = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
/// assert_eq!(back, c);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsCheck {
    /// The gate's id.
    pub id: String,
    /// Whether it passed.
    pub passed: bool,
    /// What was found.
    pub detail: String,
}

/// Whether the run did the task.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsCheck, EconomicsOutcome};
/// let gate = |id: &str, passed| EconomicsCheck { id: id.into(), passed, detail: String::new() };
/// let o = EconomicsOutcome {
///     completed: false,
///     checks: vec![gate("visible_tests", true), gate("acceptance_tests", false)],
///     changed_files: vec!["ledgerlite/vat.py".into()],
/// };
/// assert_eq!(o.completed, o.checks.iter().all(|c| c.passed), "completed: every gate passed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsOutcome {
    /// Every gate passed.
    pub completed: bool,
    /// Each gate's verdict.
    pub checks: Vec<EconomicsCheck>,
    /// Files the sessions changed, added or deleted.
    pub changed_files: Vec<String>,
}

/// The repository revision a run was recorded at.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRevision;
/// let r: EconomicsRevision =
///     serde_json::from_value(serde_json::json!({ "commit": "8d37d011ef", "dirty": true }))
///         .unwrap();
/// assert!(r.dirty, "a run recorded over uncommitted changes says so");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsRevision {
    /// The commit.
    pub commit: String,
    /// Whether the working tree differed from it.
    pub dirty: bool,
}

/// One recorded run: a task, one variant, one repetition, every session of it. Raw facts
/// only; nothing here is derived from another field.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRun;
/// let run: EconomicsRun = serde_json::from_value(serde_json::json!({
///     "schema": "economics-run/v1", "id": "vat-rounding--baseline--r1", "suite": "pilot",
///     "suite_version": 1, "methodology": 1, "task": "vat-rounding", "variant": "baseline",
///     "repetition": 1, "provider": "anthropic",
///     "harness": { "name": "claude-code", "version": "2.0.1" },
///     "model_requested": "claude-sonnet-5", "models_reported": ["claude-sonnet-5"],
///     "repository": { "commit": "8d37d011ef", "dirty": false }, "majordomus_version": "0.7.0",
///     "fixture_digest": "f0", "inputs_digest": "i0", "configuration_digest": "c0",
///     "started_at": "2026-09-24T10:00:00Z", "finished_at": "2026-09-24T10:05:00Z",
///     "sessions": [], "outcome": { "completed": false, "checks": [], "changed_files": [] }
/// }))
/// .unwrap();
/// assert_eq!(run.id, format!("{}--{}--r{}", run.task, run.variant, run.repetition));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsRun {
    /// `economics-run/v1`.
    pub schema: String,
    /// `<task>--<variant>--r<repetition>`.
    pub id: String,
    /// The suite.
    pub suite: String,
    /// The suite's version.
    pub suite_version: u32,
    /// The methodology version.
    pub methodology: u32,
    /// The task.
    pub task: String,
    /// The variant.
    pub variant: String,
    /// 1-based.
    pub repetition: u32,
    /// The provider.
    pub provider: String,
    /// The harness.
    pub harness: EconomicsHarness,
    /// The model the harness was asked for.
    pub model_requested: String,
    /// The models the harness reported using, sorted.
    pub models_reported: Vec<String>,
    /// The repository revision.
    pub repository: EconomicsRevision,
    /// The Majordomus version the treatment was installed from.
    pub majordomus_version: String,
    /// Digest of the fixture, setup patch and task declaration the run started from.
    pub fixture_digest: String,
    /// Digest of the suite's freshness inputs when the run was recorded.
    pub inputs_digest: String,
    /// Digest of the harness invocation (flags, settings), the prompt excepted.
    pub configuration_digest: String,
    /// When the first session started, UTC.
    pub started_at: String,
    /// When the verification finished, UTC.
    pub finished_at: String,
    /// The sessions.
    pub sessions: Vec<EconomicsSession>,
    /// The success gates' verdict.
    pub outcome: EconomicsOutcome,
}

/// Token counts of one context-compilation seed.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsContextSeed;
/// let s: EconomicsContextSeed = serde_json::from_value(serde_json::json!({
///     "seed": "I0042", "candidates": 12, "candidate_bytes": 44000, "candidate_tokens": 11000,
///     "considered": 90, "considered_tokens": 160000, "selected": 9, "selected_bytes": 32800,
///     "selected_tokens": 8200, "estimated_selected_tokens": 8200,
///     "excluded_tokens": { "budget": 2800 }, "unresolved": 0, "over_budget": false
/// }))
/// .unwrap();
/// assert_eq!(s.selected_tokens + s.excluded_tokens["budget"], s.candidate_tokens);
/// assert_eq!(s.estimated_selected_tokens, s.selected_bytes / 4, "the compiler's own estimate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsContextSeed {
    /// The work the compiler was asked about (an issue id).
    pub seed: String,
    /// Distinct files the compiler judged relevant to the work: what it selected, and what
    /// it left out only because the budget ran out. The denominator of context reduction.
    pub candidates: u64,
    /// Their bytes.
    pub candidate_bytes: u64,
    /// Their tokens, counted.
    pub candidate_tokens: u64,
    /// Distinct files the compiler reached at all, relevant or not.
    pub considered: u64,
    /// Their tokens, counted. Never a denominator: most of it the compiler judged irrelevant.
    pub considered_tokens: u64,
    /// Distinct files it selected.
    pub selected: u64,
    /// Their bytes.
    pub selected_bytes: u64,
    /// Their tokens, counted.
    pub selected_tokens: u64,
    /// The compiler's own cost estimate of what it selected (bytes over four).
    pub estimated_selected_tokens: u64,
    /// Counted tokens it left out, by the reason it gave.
    pub excluded_tokens: BTreeMap<String, u64>,
    /// Entries naming nothing on disk, which could not be counted.
    pub unresolved: u64,
    /// Whether the compiler reported its selection over budget.
    pub over_budget: bool,
}

/// The identity of a tokenizer, recorded with every count it produced.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsTokenizer;
/// let t: EconomicsTokenizer = serde_json::from_value(serde_json::json!({
///     "encoding": "o200k_base", "implementation": "tiktoken-rs 0.12.0"
/// }))
/// .unwrap();
/// let other = EconomicsTokenizer { implementation: "tiktoken-rs 0.13.0".into(), ..t.clone() };
/// assert_ne!(t, other, "the same encoding from another implementation is another tokenizer");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsTokenizer {
    /// The encoding: `o200k_base`.
    pub encoding: String,
    /// The implementation and its pinned version.
    pub implementation: String,
}

/// One recorded measurement of the context suite: every seed the compiler compiled, counted,
/// and every seed it refused, named with its reason rather than dropped.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsContextRun;
/// let run: EconomicsContextRun = serde_json::from_value(serde_json::json!({
///     "schema": "economics-context-run/v1", "suite": "context", "suite_version": 1,
///     "methodology": 1, "repository": { "commit": "8d37d011ef", "dirty": false },
///     "majordomus_version": "0.7.0",
///     "tokenizer": { "encoding": "o200k_base", "implementation": "tiktoken-rs 0.12.0" },
///     "inputs_digest": "i0", "budget_tokens": 8000, "measured_at": "2026-09-24T10:00:00Z",
///     "seeds": []
/// }))
/// .unwrap();
/// assert_eq!(run.tokenizer.encoding, "o200k_base");
/// assert!(run.seeds.is_empty());
/// assert!(run.refused.is_empty(), "a record from before refusals were kept reads as none");
/// let json = serde_json::to_value(&run).unwrap();
/// assert_eq!(json["refused"], serde_json::json!([]), "a new record says none explicitly");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsContextRun {
    /// `economics-context-run/v1`.
    pub schema: String,
    /// The suite.
    pub suite: String,
    /// The suite's version.
    pub suite_version: u32,
    /// The methodology version.
    pub methodology: u32,
    /// The repository revision measured.
    pub repository: EconomicsRevision,
    /// The Majordomus version that compiled the contexts.
    pub majordomus_version: String,
    /// The tokenizer.
    pub tokenizer: EconomicsTokenizer,
    /// Digest of the suite's freshness inputs when measured.
    pub inputs_digest: String,
    /// The compiler's budget, in its own units.
    pub budget_tokens: u64,
    /// When measured, UTC.
    pub measured_at: String,
    /// One entry per seed, sorted by seed.
    pub seeds: Vec<EconomicsContextSeed>,
    /// The seeds the compiler refused, each with the reason it gave (`I0042: no such
    /// issue`). They were not measured, so they are in no count above; they are kept here
    /// so that a ratio over the seeds that compiled is never read as one over every seed.
    /// A record written before this field existed reads as an empty list.
    #[serde(default)]
    pub refused: Vec<String>,
}

// ---------------------------------------------------------------- derived answers

/// How recorded evidence stands against the repository as it is now.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsFreshness;
/// assert_eq!(serde_json::to_value(EconomicsFreshness::NoEvidence).unwrap(), "no_evidence");
/// let f: EconomicsFreshness = serde_json::from_value("incompatible".into()).unwrap();
/// assert_eq!(f, EconomicsFreshness::Incompatible);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EconomicsFreshness {
    /// Recorded under this methodology, and nothing it depends on changed since.
    Current,
    /// Recorded under this methodology, but a mechanism it measured changed since.
    Stale,
    /// Recorded under another methodology version: not comparable at all.
    Incompatible,
    /// Nothing recorded.
    NoEvidence,
}

/// Why a pair does or does not count.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsPairStatus;
/// assert_eq!(serde_json::to_value(EconomicsPairStatus::ControlFailed).unwrap(), "control_failed");
/// let s: EconomicsPairStatus = serde_json::from_value("usage_unavailable".into()).unwrap();
/// assert_ne!(s, EconomicsPairStatus::Valid);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EconomicsPairStatus {
    /// Both runs succeeded, are comparable and report usage.
    Valid,
    /// The control run failed a success gate.
    ControlFailed,
    /// The treatment run failed a success gate.
    TreatmentFailed,
    /// Both failed.
    BothFailed,
    /// A run reports no usage to compare.
    UsageUnavailable,
    /// The runs differ in something the methodology requires them to share.
    Incomparable,
    /// One side was never recorded.
    Missing,
    /// A run is excluded under the outlier rule.
    Excluded,
}

/// A run's totals, derived from its raw usage.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsUsage;
/// let u = EconomicsUsage {
///     input_uncached: 10, cache_write: 90, cache_read: 900, output: 50,
///     total_input: 1000, total: 1050, requests: 3, ..EconomicsUsage::default()
/// };
/// assert_eq!(u.total_input, u.input_uncached + u.cache_write + u.cache_read);
/// let json = serde_json::to_value(&u).unwrap();
/// assert!(json.get("reasoning").is_none() && json.get("cost_microusd").is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsUsage {
    /// Input neither read from nor written to the cache.
    pub input_uncached: u64,
    /// Input written to the cache.
    pub cache_write: u64,
    /// Input read from the cache.
    pub cache_read: u64,
    /// Output, reasoning included.
    pub output: u64,
    /// Reasoning tokens, when every request reported them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u64>,
    /// All input.
    pub total_input: u64,
    /// All input and output.
    pub total: u64,
    /// Provider requests.
    pub requests: u64,
    /// Tool calls.
    pub tool_calls: u64,
    /// The harness's cost projection, in millionths of a dollar, when every session had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
}

/// A control run and its treatment run, and what follows from them.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsPair, EconomicsPairStatus};
/// let p: EconomicsPair = serde_json::from_value(serde_json::json!({
///     "suite": "pilot", "task": "vat-rounding", "category": "bugfix", "sessions": 1,
///     "repetition": 2, "control": "vat-rounding--baseline--r2", "status": "missing",
///     "reasons": ["no treatment run recorded"]
/// }))
/// .unwrap();
/// assert_eq!(p.status, EconomicsPairStatus::Missing);
/// assert!(p.treatment.is_none() && p.token_reduction.is_none(), "no side, no reduction");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsPair {
    /// The suite.
    pub suite: String,
    /// The task.
    pub task: String,
    /// The task's category.
    pub category: String,
    /// Sessions the task spans.
    pub sessions: usize,
    /// The repetition.
    pub repetition: u32,
    /// The control run's id, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// The treatment run's id, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment: Option<String>,
    /// Whether it counts.
    pub status: EconomicsPairStatus,
    /// Why not, when it does not.
    #[serde(default)]
    pub reasons: Vec<String>,
    /// The control's totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_usage: Option<EconomicsUsage>,
    /// The treatment's totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_usage: Option<EconomicsUsage>,
    /// `1 - treatment / control` on total tokens. Negative: the treatment cost more.
    /// Present on a valid pair, and on an excluded pair that would otherwise be valid, so
    /// that the outlier rule's effect can be shown both ways; only valid pairs enter a
    /// metric other than `effective_token_reduction.including_excluded`. The same holds for
    /// every reduction and overhead below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_reduction: Option<f64>,
    /// The same on the harness's cost projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_reduction: Option<f64>,
    /// The same on tool calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_reduction: Option<f64>,
    /// The same on the tokens of the last session only: what continuing cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation_reduction: Option<f64>,
    /// First-request input of the treatment minus that of the control: what the treatment
    /// adds before the model has done anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_request_overhead: Option<i64>,
}

/// The shape of a set of per-pair values.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsDistribution;
/// let d: EconomicsDistribution = serde_json::from_value(serde_json::json!({
///     "n": 1, "mean": 0.4, "median": 0.4, "p25": 0.4, "p75": 0.4, "p95": 0.4,
///     "min": 0.4, "max": 0.4
/// }))
/// .unwrap();
/// assert!(d.sd.is_none(), "one value has no sample standard deviation");
/// assert!(d.min <= d.p25 && d.p25 <= d.median && d.median <= d.p75 && d.p75 <= d.max);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsDistribution {
    /// How many values.
    pub n: usize,
    /// Arithmetic mean.
    pub mean: f64,
    /// Median.
    pub median: f64,
    /// 25th percentile.
    pub p25: f64,
    /// 75th percentile.
    pub p75: f64,
    /// 95th percentile.
    pub p95: f64,
    /// Smallest.
    pub min: f64,
    /// Largest.
    pub max: f64,
    /// Sample standard deviation; absent for one value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd: Option<f64>,
}

/// A bootstrap interval, with everything needed to reproduce it.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsInterval;
/// let i: EconomicsInterval = serde_json::from_value(serde_json::json!({
///     "level_bp": 9500, "low": 0.21, "high": 0.38,
///     "method": "bootstrap percentile of the median", "resamples": 10000, "seed": 20260924
/// }))
/// .unwrap();
/// assert!(i.low <= i.high);
/// assert_eq!(serde_json::to_value(&i).unwrap()["seed"], 20260924, "the seed travels with it");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsInterval {
    /// Confidence in basis points.
    pub level_bp: u32,
    /// Lower bound.
    pub low: f64,
    /// Upper bound.
    pub high: f64,
    /// How it was computed: a cluster bootstrap over tasks for every metric over pairs or
    /// runs, whose repetitions of one task are not independent, and an independent
    /// bootstrap over seeds for the context suite.
    pub method: String,
    /// Resamples.
    pub resamples: u32,
    /// Generator seed.
    pub seed: u64,
}

/// Where a metric stands against the publication rule: whether its value may back a claim,
/// is a result too thin to claim anything, or was never measured at all.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsMetricStatus;
/// assert_eq!(serde_json::to_value(EconomicsMetricStatus::NotMeasured).unwrap(), "not_measured");
/// let s: EconomicsMetricStatus = serde_json::from_value("preliminary".into()).unwrap();
/// assert_ne!(s, EconomicsMetricStatus::Verified, "a preliminary result is not a claim");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EconomicsMetricStatus {
    /// Measured, and meets the publication rule. Only the primary metric over all the
    /// evidence can be: the rule is evaluated for it and for nothing else, so no secondary
    /// metric and no narrowed slice is ever verified.
    Verified,
    /// Measured, but no claim may rest on it: below the publication rule, narrowed by a
    /// query, or a metric the rule was never evaluated for. A result, not a claim.
    Preliminary,
    /// Measured deterministically; the publication rule for sampled data does not apply.
    Measured,
    /// Nothing recorded to compute it from.
    NotMeasured,
}

/// One metric: its value, where it comes from, and how much it can bear.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsClass, EconomicsMetric, EconomicsMetricStatus};
/// let m: EconomicsMetric = serde_json::from_value(serde_json::json!({
///     "id": "effective_token_reduction", "title": "Effective token reduction",
///     "class": "derived", "inputs": "observed", "unit": "ratio",
///     "formula": "median of 1 - treatment / control", "status": "not_measured", "n": 0,
///     "suite": "pilot"
/// }))
/// .unwrap();
/// assert_eq!(m.inputs, Some(EconomicsClass::Observed));
/// assert!(m.value.is_none() && m.status == EconomicsMetricStatus::NotMeasured);
/// assert!(serde_json::to_value(&m).unwrap().get("value").is_none(), "no value, not a zero");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsMetric {
    /// The metric's id.
    pub id: String,
    /// A name.
    pub title: String,
    /// The measurement class.
    pub class: EconomicsClass,
    /// For a derived metric, the class of what it was derived from: a reduction derived from
    /// observed usage is not the same evidence as one derived from counted bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<EconomicsClass>,
    /// `ratio`, `tokens`, `calls`.
    pub unit: String,
    /// The formula, in words.
    pub formula: String,
    /// What the metric is not, where it is easily mistaken for something else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not: Option<String>,
    /// The headline value; absent when nothing was measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Where it stands.
    pub status: EconomicsMetricStatus,
    /// How many values it rests on.
    pub n: usize,
    /// Their distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution: Option<EconomicsDistribution>,
    /// The interval, when enough values exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<EconomicsInterval>,
    /// The suite.
    pub suite: String,
    /// The runs, pairs or seeds it rests on.
    #[serde(default)]
    pub evidence: Vec<String>,
    /// What a reader must know before using it.
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// A metric's value within one segment of the pairs.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsSegment;
/// let s: EconomicsSegment = serde_json::from_value(serde_json::json!({
///     "dimension": "category", "value": "multi-session", "n": 0
/// }))
/// .unwrap();
/// assert!(s.token_reduction.is_none(), "a segment with no valid pair has no distribution");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsSegment {
    /// `category`, `sessions`, `complexity`.
    pub dimension: String,
    /// The segment.
    pub value: String,
    /// Valid pairs in it.
    pub n: usize,
    /// Distribution of per-pair token reduction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_reduction: Option<EconomicsDistribution>,
}

/// Pair counts of a suite, so that nothing is dropped out of sight.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsPairCounts;
/// let c = EconomicsPairCounts {
///     declared: 14, attempted: 12, valid: 9, control_failed: 1,
///     treatment_failed: 1, both_failed: 0, other: 1,
/// };
/// let accounted = c.valid + c.control_failed + c.treatment_failed + c.both_failed + c.other;
/// assert_eq!(accounted, c.attempted, "every attempted pair is in exactly one bucket");
/// assert_eq!(EconomicsPairCounts::default().declared, 0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsPairCounts {
    /// Pairs the suite declares.
    pub declared: usize,
    /// Pairs with at least one run recorded.
    pub attempted: usize,
    /// Pairs that count.
    pub valid: usize,
    /// Pairs whose control failed.
    pub control_failed: usize,
    /// Pairs whose treatment failed.
    pub treatment_failed: usize,
    /// Pairs where both failed.
    pub both_failed: usize,
    /// Pairs not comparable, missing a side, lacking usage, or excluded.
    pub other: usize,
}

/// A suite and the state of its evidence.
///
/// ```
/// use majordomus_cli::economics::model::{
///     EconomicsFreshness, EconomicsPairCounts, EconomicsSuiteView,
/// };
/// let v = EconomicsSuiteView {
///     id: "pilot".into(), kind: "live".into(), version: 1, title: "Pilot".into(),
///     model: Some("claude-sonnet-5".into()), freshness: EconomicsFreshness::NoEvidence,
///     freshness_detail: Some("no run recorded".into()), runs: 0,
///     pairs: EconomicsPairCounts { declared: 14, ..EconomicsPairCounts::default() },
///     revisions: vec![], harnesses: vec![], models_reported: vec![],
/// };
/// let json = serde_json::to_value(&v).unwrap();
/// assert_eq!(json["freshness"], "no_evidence");
/// assert_eq!(json["pairs"]["declared"], 14, "declared pairs show even when none ran");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsSuiteView {
    /// The suite.
    pub id: String,
    /// `live` or `context`.
    pub kind: String,
    /// Its version.
    pub version: u32,
    /// A name.
    pub title: String,
    /// The model, for a live suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Where its evidence stands.
    pub freshness: EconomicsFreshness,
    /// Why, when not current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_detail: Option<String>,
    /// Runs recorded.
    pub runs: usize,
    /// Pair counts.
    pub pairs: EconomicsPairCounts,
    /// The revisions the evidence was recorded at.
    #[serde(default)]
    pub revisions: Vec<String>,
    /// The harness versions that ran it.
    #[serde(default)]
    pub harnesses: Vec<String>,
    /// The models the harness reported.
    #[serde(default)]
    pub models_reported: Vec<String>,
}

/// The one sentence the evidence allows, and why it allows no stronger one.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsVerdict;
/// let v: EconomicsVerdict = serde_json::from_value(serde_json::json!({
///     "publishable": false,
///     "statement": "No quantitative token claim: 9 valid pairs of the 30 the rule requires.",
///     "unmet": ["min_valid_pairs: 9 < 30"]
/// }))
/// .unwrap();
/// assert!(!v.publishable);
/// assert_eq!(v.unmet.len(), 1, "every threshold not met is named");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsVerdict {
    /// Whether a quantitative total-token claim meets the publication rule.
    pub publishable: bool,
    /// The strongest statement the evidence supports, generated, never typed.
    pub statement: String,
    /// Every publication threshold not met.
    #[serde(default)]
    pub unmet: Vec<String>,
}

/// The context suite's latest measurement, in brief.
///
/// ```
/// use std::collections::BTreeMap;
/// use majordomus_cli::economics::model::{EconomicsContextView, EconomicsTokenizer};
/// let v = EconomicsContextView {
///     revision: "8d37d011ef".into(), measured_at: "2026-09-24T10:00:00Z".into(),
///     tokenizer: EconomicsTokenizer {
///         encoding: "o200k_base".into(), implementation: "tiktoken-rs 0.12.0".into(),
///     },
///     seeds: 2, candidate_tokens: 20_000, considered_tokens: 300_000, selected_tokens: 15_000,
///     estimated_selected_tokens: 16_000,
///     excluded_tokens: BTreeMap::from([("budget".into(), 5_000)]),
///     seeds_over_budget_counted: 0, budget_tokens: 8_000,
/// };
/// assert_eq!(v.selected_tokens + v.excluded_tokens["budget"], v.candidate_tokens);
/// assert_eq!(serde_json::to_value(&v).unwrap()["tokenizer"]["encoding"], "o200k_base");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsContextView {
    /// The revision measured.
    pub revision: String,
    /// When.
    pub measured_at: String,
    /// The tokenizer.
    pub tokenizer: EconomicsTokenizer,
    /// Seeds measured.
    pub seeds: usize,
    /// Counted candidate tokens (relevant: selected or cut by the budget), all seeds.
    pub candidate_tokens: u64,
    /// Counted tokens of everything the compiler reached, all seeds.
    pub considered_tokens: u64,
    /// Counted selected tokens, all seeds.
    pub selected_tokens: u64,
    /// The compiler's own estimate of the selected tokens, all seeds.
    pub estimated_selected_tokens: u64,
    /// Counted tokens left out, by reason, all seeds.
    pub excluded_tokens: BTreeMap<String, u64>,
    /// Seeds whose counted selection exceeds the compiler's budget.
    pub seeds_over_budget_counted: usize,
    /// The compiler's budget.
    pub budget_tokens: u64,
}

/// A recorded measurement over time: one entry per recorded context run.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsHistoryPoint;
/// let p: EconomicsHistoryPoint = serde_json::from_value(serde_json::json!({
///     "suite": "context", "methodology": 1, "revision": "8d37d011ef",
///     "at": "2026-09-24T10:00:00Z", "metric": "context_reduction_ratio", "value": 0.62, "n": 40
/// }))
/// .unwrap();
/// assert_eq!((p.methodology, p.n), (1, 40));
/// assert_eq!(serde_json::to_value(&p).unwrap()["metric"], "context_reduction_ratio");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsHistoryPoint {
    /// The suite.
    pub suite: String,
    /// The methodology version, so that incompatible versions are never drawn as one line.
    pub methodology: u32,
    /// The revision.
    pub revision: String,
    /// When.
    pub at: String,
    /// The metric.
    pub metric: String,
    /// Its value.
    pub value: f64,
    /// How many values it rests on.
    pub n: usize,
}

/// The answer of `economics.summary`: every metric, where each comes from, and the one
/// statement the evidence allows.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsSummary;
/// let s: EconomicsSummary = serde_json::from_value(serde_json::json!({
///     "present": false,
///     "verdict": { "publishable": false, "statement": "No methodology is declared." },
///     "metrics": [], "suites": [], "pairs": [], "segments": [], "history": [],
///     "variants": [], "hypotheses": []
/// }))
/// .unwrap();
/// assert!(s.methodology.is_none() && s.publication.is_none() && s.diagnostics.is_empty());
/// let json = serde_json::to_value(&s).unwrap();
/// assert!(json.get("methodology").is_none(), "an absent methodology is absent, not null");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsSummary {
    /// Whether a methodology is declared here at all.
    pub present: bool,
    /// The methodology version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methodology: Option<u32>,
    /// The question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// The primary metric's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_metric: Option<String>,
    /// What the total-token evidence allows to be said.
    pub verdict: EconomicsVerdict,
    /// Every metric.
    pub metrics: Vec<EconomicsMetric>,
    /// Every suite and its evidence.
    pub suites: Vec<EconomicsSuiteView>,
    /// Every pair, valid or not.
    pub pairs: Vec<EconomicsPair>,
    /// Token reduction by segment.
    pub segments: Vec<EconomicsSegment>,
    /// The context suite in brief.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<EconomicsContextView>,
    /// The metric over recorded measurements.
    pub history: Vec<EconomicsHistoryPoint>,
    /// The arms of the experiment.
    pub variants: Vec<EconomicsVariant>,
    /// The hypotheses, as hypotheses.
    pub hypotheses: Vec<EconomicsHypothesis>,
    /// The publication rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<EconomicsPublicationPolicy>,
    /// Problems reading the declarations or the evidence.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

// ---------------------------------------------------------------- queries

/// The input of `economics.summary`: optional narrowing of the pairs and metrics. The
/// verdict is always about all the evidence, whatever is narrowed, and a narrowed metric
/// is never verified: the publication rule was evaluated over all of it, not over a slice.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsQuery;
/// let all: EconomicsQuery = serde_json::from_value(serde_json::json!({})).unwrap();
/// assert_eq!(all, EconomicsQuery::default());
/// let q: EconomicsQuery =
///     serde_json::from_value(serde_json::json!({ "suite": "pilot", "category": "bugfix" }))
///         .unwrap();
/// assert_eq!(q.suite.as_deref(), Some("pilot"));
/// let typo = serde_json::json!({ "suites": "pilot" });
/// let refused = serde_json::from_value::<EconomicsQuery>(typo);
/// assert!(refused.is_err(), "a misspelt filter is refused");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct EconomicsQuery {
    /// Only this suite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    /// Only tasks of this category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Only this task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Only runs that asked for this model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// The input of `economics.explain`: the id of the one metric to explain. Nothing else is
/// accepted, so a misspelt field is refused rather than silently ignored.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsExplainInput;
/// let i: EconomicsExplainInput =
///     serde_json::from_value(serde_json::json!({ "metric": "context_reduction_ratio" })).unwrap();
/// assert_eq!(i.metric, "context_reduction_ratio");
/// assert!(serde_json::from_value::<EconomicsExplainInput>(serde_json::json!({})).is_err());
/// let extra = serde_json::json!({ "metric": "context_reduction_ratio", "suite": "context" });
/// assert!(serde_json::from_value::<EconomicsExplainInput>(extra).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomicsExplainInput {
    /// The metric's id, as `economics.summary` lists it.
    pub metric: String,
}

/// The answer of `economics.explain`: one metric, and everything it rests on.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsClass, EconomicsExplanation};
/// let e: EconomicsExplanation = serde_json::from_value(serde_json::json!({
///     "metric": { "id": "context_reduction_ratio", "title": "Context reduction",
///         "class": "derived", "inputs": "counted", "unit": "ratio",
///         "formula": "median over seeds of 1 - selected_tokens / candidate_tokens",
///         "status": "not_measured", "n": 0, "suite": "context" },
///     "class_meaning": "Arithmetic over observed or counted values.", "methodology": 1,
///     "suites": [], "pairs": [], "excluded": [], "variants": [],
///     "reproduce": ["majordomus economics measure --suite context"]
/// }))
/// .unwrap();
/// assert_eq!(e.metric.inputs, Some(EconomicsClass::Counted));
/// assert!(e.pairs.is_empty(), "a counted metric rests on seeds, not on pairs");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsExplanation {
    /// The metric.
    pub metric: EconomicsMetric,
    /// What its class means, in the methodology's words.
    pub class_meaning: String,
    /// The methodology version.
    pub methodology: u32,
    /// The suites it draws on.
    pub suites: Vec<EconomicsSuiteView>,
    /// The pairs it rests on, valid or not, when it rests on pairs.
    pub pairs: Vec<EconomicsPair>,
    /// Runs excluded under the outlier rule.
    pub excluded: Vec<EconomicsExclusion>,
    /// The arms compared.
    pub variants: Vec<EconomicsVariant>,
    /// How to reproduce it.
    pub reproduce: Vec<String>,
}

/// The input of `economics.runs`: optional filters on suite, task and variant, all of which
/// must match. An empty query lists every recorded run, and an unknown field is refused.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRunsQuery;
/// let wire = serde_json::json!({ "suite": "pilot", "variant": "baseline" });
/// let q: EconomicsRunsQuery = serde_json::from_value(wire.clone()).unwrap();
/// assert!(q.task.is_none(), "an absent filter matches every task");
/// assert_eq!(serde_json::to_value(&q).unwrap(), wire, "absent filters are not written back");
/// let by_model = serde_json::json!({ "model": "claude-sonnet-5" });
/// assert!(serde_json::from_value::<EconomicsRunsQuery>(by_model).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct EconomicsRunsQuery {
    /// Only this suite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    /// Only this task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Only this variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// One run in brief, with its derived totals.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRunView;
/// let v: EconomicsRunView = serde_json::from_value(serde_json::json!({
///     "id": "vat-rounding--majordomus--r1", "suite": "pilot", "task": "vat-rounding",
///     "variant": "majordomus", "repetition": 1, "model": "claude-sonnet-5",
///     "revision": "8d37d011ef", "completed": false, "checks": [], "sessions": 1,
///     "path": ".ai/repo/benchmarks/economics/runs/pilot/vat-rounding--majordomus--r1.json"
/// }))
/// .unwrap();
/// assert!(v.usage.is_none(), "a run whose provider reported no usage has no totals");
/// assert!(v.path.ends_with(&format!("{}.json", v.id)));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsRunView {
    /// The run's id.
    pub id: String,
    /// The suite.
    pub suite: String,
    /// The task.
    pub task: String,
    /// The variant.
    pub variant: String,
    /// The repetition.
    pub repetition: u32,
    /// The model requested.
    pub model: String,
    /// The revision.
    pub revision: String,
    /// Whether every success gate passed.
    pub completed: bool,
    /// Each gate's verdict.
    pub checks: Vec<EconomicsCheck>,
    /// Sessions.
    pub sessions: usize,
    /// Totals, when the provider reported usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<EconomicsUsage>,
    /// Where the raw record is.
    pub path: String,
}

/// The answer of `economics.runs`: the raw facts every metric is computed from.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsRunList;
/// let l: EconomicsRunList =
///     serde_json::from_value(serde_json::json!({ "count": 0, "runs": [] })).unwrap();
/// assert_eq!(l.count, l.runs.len());
/// assert!(l.diagnostics.is_empty(), "no diagnostics listed means none");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EconomicsRunList {
    /// How many.
    pub count: usize,
    /// The runs, sorted by id.
    pub runs: Vec<EconomicsRunView>,
    /// Records that could not be read.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}
