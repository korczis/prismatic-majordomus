//! The proof plan: the tests that are to settle what Majordomus is worth, registered before
//! any of their records exists, and each protocol frozen by its own digest.
//!
//! `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) names every proof test:
//! the question it answers, the claims of `docs/CLAIMS.yaml` it settles, the dimension it
//! measures, the instrument that measures it, and, once it is registered, its protocol. A
//! protocol is how the test is decided, fixed in advance: a paired benchmark over an
//! economics suite's registration block, a count of trials, or an observation window.
//!
//! A protocol carries `registered_digest`, the [`digest`] of its own canonical form, and a
//! record counts toward its test only when it carries that digest and was recorded after
//! `registered_at` ([`counts_toward`]). Each protocol is frozen by its own digest rather
//! than by the plan file's, so registering a second test's protocol never makes the first
//! test's records stale, and the plan file is a freshness input of no suite: [`load`]
//! refuses a suite whose freshness inputs reach it.
//!
//! The canonical form is the protocol as the YAML reader reads it, without its
//! `registered_digest`, with every object's keys sorted and no whitespace. It is the data
//! as written, not a typed value written back, so a later version of this code that adds a
//! defaulted field changes no digest and stales no record.
//!
//! ```
//! use majordomus_cli::economics::proof::{load, NO_PLAN};
//! let dir = tempfile::tempdir().unwrap();
//! let findings = load(dir.path()).unwrap_err();
//! assert_eq!(findings[0].kind, "no_plan");
//! assert!(findings[0].detail.starts_with(NO_PLAN), "an absent plan is not an empty plan");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::claims::EconomicsFinding;
use super::model::EconomicsClass;
use crate::metadata::yaml;

/// Where the plan lives, relative to the repository root.
pub const PLAN: &str = ".ai/repo/benchmarks/proof-plan.yaml";

/// The schema the plan carries.
pub const PLAN_SCHEMA: &str = "proof-plan/v1";

/// What [`load`] says, first, when there is no plan file.
pub const NO_PLAN: &str = "no proof plan is registered";

/// The dimensions a proof test may measure, in the order the owner's report names them.
pub const DIMENSIONS: &[&str] = &[
    "accepted_results",
    "wall_time",
    "human_interventions",
    "ci_reruns",
    "spend",
    "recovery",
    "onboarding",
];

/// The kinds of decision a protocol may declare.
pub const DECISION_KINDS: &[&str] = &["paired", "trials", "window"];

/// The validated proof plan. Only [`load`] builds one, and only when no finding stands.
///
/// ```
/// use majordomus_cli::economics::proof::ProofPlan;
/// let plan = ProofPlan { schema: "proof-plan/v1".into(), tests: vec![] };
/// assert!(plan.test("t1").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofPlan {
    /// `proof-plan/v1`.
    pub schema: String,
    /// The tests, in the plan's order.
    pub tests: Vec<ProofTest>,
}

impl ProofPlan {
    /// The test with this id, if the plan registers it.
    ///
    /// ```
    /// use majordomus_cli::economics::proof::{ProofInstrument, ProofPlan, ProofTest};
    /// let test = ProofTest {
    ///     id: "t5-resilience".into(), title: "Resilience".into(), question: "q".into(),
    ///     settles: vec!["proof-t5".into()], dimension: "recovery".into(), value_words: vec![],
    ///     instrument: ProofInstrument { status: "planned".into(), issues: vec!["I3353".into()] },
    ///     protocol: None,
    /// };
    /// let plan = ProofPlan { schema: "proof-plan/v1".into(), tests: vec![test] };
    /// assert_eq!(plan.test("t5-resilience").unwrap().dimension, "recovery");
    /// assert!(plan.test("t6-first-intent").is_none());
    /// ```
    pub fn test(&self, id: &str) -> Option<&ProofTest> {
        self.tests.iter().find(|t| t.id == id)
    }
}

/// One proof test: what it asks, what it settles, what it measures and with what.
///
/// ```
/// use majordomus_cli::economics::proof::ProofTest;
/// let t: ProofTest = serde_json::from_value(serde_json::json!({
///     "id": "t1-external-onboarding", "title": "Onboarding", "question": "Can a newcomer?",
///     "settles": ["proof-t1"], "dimension": "onboarding", "value_words": ["first intent"],
///     "instrument": { "status": "planned", "issues": ["I3353"] }
/// }))
/// .unwrap();
/// assert!(t.protocol.is_none(), "a test is registered before its protocol");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofTest {
    /// A slug, unique in the plan.
    pub id: String,
    /// A name.
    pub title: String,
    /// The question the test answers.
    pub question: String,
    /// The ids of the claims of `docs/CLAIMS.yaml` the test settles.
    pub settles: Vec<String>,
    /// One of [`DIMENSIONS`].
    pub dimension: String,
    /// The words a result is stated in.
    #[serde(default)]
    pub value_words: Vec<String>,
    /// What measures it.
    pub instrument: ProofInstrument,
    /// How it is decided, once registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProofProtocol>,
}

/// The instrument of a test: `built`, or `planned` with the issues that build it.
///
/// ```
/// use majordomus_cli::economics::proof::ProofInstrument;
/// let i: ProofInstrument =
///     serde_json::from_value(serde_json::json!({ "status": "built" })).unwrap();
/// assert!(i.issues.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofInstrument {
    /// `built` or `planned`.
    pub status: String,
    /// The issue ids that build it, or built it.
    #[serde(default)]
    pub issues: Vec<String>,
}

/// A registered protocol. `registered_digest` is the [`protocol_digest`] of the protocol as
/// written; the validator refuses a protocol whose digest does not match.
///
/// ```
/// use majordomus_cli::economics::proof::{ProofDecision, ProofProtocol};
/// let p: ProofProtocol = serde_json::from_value(serde_json::json!({
///     "version": 1, "registered_at": "2026-10-20T00:00:00Z",
///     "changes": [{ "version": 1, "why": "The first protocol." }],
///     "decision": { "kind": "paired", "suite": "pilot", "registration_digest": "sha256:00" },
///     "registered_digest": "sha256:00"
/// }))
/// .unwrap();
/// assert!(matches!(p.decision, ProofDecision::Paired { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofProtocol {
    /// The protocol's own version, from 1.
    pub version: u32,
    /// When it was registered, `YYYY-MM-DDTHH:MM:SSZ`. A record from before does not count.
    pub registered_at: String,
    /// The change log: one entry for every version.
    pub changes: Vec<ProofChange>,
    /// How the test is decided.
    pub decision: ProofDecision,
    /// The digest of this protocol's canonical form.
    pub registered_digest: String,
}

/// One entry of a protocol's change log.
///
/// ```
/// use majordomus_cli::economics::proof::ProofChange;
/// let c: ProofChange =
///     serde_json::from_value(serde_json::json!({ "version": 1, "why": "First." })).unwrap();
/// assert_eq!(c.version, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofChange {
    /// The version it introduced.
    pub version: u32,
    /// Why.
    pub why: String,
}

/// How a protocol decides its test.
///
/// ```
/// use majordomus_cli::economics::proof::ProofDecision;
/// let d: ProofDecision = serde_json::from_value(serde_json::json!({
///     "kind": "trials", "fields": [{ "name": "minutes", "class": "observed" }],
///     "min_trials": 3, "min_successes": 2, "all_must_pass": false
/// }))
/// .unwrap();
/// assert_eq!(d.kind(), "trials");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProofDecision {
    /// A paired benchmark over an economics suite, pre-registered by the suite's own
    /// registration block: the protocol names the suite and that block's [`digest`].
    Paired {
        /// The economics suite's id.
        suite: String,
        /// The digest of the suite's `registration` block.
        registration_digest: String,
    },
    /// Independent trials, each a record of the declared fields.
    Trials {
        /// The fields every trial record carries.
        fields: Vec<ProofField>,
        /// How many trials decide the test.
        min_trials: u32,
        /// How many of them must succeed.
        min_successes: u32,
        /// Whether one failed trial decides against.
        all_must_pass: bool,
    },
    /// An observation window over admitted work.
    Window {
        /// The period observed.
        period: ProofPeriod,
        /// The work the window admits.
        admits: String,
        /// For each field, the largest share of admitted work it may leave not observed.
        bounds: Vec<ProofBound>,
    },
}

impl ProofDecision {
    /// The decision's kind, as the plan spells it: one of [`DECISION_KINDS`].
    ///
    /// ```
    /// use majordomus_cli::economics::proof::ProofDecision;
    /// let d = ProofDecision::Paired { suite: "pilot".into(), registration_digest: "x".into() };
    /// assert_eq!(d.kind(), "paired");
    /// ```
    pub fn kind(&self) -> &'static str {
        match self {
            ProofDecision::Paired { .. } => "paired",
            ProofDecision::Trials { .. } => "trials",
            ProofDecision::Window { .. } => "window",
        }
    }
}

/// A field a trial record carries, with the class its number must have.
///
/// ```
/// use majordomus_cli::economics::proof::ProofField;
/// use majordomus_cli::economics::model::EconomicsClass;
/// let f: ProofField =
///     serde_json::from_value(serde_json::json!({ "name": "minutes", "class": "counted" }))
///         .unwrap();
/// assert_eq!(f.class, EconomicsClass::Counted);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofField {
    /// The field's name.
    pub name: String,
    /// Its class: observed, counted or derived.
    pub class: EconomicsClass,
}

/// The period a window observes, both ends `YYYY-MM-DDTHH:MM:SSZ`, `from` before `until`.
///
/// ```
/// use majordomus_cli::economics::proof::ProofPeriod;
/// let p: ProofPeriod = serde_json::from_value(serde_json::json!({
///     "from": "2026-10-20T00:00:00Z", "until": "2026-11-19T00:00:00Z"
/// }))
/// .unwrap();
/// assert!(p.from < p.until);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofPeriod {
    /// The first instant observed.
    pub from: String,
    /// The instant observation ends.
    pub until: String,
}

/// A bound on one field's not-observed share of a window's admitted work, in basis points.
///
/// ```
/// use majordomus_cli::economics::proof::ProofBound;
/// let b: ProofBound = serde_json::from_value(serde_json::json!({
///     "field": "interventions", "max_not_observed_bp": 2000
/// }))
/// .unwrap();
/// assert_eq!(b.max_not_observed_bp, 2000);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofBound {
    /// The field.
    pub field: String,
    /// The largest share of admitted work that may leave it not observed, 0 to 10000.
    pub max_not_observed_bp: u32,
}

/// The one digest of the proof plan: SHA-256 over the canonical JSON of `value` (object
/// keys sorted at every depth, no whitespace), spelled `sha256:<hex>`. A protocol's
/// `registered_digest` and a paired protocol's `registration_digest` are both this
/// function, so pre-registration is written once and read the same way everywhere.
///
/// ```
/// use majordomus_cli::economics::proof::digest;
/// let a = digest(&serde_json::json!({ "b": 1, "a": [1, 2] }));
/// assert_eq!(a, digest(&serde_json::json!({ "a": [1, 2], "b": 1 })), "key order is not data");
/// assert_ne!(a, digest(&serde_json::json!({ "a": [2, 1], "b": 1 })), "list order is");
/// assert!(a.starts_with("sha256:") && a.len() == 7 + 64);
/// ```
pub fn digest(value: &Value) -> String {
    let bytes = crate::mesh::journal::canonical_json(value);
    format!("sha256:{}", crate::policy::sha256_bytes_hex(&bytes))
}

/// The digest a protocol registers: [`digest`] of the protocol as written, without its own
/// `registered_digest`.
///
/// ```
/// use majordomus_cli::economics::proof::{digest, protocol_digest};
/// let p = serde_json::json!({ "version": 1, "registered_digest": "sha256:stale" });
/// assert_eq!(protocol_digest(&p), digest(&serde_json::json!({ "version": 1 })));
/// ```
pub fn protocol_digest(protocol: &Value) -> String {
    let mut canonical = protocol.clone();
    if let Value::Object(map) = &mut canonical {
        map.remove("registered_digest");
    }
    digest(&canonical)
}

/// What a record carries that decides whether it counts: the digest of the protocol it was
/// recorded under, and when it was recorded. Every proof record type implements it.
///
/// ```
/// use majordomus_cli::economics::proof::{ProofRecord, ProofStamp};
/// let at = "2026-10-21T00:00:00Z";
/// let s = ProofStamp { protocol_digest: "sha256:ab".into(), recorded_at: at.into() };
/// assert_eq!((s.protocol_digest(), s.recorded_at()), ("sha256:ab", at));
/// ```
pub trait ProofRecord {
    /// The digest of the protocol the record was made under, as [`digest`] spells it.
    ///
    /// ```
    /// use majordomus_cli::economics::proof::{ProofRecord, ProofStamp};
    /// let at = "2026-10-21T00:00:00Z";
    /// let s = ProofStamp { protocol_digest: "sha256:ab".into(), recorded_at: at.into() };
    /// assert_eq!(ProofRecord::protocol_digest(&s), "sha256:ab");
    /// ```
    fn protocol_digest(&self) -> &str;

    /// The instant the record was made, written as `YYYY-MM-DDTHH:MM:SSZ`.
    ///
    /// ```
    /// use majordomus_cli::economics::proof::{ProofRecord, ProofStamp};
    /// let at = "2026-10-21T00:00:00Z";
    /// let s = ProofStamp { protocol_digest: "sha256:ab".into(), recorded_at: at.into() };
    /// assert_eq!(ProofRecord::recorded_at(&s), at);
    /// ```
    fn recorded_at(&self) -> &str;
}

/// The smallest [`ProofRecord`]: the two values and nothing else.
///
/// ```
/// use majordomus_cli::economics::proof::{ProofRecord, ProofStamp};
/// let s: ProofStamp = serde_json::from_value(serde_json::json!({
///     "protocol_digest": "sha256:ab", "recorded_at": "2026-10-21T00:00:00Z"
/// }))
/// .unwrap();
/// assert_eq!(s.protocol_digest(), "sha256:ab");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofStamp {
    /// The digest of the protocol the record was made under.
    pub protocol_digest: String,
    /// When it was recorded.
    pub recorded_at: String,
}

impl ProofRecord for ProofStamp {
    fn protocol_digest(&self) -> &str {
        &self.protocol_digest
    }

    fn recorded_at(&self) -> &str {
        &self.recorded_at
    }
}

/// Whether a record counts toward a test, and when it does not, why.
///
/// ```
/// use majordomus_cli::economics::proof::ProofCount;
/// assert!(ProofCount::Counts.counts());
/// assert!(!ProofCount::NoProtocol.counts());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProofCount {
    /// It carries the test's protocol digest and postdates its registration.
    Counts,
    /// The test has no protocol yet, so nothing counts toward it.
    NoProtocol,
    /// It was recorded under another protocol.
    OtherProtocol {
        /// The digest the record carries.
        carried: String,
        /// The digest the test's protocol registers.
        registered: String,
    },
    /// It was recorded at or before the protocol's registration.
    BeforeRegistration {
        /// When the record was made.
        recorded_at: String,
        /// When the protocol was registered.
        registered_at: String,
    },
    /// Its time, or the protocol's, is not `YYYY-MM-DDTHH:MM:SSZ`.
    UnreadableTime {
        /// The time that could not be read.
        time: String,
    },
}

impl ProofCount {
    /// Whether the record counts toward its test: true only for [`ProofCount::Counts`].
    ///
    /// ```
    /// use majordomus_cli::economics::proof::ProofCount;
    /// let other = ProofCount::OtherProtocol { carried: "a".into(), registered: "b".into() };
    /// assert!(!other.counts());
    /// ```
    pub fn counts(&self) -> bool {
        matches!(self, ProofCount::Counts)
    }

    /// Why the record does not count, for a list a person reads; `None` when it counts.
    ///
    /// ```
    /// use majordomus_cli::economics::proof::ProofCount;
    /// assert_eq!(ProofCount::Counts.reason(), None);
    /// let early = ProofCount::BeforeRegistration {
    ///     recorded_at: "2026-10-19T00:00:00Z".into(),
    ///     registered_at: "2026-10-20T00:00:00Z".into(),
    /// };
    /// assert!(early.reason().unwrap().contains("before the protocol was registered"));
    /// ```
    pub fn reason(&self) -> Option<String> {
        match self {
            ProofCount::Counts => None,
            ProofCount::NoProtocol => Some("the test has no registered protocol".into()),
            ProofCount::OtherProtocol {
                carried,
                registered,
            } => Some(format!(
                "recorded under protocol {carried}, and the test registers {registered}"
            )),
            ProofCount::BeforeRegistration {
                recorded_at,
                registered_at,
            } => Some(format!(
                "recorded at {recorded_at}, not after {registered_at}, before the protocol \
                 was registered"
            )),
            ProofCount::UnreadableTime { time } => Some(format!(
                "{time:?} is not a time of the form YYYY-MM-DDTHH:MM:SSZ"
            )),
        }
    }
}

/// Whether `record` counts toward `test`: only when the test has a protocol, the record
/// carries that protocol's digest, and it was recorded strictly after `registered_at`. A
/// test's own protocol decides, so another test's registration changes nothing here.
///
/// ```
/// use majordomus_cli::economics::proof::*;
/// let mut test = ProofTest {
///     id: "t3".into(), title: "t".into(), question: "q".into(), settles: vec!["c".into()],
///     dimension: "wall_time".into(), value_words: vec![],
///     instrument: ProofInstrument { status: "built".into(), issues: vec![] }, protocol: None,
/// };
/// let stamp = |d: &str, at: &str| ProofStamp {
///     protocol_digest: d.into(),
///     recorded_at: at.into(),
/// };
/// let late = stamp("sha256:p", "2026-10-21T00:00:00Z");
/// assert_eq!(counts_toward(&late, &test), ProofCount::NoProtocol);
/// test.protocol = Some(ProofProtocol {
///     version: 1, registered_at: "2026-10-20T00:00:00Z".into(),
///     changes: vec![ProofChange { version: 1, why: "First.".into() }],
///     decision: ProofDecision::Paired { suite: "pilot".into(), registration_digest: "r".into() },
///     registered_digest: "sha256:p".into(),
/// });
/// assert!(counts_toward(&late, &test).counts());
/// assert!(!counts_toward(&stamp("sha256:q", "2026-10-21T00:00:00Z"), &test).counts());
/// assert!(!counts_toward(&stamp("sha256:p", "2026-10-20T00:00:00Z"), &test).counts());
/// ```
pub fn counts_toward(record: &impl ProofRecord, test: &ProofTest) -> ProofCount {
    let Some(protocol) = &test.protocol else {
        return ProofCount::NoProtocol;
    };
    if record.protocol_digest() != protocol.registered_digest {
        return ProofCount::OtherProtocol {
            carried: record.protocol_digest().to_string(),
            registered: protocol.registered_digest.clone(),
        };
    }
    let time = |t: &str| crate::peers::parse_rfc3339(t).ok_or(t.to_string());
    match (time(record.recorded_at()), time(&protocol.registered_at)) {
        (Ok(at), Ok(registered)) if at > registered => ProofCount::Counts,
        (Ok(_), Ok(_)) => ProofCount::BeforeRegistration {
            recorded_at: record.recorded_at().to_string(),
            registered_at: protocol.registered_at.clone(),
        },
        (Err(time), _) | (_, Err(time)) => ProofCount::UnreadableTime { time },
    }
}

/// Read and validate the plan under `root`, fail-closed: the plan when no finding stands,
/// every finding otherwise. An absent file is the finding `no_plan`, never an empty valid
/// plan. The kinds, each with its own detail:
///
/// - `no_plan`, `plan_unreadable`, `schema_unknown`, `no_tests`: the file as a whole.
/// - `test_malformed`: a test or its protocol does not have the declared shape.
/// - `test_duplicate`, `dimension_unknown`, `claim_unknown`, `settles_nothing`,
///   `instrument_unknown`, `instrument_unplanned`: a test's own fields.
/// - `decision_unknown`, `digest_mismatch`, `registered_at_invalid`, `version_unchanged`:
///   a protocol.
/// - `paired_suite_unknown`, `paired_registration_absent`, `paired_registration_mismatch`:
///   a paired protocol whose suite or registration digest does not resolve.
/// - `trials_invalid`, `window_invalid`: a trials or window decision that cannot decide.
/// - `claims_unreadable`, `suite_unreadable`, `suite_duplicate`: an input the checks read, or
///   two suite files that declare one id.
/// - `plan_is_freshness_input`: a suite's freshness inputs could select the plan file, read
///   as `git ls-files` reads a pathspec list, with its wildcards and magic.
///
/// ```
/// use majordomus_cli::economics::proof::{load, PLAN};
/// let dir = tempfile::tempdir().unwrap();
/// let plan = dir.path().join(PLAN);
/// std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
/// std::fs::write(&plan, "schema: proof-plan/v0\ntests: []\n").unwrap();
/// let kinds: Vec<String> = load(dir.path()).unwrap_err().into_iter().map(|f| f.kind).collect();
/// assert_eq!(kinds, ["schema_unknown", "no_tests"]);
/// ```
pub fn load(root: &Path) -> Result<ProofPlan, Vec<EconomicsFinding>> {
    let text = match std::fs::read_to_string(root.join(PLAN)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(vec![finding(
                "no_plan",
                PLAN,
                format!("{NO_PLAN}: write {PLAN} (schema {PLAN_SCHEMA})"),
            )]);
        }
        Err(e) => return Err(vec![finding("plan_unreadable", PLAN, e.to_string())]),
    };
    let map = yaml::parse_mapping(&text).map_err(|e| vec![finding("plan_unreadable", PLAN, e)])?;
    validate(root, &map)
}

fn finding(kind: &str, path: &str, detail: String) -> EconomicsFinding {
    EconomicsFinding {
        kind: kind.into(),
        path: path.into(),
        line: None,
        detail,
    }
}

/// An economics suite as written: its file and its parsed mapping.
struct RawSuite {
    path: String,
    map: Map<String, Value>,
}

/// Every economics suite, by id, read as data, with the files that could not be read. A
/// suite without an id, or with an id another file already declares, is a finding and is
/// not keyed, so a paired protocol never resolves against the wrong file.
fn raw_suites(root: &Path, findings: &mut Vec<EconomicsFinding>) -> BTreeMap<String, RawSuite> {
    let mut out = BTreeMap::new();
    for p in super::sorted_entries(&root.join(super::DIR).join("suites")) {
        if p.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let shown = format!(
            "{}/suites/{}",
            super::DIR,
            p.file_name().unwrap_or_default().to_string_lossy()
        );
        let parsed = std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| yaml::parse_mapping(&t));
        match parsed {
            Ok(map) => match map.get("id").and_then(Value::as_str).map(str::to_string) {
                None => findings.push(finding("suite_unreadable", &shown, "it has no id".into())),
                Some(id) => match out.get(&id) {
                    Some(RawSuite { path, .. }) => findings.push(finding(
                        "suite_duplicate",
                        &shown,
                        format!("it declares suite {id}, which {path} already declares"),
                    )),
                    None => {
                        out.insert(id, RawSuite { path: shown, map });
                    }
                },
            },
            Err(e) => findings.push(finding("suite_unreadable", &shown, e)),
        }
    }
    out
}

/// The plan file and every directory above it: what a pathspec reaches the plan through.
fn plan_and_ancestors() -> Vec<&'static str> {
    let mut out = vec![PLAN];
    let mut at = PLAN;
    while let Some(i) = at.rfind('/') {
        at = &PLAN[..i];
        out.push(at);
    }
    out
}

/// One git pathspec as `git ls-files` reads it: its magic, then the pattern.
struct Pathspec<'a> {
    pattern: &'a str,
    exclude: bool,
    literal: bool,
    icase: bool,
}

impl<'a> Pathspec<'a> {
    /// Read the long (`:(glob,icase)x`) and short (`:/x`, `:!x`, `:^x`) magic. A magic word
    /// this does not know only narrows what git selects, so it is read as no narrowing.
    fn parse(spec: &'a str) -> Pathspec<'a> {
        let (mut exclude, mut literal, mut icase) = (false, false, false);
        let mut rest = spec;
        if let Some(long) = spec.strip_prefix(":(") {
            if let Some(end) = long.find(')') {
                for word in long[..end].split(',').map(str::trim) {
                    match word {
                        "exclude" => exclude = true,
                        "literal" => literal = true,
                        "icase" => icase = true,
                        _ => {}
                    }
                }
                rest = &long[end + 1..];
            }
        } else if let Some(short) = spec.strip_prefix(':') {
            let end = short.find(|c| !"/!^".contains(c)).unwrap_or(short.len());
            exclude = short[..end].contains(['!', '^']);
            rest = short[end..].strip_prefix(':').unwrap_or(&short[end..]);
        }
        while let Some(r) = rest.strip_prefix("./") {
            rest = r;
        }
        Pathspec {
            pattern: rest.trim_end_matches('/'),
            exclude,
            literal,
            icase,
        }
    }

    fn wild(&self) -> bool {
        !self.literal && self.pattern.contains(['*', '?', '['])
    }

    /// Whether it selects the plan file, whose path is lower case. `wide` ignores case and
    /// reads every wildcard as crossing `/`, which selects at least what git selects; without
    /// it only the exact name of the file or a directory above it counts, which selects at
    /// most what git selects.
    fn selects_plan(&self, wide: bool) -> bool {
        let p = if wide || self.icase {
            self.pattern.to_lowercase()
        } else {
            self.pattern.to_string()
        };
        if p.is_empty() || p == "." {
            return true;
        }
        plan_and_ancestors().into_iter().any(|path| {
            p == path || (wide && self.wild() && wildmatch(p.as_bytes(), path.as_bytes()))
        })
    }
}

/// A wildcard match where `*` crosses `/`, `?` is one byte and `[..]` one byte of a class,
/// with `**/` also matching no directory: at least what either git pathspec reading selects.
fn wildmatch(p: &[u8], t: &[u8]) -> bool {
    match p.first() {
        None => t.is_empty(),
        Some(b'*') => {
            let stars = p.iter().take_while(|&&c| c == b'*').count();
            let rest = &p[stars..];
            if stars > 1 && rest.first() == Some(&b'/') && wildmatch(&rest[1..], t) {
                return true;
            }
            (0..=t.len()).any(|i| wildmatch(rest, &t[i..]))
        }
        Some(b'?') => !t.is_empty() && wildmatch(&p[1..], &t[1..]),
        Some(b'[') => match (p[1..].iter().position(|&c| c == b']'), t.first()) {
            (Some(close), Some(&c)) if close > 0 => {
                let class = &p[1..=close];
                let (negated, class) = match class.first() {
                    Some(b'!' | b'^') => (true, &class[1..]),
                    _ => (false, class),
                };
                let mut hit = false;
                let mut i = 0;
                while i < class.len() {
                    if i + 2 < class.len() && class[i + 1] == b'-' {
                        hit |= class[i] <= c && c <= class[i + 2];
                        i += 3;
                    } else {
                        hit |= class[i] == c;
                        i += 1;
                    }
                }
                hit != negated && wildmatch(&p[close + 2..], &t[1..])
            }
            _ => t.first() == Some(&b'[') && wildmatch(&p[1..], &t[1..]),
        },
        Some(&c) => t.first() == Some(&c) && wildmatch(&p[1..], &t[1..]),
    }
}

/// What carries a suite's freshness inputs to the plan file, read as `git ls-files` reads
/// the list and failing toward refusal: an including pathspec that could select the plan, or
/// a list that selects every file because it is empty or only excludes. An excluding
/// pathspec removes the plan only when it names the file or a directory above it.
fn reaching_input(specs: &[&str]) -> Option<String> {
    let parsed: Vec<(&str, Pathspec)> = specs.iter().map(|s| (*s, Pathspec::parse(s))).collect();
    if parsed
        .iter()
        .any(|(_, p)| p.exclude && p.selects_plan(false))
    {
        return None;
    }
    let mut includes = parsed.iter().filter(|(_, p)| !p.exclude).peekable();
    if includes.peek().is_none() {
        return Some(
            "a freshness_inputs list with no including pathspec, which git reads as every \
             tracked file,"
                .into(),
        );
    }
    includes
        .find(|(_, p)| p.selects_plan(true))
        .map(|(s, _)| format!("freshness input {s:?}"))
}

fn validate(root: &Path, map: &Map<String, Value>) -> Result<ProofPlan, Vec<EconomicsFinding>> {
    let mut findings = Vec::new();
    let schema = map.get("schema").and_then(Value::as_str).unwrap_or("");
    if schema != PLAN_SCHEMA {
        findings.push(finding(
            "schema_unknown",
            PLAN,
            format!("the plan's schema is {schema:?}; write {PLAN_SCHEMA}"),
        ));
    }
    let raw_tests: Vec<Value> = match map.get("tests") {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };
    if raw_tests.is_empty() {
        findings.push(finding(
            "no_tests",
            PLAN,
            "the plan registers no test; a plan is the list of tests it registers".into(),
        ));
    }
    let claims = match super::claims::claim_ids(root) {
        Ok(ids) => ids,
        Err(e) => {
            findings.push(finding("claims_unreadable", super::claims::CLAIMS, e));
            None
        }
    };
    let suites = raw_suites(root, &mut findings);
    for suite in suites.values() {
        let Some(inputs) = suite.map.get("freshness_inputs").and_then(Value::as_array) else {
            continue;
        };
        let specs: Vec<&str> = inputs.iter().filter_map(Value::as_str).collect();
        if let Some(what) = reaching_input(&specs) {
            findings.push(finding(
                "plan_is_freshness_input",
                &suite.path,
                format!(
                    "{what} reaches {PLAN}: each protocol is frozen by its own digest, so the \
                     plan is a freshness input of no suite"
                ),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    let mut tests = Vec::new();
    for (i, raw) in raw_tests.iter().enumerate() {
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(format!("#{}", i + 1));
        if !seen.insert(id.clone()) {
            findings.push(finding(
                "test_duplicate",
                PLAN,
                format!("test {id} is registered twice; a test id is unique in the plan"),
            ));
        }
        let kind = raw
            .pointer("/protocol/decision/kind")
            .and_then(Value::as_str);
        let mut raw = raw.clone();
        if let Some(kind) = kind.filter(|k| !DECISION_KINDS.contains(k)) {
            findings.push(finding(
                "decision_unknown",
                PLAN,
                format!(
                    "test {id}: decision kind {kind:?} is not one of {}",
                    DECISION_KINDS.join(", ")
                ),
            ));
            // the test's own fields are still judged, without the protocol that cannot be read
            if let Some(test) = raw.as_object_mut() {
                test.remove("protocol");
            }
        }
        match serde_json::from_value::<ProofTest>(raw.clone()) {
            Ok(test) => {
                let protocol = raw.get("protocol").cloned().unwrap_or(Value::Null);
                check_test(&test, &protocol, claims.as_ref(), &suites, &mut findings);
                tests.push(test);
            }
            Err(e) => findings.push(finding("test_malformed", PLAN, format!("test {id}: {e}"))),
        }
    }
    if findings.is_empty() {
        Ok(ProofPlan {
            schema: schema.to_string(),
            tests,
        })
    } else {
        Err(findings)
    }
}

fn check_test(
    test: &ProofTest,
    raw_protocol: &Value,
    claims: Option<&BTreeSet<String>>,
    suites: &BTreeMap<String, RawSuite>,
    findings: &mut Vec<EconomicsFinding>,
) {
    let id = &test.id;
    let mut refuse = |kind: &str, detail: String| {
        findings.push(finding(kind, PLAN, format!("test {id}: {detail}")));
    };
    if !DIMENSIONS.contains(&test.dimension.as_str()) {
        refuse(
            "dimension_unknown",
            format!(
                "dimension {:?} is not one of {}",
                test.dimension,
                DIMENSIONS.join(", ")
            ),
        );
    }
    if test.settles.is_empty() {
        refuse(
            "settles_nothing",
            "it settles no claim; name the claims of docs/CLAIMS.yaml it decides".into(),
        );
    }
    for claim in &test.settles {
        if !claims.is_some_and(|ids| ids.contains(claim)) {
            refuse(
                "claim_unknown",
                format!("it settles {claim}, which docs/CLAIMS.yaml does not declare"),
            );
        }
    }
    match (
        test.instrument.status.as_str(),
        test.instrument.issues.is_empty(),
    ) {
        ("planned", true) => refuse(
            "instrument_unplanned",
            "its instrument is planned and names no issue that builds it".into(),
        ),
        ("planned" | "built", _) => {}
        (other, _) => refuse(
            "instrument_unknown",
            format!("instrument status {other:?} is not built or planned"),
        ),
    }
    let Some(protocol) = &test.protocol else {
        return;
    };
    let computed = protocol_digest(raw_protocol);
    if computed != protocol.registered_digest {
        refuse(
            "digest_mismatch",
            format!(
                "the protocol registers {} and its canonical form digests to {computed}; a \
                 changed protocol is a new version with a changes entry and a new digest",
                protocol.registered_digest
            ),
        );
    }
    if crate::peers::parse_rfc3339(&protocol.registered_at).is_none() {
        refuse(
            "registered_at_invalid",
            format!(
                "registered_at {:?} is not YYYY-MM-DDTHH:MM:SSZ",
                protocol.registered_at
            ),
        );
    }
    let logged: BTreeSet<u32> = protocol.changes.iter().map(|c| c.version).collect();
    let missing: Vec<String> = (1..=protocol.version)
        .filter(|v| !logged.contains(v))
        .map(|v| v.to_string())
        .collect();
    if protocol.version == 0 {
        refuse(
            "version_unchanged",
            "protocol version 0: versions start at 1, and each has a changes entry".into(),
        );
    } else if !missing.is_empty() {
        refuse(
            "version_unchanged",
            format!(
                "protocol version {} has no changes entry for version {}; every version says \
                 why it changed",
                protocol.version,
                missing.join(", ")
            ),
        );
    }
    match &protocol.decision {
        ProofDecision::Paired {
            suite,
            registration_digest,
        } => match suites.get(suite).map(|s| s.map.get("registration")) {
            None => refuse(
                "paired_suite_unknown",
                format!("its paired protocol names suite {suite}, which is not declared"),
            ),
            Some(None) => refuse(
                "paired_registration_absent",
                format!("suite {suite} has no registration block for a paired protocol to name"),
            ),
            Some(Some(block)) if digest(block) != *registration_digest => refuse(
                "paired_registration_mismatch",
                format!(
                    "it names registration {registration_digest} of suite {suite}, whose \
                     registration block digests to {}",
                    digest(block)
                ),
            ),
            Some(Some(_)) => {}
        },
        ProofDecision::Trials {
            fields,
            min_trials,
            min_successes,
            all_must_pass,
        } => {
            let mut problems = Vec::new();
            if fields.is_empty() {
                problems.push("it declares no record field".to_string());
            }
            let mut names = BTreeSet::new();
            for f in fields {
                if !names.insert(f.name.as_str()) {
                    problems.push(format!("field {} is declared twice", f.name));
                }
                if !f.class.is_measurement() {
                    problems.push(format!(
                        "field {} is {}, and a trial records only observed, counted or \
                         derived numbers",
                        f.name,
                        f.class.word()
                    ));
                }
            }
            if *min_trials == 0 || min_successes > min_trials {
                problems.push(format!(
                    "min_successes {min_successes} of min_trials {min_trials}: at least one \
                     trial, and no more successes than trials"
                ));
            } else if *all_must_pass && min_successes != min_trials {
                problems.push(format!(
                    "all_must_pass with min_successes {min_successes} of min_trials \
                     {min_trials}: when every trial must pass, every trial is a success"
                ));
            }
            if !problems.is_empty() {
                refuse("trials_invalid", problems.join("; "));
            }
        }
        ProofDecision::Window {
            period,
            admits,
            bounds,
        } => {
            let mut problems = Vec::new();
            match (
                crate::peers::parse_rfc3339(&period.from),
                crate::peers::parse_rfc3339(&period.until),
            ) {
                (Some(from), Some(until)) if from < until => {}
                _ => problems.push(format!(
                    "period {} to {} is not two YYYY-MM-DDTHH:MM:SSZ times in order",
                    period.from, period.until
                )),
            }
            if admits.trim().is_empty() {
                problems.push("it admits no work".to_string());
            }
            if bounds.is_empty() {
                problems.push("it bounds no field".to_string());
            }
            let mut names = BTreeSet::new();
            for b in bounds {
                if !names.insert(b.field.as_str()) {
                    problems.push(format!("field {} is bounded twice", b.field));
                }
                if b.max_not_observed_bp > 10_000 {
                    problems.push(format!(
                        "field {} allows {} basis points not observed, more than all of it",
                        b.field, b.max_not_observed_bp
                    ));
                }
            }
            if !problems.is_empty() {
                refuse("window_invalid", problems.join("; "));
            }
        }
    }
}
