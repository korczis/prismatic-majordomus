//! The `dashboard` module: the Overview of the Dashboard Suite, as a projection of the
//! capabilities that already answer each of its facts (ADR 0088).
//!
//! **A dashboard has no data of its own.** `dashboard.overview` answers four questions —
//! *is it healthy*, *what changed*, *what is broken*, *what needs action* — and every card
//! under them is one fact read out of another capability's answer through the same
//! executor MCP, the HTTP routes and the Cockpit use. Nothing here is stored, counted or
//! judged a second time: the card carries the capability it asked, the input it asked with,
//! and the RFC 6901 pointer into that answer its value was read from, so a reader — or a
//! test — can ask the source the same question and get the same value.
//!
//! **The value is the source's, exactly.** A card's [`DashboardSource::measure`] is one of
//! two readings, and neither is arithmetic on the source's behalf:
//!
//! - [`DashboardMeasure::Exact`] — `value == answer.pointer(pointer)`, and `null` when the
//!   pointer resolves to nothing;
//! - [`DashboardMeasure::Count`] — the number at the pointer, or the length of the list or
//!   map there, and `0` when the source left it out: the sources omit an empty list and a
//!   zero tally (`skip_serializing_if`), so absence *is* their way of saying none.
//!
//! `tests/dashboard.rs` and `test/cases/534_the_overview_answers_four_questions.sh` hold
//! that sentence for every card, naming none: they execute each card's source with its
//! input and compare. A card that computed its own number fails them.
//!
//! **Status words are the sources' verdicts.** `fail` where the source says something
//! refuses (a failing check, a worktree out of place, a question that refuses `finish`),
//! `warn` where it names something worth looking at (a pending bump, a blocked issue, two
//! workers claiming one scope), `ok` otherwise — and `unknown`, never `ok`, when the source
//! could not answer, with its error as the card's detail. No threshold is invented here.
//!
//! **It is cheap.** Each source is asked once per answer, and the sources are the ones that
//! read what this process already holds: no delivery report, no release analysis, no
//! evidence judgement — nothing that rebuilds canonical state or refuses on a network.
//!
//! ```
//! use majordomus_cli::capability::builtin::dashboard::module;
//!
//! // one capability, reachable from every surface the registry projects
//! let m = module();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "dashboard.overview");
//! assert_eq!(c.exposure.http.as_ref().unwrap().path, "/api/v1/dashboard/overview");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["dashboard", "overview"]);
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::health::HealthStatus;
use super::{get, mcp, Empty};

// ---------------------------------------------------------------- output types

/// How a card's value is read out of its source's answer. Neither reading is arithmetic on
/// the source's behalf.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::DashboardMeasure;
/// use serde_json::json;
///
/// let answer = json!({ "status": "warn", "tallies": { "ok": 6 }, "overlaps": [1, 2] });
/// // exact: the value at the pointer, null when there is none
/// assert_eq!(DashboardMeasure::Exact.read(&answer, "/status"), json!("warn"));
/// assert_eq!(DashboardMeasure::Exact.read(&answer, "/tallies/fail"), json!(null));
/// // count: a number as it stands, a list or map by its length, and an omitted one is none
/// assert_eq!(DashboardMeasure::Count.read(&answer, "/tallies/ok"), json!(6));
/// assert_eq!(DashboardMeasure::Count.read(&answer, "/overlaps"), json!(2));
/// assert_eq!(DashboardMeasure::Count.read(&answer, "/tallies/fail"), json!(0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DashboardMeasure {
    /// The value at the pointer, exactly; `null` when the pointer resolves to nothing.
    Exact,
    /// The number at the pointer, or the length of the list or map there; `0` when the
    /// source left it out, which is how the sources say an empty list or a zero tally.
    Count,
}

impl DashboardMeasure {
    /// Read the card's value out of the source's answer.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::dashboard::DashboardMeasure;
    /// use serde_json::json;
    /// let answer = json!({ "issue": { "id": "I0001" } });
    /// assert_eq!(DashboardMeasure::Exact.read(&answer, "/issue/id"), json!("I0001"));
    /// assert_eq!(DashboardMeasure::Count.read(&answer, "/blockers"), json!(0));
    /// ```
    pub fn read(self, answer: &Value, pointer: &str) -> Value {
        let at = answer.pointer(pointer);
        match self {
            DashboardMeasure::Exact => at.cloned().unwrap_or(Value::Null),
            DashboardMeasure::Count => match at {
                None | Some(Value::Null) => json!(0),
                Some(Value::Array(list)) => json!(list.len()),
                Some(Value::Object(map)) => json!(map.len()),
                Some(other) => other.clone(),
            },
        }
    }
}

/// Where a card's value came from: the capability asked, the input it was asked with, and
/// the pointer into its answer. Asking the capability the same question and reading the
/// pointer with the same measure yields the card's value; that is the whole contract.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::{DashboardMeasure, DashboardSource};
/// use serde_json::json;
///
/// let s: DashboardSource = serde_json::from_value(json!({
///     "capability": "health.report",
///     "input": {},
///     "pointer": "/status",
///     "measure": "exact",
/// })).unwrap();
/// assert_eq!(s.measure, DashboardMeasure::Exact);
/// assert_eq!(s.measure.read(&json!({ "status": "ok" }), &s.pointer), json!("ok"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DashboardSource {
    /// The canonical id of the capability the value was read from.
    pub capability: String,
    /// The input it was asked with, as its input schema takes it.
    pub input: Value,
    /// An RFC 6901 JSON pointer into that capability's answer.
    pub pointer: String,
    /// How the value is read at the pointer.
    pub measure: DashboardMeasure,
}

/// One fact on a dashboard: a value read out of one capability's answer, the verdict the
/// source gives it, where a reader drills in, and what acts on it.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::DashboardCard;
/// use serde_json::json;
///
/// let card: DashboardCard = serde_json::from_value(json!({
///     "id": "health-status",
///     "title": "Health",
///     "value": "ok",
///     "status": "ok",
///     "detail": "the worst of 8 check(s)",
///     "source": { "capability": "health.report", "input": {}, "pointer": "/status", "measure": "exact" },
///     "route": "/cockpit/health",
/// })).unwrap();
/// // the value is the source's: reading the pointer out of its answer gives it back
/// assert_eq!(card.source.measure.read(&json!({ "status": "ok" }), &card.source.pointer), card.value);
/// assert!(card.action.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DashboardCard {
    /// A stable id, `[a-z][a-z0-9-]*`, unique across the dashboard.
    pub id: String,
    /// What the value is.
    pub title: String,
    /// The value, read out of the source's answer by the source's measure: a number, a
    /// string or a boolean, or `null` when the source holds none.
    pub value: Value,
    /// The source's verdict on it; `unknown` when the source could not answer.
    pub status: HealthStatus,
    /// One line: what the value means, or why the source could not answer.
    pub detail: String,
    /// Where the value came from.
    pub source: DashboardSource,
    /// The Cockpit page a reader drills into for the evidence.
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The command that acts on it, when one does.
    pub action: Option<String>,
}

/// One of the four questions the Overview answers, with the cards that answer it.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::DashboardQuestion;
/// use majordomus_cli::capability::builtin::HealthStatus;
/// use serde_json::json;
///
/// let q: DashboardQuestion = serde_json::from_value(json!({
///     "id": "healthy", "title": "Is it healthy?", "status": "ok", "cards": [],
/// })).unwrap();
/// assert_eq!(q.status, HealthStatus::Ok);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DashboardQuestion {
    /// `healthy`, `changed`, `broken` or `action`.
    pub id: String,
    /// The question, as a reader asks it.
    pub title: String,
    /// The worst of its cards.
    pub status: HealthStatus,
    /// The facts that answer it, each read from a capability.
    pub cards: Vec<DashboardCard>,
}

/// The Overview: four questions, each answered by facts other capabilities hold.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::DashboardOverview;
/// use majordomus_cli::capability::builtin::HealthStatus;
/// use serde_json::json;
///
/// let o: DashboardOverview = serde_json::from_value(json!({ "status": "unknown", "questions": [] })).unwrap();
/// assert_eq!(o.status, HealthStatus::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DashboardOverview {
    /// The worst of the questions: `unknown` outranks `fail`, because an undecided fact is
    /// not a healthy one.
    pub status: HealthStatus,
    /// `healthy`, `changed`, `broken`, `action`, in that order.
    pub questions: Vec<DashboardQuestion>,
}

// ---------------------------------------------------------------- the cards

/// A verdict and a line of detail, from the card's value and the source's whole answer.
type Judge = fn(&Value, &Value) -> (HealthStatus, String);

/// One card, declared: which question it answers, where its value is, and how the source's
/// own verdict on it is worded. `pointers` is tried in order and the first that resolves
/// is the one the card carries — `plan.next` answers with an issue or with a reason.
struct Spec {
    question: &'static str,
    id: &'static str,
    title: &'static str,
    capability: &'static str,
    input: &'static str,
    pointers: &'static [&'static str],
    measure: DashboardMeasure,
    route: &'static str,
    action: Option<&'static str>,
    judge: Judge,
}

const QUESTIONS: [(&str, &str); 4] = [
    ("healthy", "Is it healthy?"),
    ("changed", "What changed?"),
    ("broken", "What is broken?"),
    ("action", "What needs action?"),
];

fn count(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

fn some_or(v: &Value, none: HealthStatus, some: HealthStatus) -> HealthStatus {
    if count(v) > 0 {
        some
    } else {
        none
    }
}

fn health_status(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let status = serde_json::from_value::<HealthStatus>(v.clone()).unwrap_or(HealthStatus::Unknown);
    let checks = answer["checks"].as_array().cloned().unwrap_or_default();
    let not_ok: Vec<String> = checks
        .iter()
        .filter(|c| c["status"] != "ok")
        .map(|c| {
            format!(
                "{} ({})",
                c["id"].as_str().unwrap_or("?"),
                c["status"].as_str().unwrap_or("?")
            )
        })
        .collect();
    let detail = if not_ok.is_empty() {
        format!("the worst of {} check(s); every one is ok", checks.len())
    } else {
        format!(
            "the worst of {} check(s); not ok: {}",
            checks.len(),
            not_ok.join(", ")
        )
    };
    (status, detail)
}

fn health_failing(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let failing: Vec<&str> = answer["checks"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["status"] == "fail")
                .filter_map(|c| c["id"].as_str())
                .collect()
        })
        .unwrap_or_default();
    let detail = if failing.is_empty() {
        "no check's engine reports a failure".to_string()
    } else {
        format!("failing: {}", failing.join(", "))
    };
    (some_or(v, HealthStatus::Ok, HealthStatus::Fail), detail)
}

fn version_declared(_: &Value, answer: &Value) -> (HealthStatus, String) {
    let tool = answer["tool"].as_str().unwrap_or("?");
    if answer["agree"].as_bool() == Some(true) {
        (
            HealthStatus::Ok,
            format!("authored in Cargo.toml; share/version.txt states {tool}, current"),
        )
    } else {
        (
            HealthStatus::Fail,
            format!("share/version.txt states {tool}, which `generate --check` refuses"),
        )
    }
}

fn last_release(v: &Value, _: &Value) -> (HealthStatus, String) {
    match v.as_str() {
        Some(r) => (
            HealthStatus::Ok,
            format!("the layer records {r} as released"),
        ),
        None => (
            HealthStatus::Ok,
            "the layer records no release yet".to_string(),
        ),
    }
}

fn pending_bump(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let n = answer["changes"].as_array().map(Vec::len).unwrap_or(0);
    match v.as_str() {
        Some("none") | None => (
            HealthStatus::Ok,
            "no commit since the last release implies a release".to_string(),
        ),
        Some(bump) => (
            HealthStatus::Warn,
            format!("{n} kind(s) of change since the last release imply a {bump} release"),
        ),
    }
}

fn next_version(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let bump = answer["bump"].as_str().unwrap_or("?");
    match (v.as_str(), answer["decided_by"].as_str()) {
        (Some(_), Some("contract")) => (
            HealthStatus::Ok,
            format!(
                "the version the public contract requires: the declared one when it already \
                 satisfies the contract, otherwise the smallest it allows; the commits imply a \
                 {bump} (evidence)"
            ),
        ),
        (Some(_), Some("contract_and_commits")) => (
            HealthStatus::Ok,
            format!(
                "the contract requires no release over the last one and the commits imply a \
                 {bump}: the smallest release above it, a patch"
            ),
        ),
        (_, Some("undecided")) => (
            HealthStatus::Unknown,
            format!(
                "undecided: the public contract could not be measured ({}), so no version is \
                 selected; the commits imply a {bump} (evidence only), and `release bump` needs \
                 an explicit target",
                answer["contract_unreadable"]
                    .as_str()
                    .unwrap_or("no reason given")
            ),
        ),
        (Some(_), _) => (
            HealthStatus::Ok,
            "the version the public contract requires".to_string(),
        ),
        (None, _) => (HealthStatus::Ok, "nothing would be released".to_string()),
    }
}

fn blocked_issues(v: &Value, answer: &Value) -> (HealthStatus, String) {
    if v.is_null() {
        return (
            HealthStatus::Unknown,
            "the plan's vocabulary declares no BLOCKED status".to_string(),
        );
    }
    (
        some_or(v, HealthStatus::Ok, HealthStatus::Warn),
        format!(
            "of {} issue(s), waiting on a dependency that is not done",
            answer["counts"]["total"].as_u64().unwrap_or(0)
        ),
    )
}

fn repository_errors(v: &Value, _: &Value) -> (HealthStatus, String) {
    (
        some_or(v, HealthStatus::Ok, HealthStatus::Fail),
        "error-level diagnostics across the whole worktree topology".to_string(),
    )
}

fn canonical(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let path = answer["worktree"]["path"].as_str().unwrap_or("?");
    if v.as_bool() == Some(true) {
        (
            HealthStatus::Ok,
            format!("{path} is where its branch belongs"),
        )
    } else {
        (
            HealthStatus::Fail,
            format!("{path} is not where its branch belongs; the guard refuses a commit there"),
        )
    }
}

fn next_issue(v: &Value, answer: &Value) -> (HealthStatus, String) {
    let milestone = answer["active_milestone"].as_str().unwrap_or("?");
    if answer["issue"].is_object() {
        (
            HealthStatus::Ok,
            format!(
                "{} (active milestone {milestone})",
                answer["issue"]["title"]
                    .as_str()
                    .unwrap_or("the next ready issue")
            ),
        )
    } else if v.is_null() {
        (
            HealthStatus::Warn,
            format!("nothing is executable and no reason was given (active milestone {milestone})"),
        )
    } else {
        (
            HealthStatus::Warn,
            format!("nothing is executable (active milestone {milestone})"),
        )
    }
}

fn blockers(v: &Value, _: &Value) -> (HealthStatus, String) {
    (
        some_or(v, HealthStatus::Ok, HealthStatus::Fail),
        "unresolved questions on this branch; every one refuses `finish --outcome completed`"
            .to_string(),
    )
}

fn overlaps(v: &Value, answer: &Value) -> (HealthStatus, String) {
    (
        some_or(v, HealthStatus::Ok, HealthStatus::Warn),
        format!(
            "pairs of workers whose claimed scopes meet, on this checkout's board of {} peer(s)",
            answer["count"].as_u64().unwrap_or(0)
        ),
    )
}

/// Every card of the Overview, in the order it is read.
const CARDS: &[Spec] = &[
    Spec {
        question: "healthy",
        id: "health-status",
        title: "Health of this process",
        capability: "health.report",
        input: "{}",
        pointers: &["/status"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/health",
        action: Some("majordomus doctor"),
        judge: health_status,
    },
    Spec {
        question: "healthy",
        id: "health-failing",
        title: "Failing checks",
        capability: "health.report",
        input: "{}",
        pointers: &["/tallies/fail"],
        measure: DashboardMeasure::Count,
        route: "/cockpit/health",
        action: Some("majordomus doctor"),
        judge: health_failing,
    },
    Spec {
        question: "changed",
        id: "version-declared",
        title: "Declared version",
        capability: "release.version",
        input: "{}",
        pointers: &["/declared"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/release",
        action: Some("majordomus release version"),
        judge: version_declared,
    },
    Spec {
        question: "changed",
        id: "last-release",
        title: "Last release",
        capability: "release.version",
        input: "{}",
        pointers: &["/last_release"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/release",
        action: Some("majordomus release changelog"),
        judge: last_release,
    },
    Spec {
        question: "changed",
        id: "pending-bump",
        title: "Bump the commits imply",
        capability: "release.version",
        input: "{}",
        pointers: &["/bump"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/release",
        action: Some("majordomus release analyze"),
        judge: pending_bump,
    },
    Spec {
        question: "changed",
        id: "next-version",
        title: "Next version",
        capability: "release.version",
        input: "{}",
        pointers: &["/next"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/release",
        action: Some("majordomus release bump"),
        judge: next_version,
    },
    Spec {
        question: "broken",
        id: "blocked-issues",
        title: "Blocked issues",
        capability: "plan.status",
        input: "{}",
        pointers: &["/counts/by_status/BLOCKED"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/capabilities/plan.status",
        action: Some("majordomus plan blocked"),
        judge: blocked_issues,
    },
    Spec {
        question: "broken",
        id: "worktree-errors",
        title: "Worktree topology errors",
        capability: "worktree.status",
        input: "{}",
        pointers: &["/repository_errors"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/worktrees",
        action: Some("majordomus worktree topology"),
        judge: repository_errors,
    },
    Spec {
        question: "broken",
        id: "worktree-canonical",
        title: "This worktree is where it belongs",
        capability: "worktree.status",
        input: "{}",
        pointers: &["/canonical"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/worktrees",
        action: Some("majordomus worktree migrate"),
        judge: canonical,
    },
    Spec {
        question: "action",
        id: "next-issue",
        title: "Next issue",
        capability: "plan.next",
        input: "{}",
        pointers: &["/issue/id", "/reason"],
        measure: DashboardMeasure::Exact,
        route: "/cockpit/capabilities/plan.next",
        action: Some("majordomus plan next"),
        judge: next_issue,
    },
    Spec {
        question: "action",
        id: "open-blockers",
        title: "Questions refusing completion",
        capability: "continuity.state",
        input: "{}",
        pointers: &["/tallies/blockers"],
        measure: DashboardMeasure::Count,
        route: "/cockpit/continuity",
        action: Some("majordomus question list"),
        judge: blockers,
    },
    Spec {
        question: "action",
        id: "peer-overlaps",
        title: "Overlapping claims",
        capability: "peers.list",
        input: r#"{"checkouts":"this"}"#,
        pointers: &["/overlaps"],
        measure: DashboardMeasure::Count,
        route: "/cockpit/capabilities/peers.list",
        action: None,
        judge: overlaps,
    },
];

// ---------------------------------------------------------------- the handler

/// The overview: each source asked once, through the executor, and every card read out of
/// its answer.
fn overview(ctx: &Context, _: Empty) -> Result<DashboardOverview, CapabilityError> {
    // one execution per (capability, input): four cards of `release.version` are one call
    let mut answers: BTreeMap<(&str, &str), Result<Value, String>> = BTreeMap::new();
    for spec in CARDS {
        answers
            .entry((spec.capability, spec.input))
            .or_insert_with(|| {
                let input: Value = serde_json::from_str(spec.input)
                    .map_err(|e| format!("the card's own input does not parse: {e}"))?;
                let answer = ctx
                    .execute(spec.capability, input)
                    .map_err(|e| e.to_string());
                ctx.progress
                    .step_done(spec.capability, answer.is_ok(), None);
                answer
            });
    }

    let questions: Vec<DashboardQuestion> = QUESTIONS
        .iter()
        .map(|(id, title)| {
            let cards: Vec<DashboardCard> = CARDS
                .iter()
                .filter(|s| s.question == *id)
                .map(|spec| card(spec, &answers[&(spec.capability, spec.input)]))
                .collect();
            DashboardQuestion {
                id: (*id).into(),
                title: (*title).into(),
                status: cards
                    .iter()
                    .fold(HealthStatus::Ok, |acc, c| acc.worse(c.status)),
                cards,
            }
        })
        .collect();
    Ok(DashboardOverview {
        status: questions
            .iter()
            .fold(HealthStatus::Ok, |acc, q| acc.worse(q.status)),
        questions,
    })
}

/// One card from its declaration and its source's answer — or, when the source could not
/// answer, an `unknown` card that says why. Never a pass in place of an answer.
fn card(spec: &Spec, answer: &Result<Value, String>) -> DashboardCard {
    let input: Value = serde_json::from_str(spec.input).unwrap_or(Value::Null);
    let mut source = DashboardSource {
        capability: spec.capability.into(),
        input,
        pointer: spec.pointers[0].into(),
        measure: spec.measure,
    };
    let (value, status, detail) = match answer {
        Ok(answer) => {
            if let Some(p) = spec.pointers.iter().find(|p| answer.pointer(p).is_some()) {
                source.pointer = (*p).into();
            }
            let value = spec.measure.read(answer, &source.pointer);
            let (status, detail) = (spec.judge)(&value, answer);
            (value, status, detail)
        }
        Err(reason) => (
            Value::Null,
            HealthStatus::Unknown,
            format!(
                "{} could not answer, so this is unknown rather than a pass: {reason}",
                spec.capability
            ),
        ),
    };
    DashboardCard {
        id: spec.id.into(),
        title: spec.title.into(),
        value,
        status,
        detail,
        source,
        route: spec.route.into(),
        action: spec.action.map(str::to_string),
    }
}

// ---------------------------------------------------------------- the module

/// The `dashboard` module: the Dashboard Suite's pages, each a projection of the
/// capabilities that hold its facts. The Overview is the first.
///
/// ```
/// use majordomus_cli::capability::builtin::dashboard::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "dashboard");
/// let tool = m.capabilities[0].capability.exposure.mcp.as_ref().and_then(|m| m.tool.clone());
/// assert_eq!(tool.as_deref(), Some("majordomus_dashboard_overview"));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "dashboard",
        title: "Dashboards",
        description: "The Dashboard Suite as a projection of the canonical state: every card is one fact read out of an existing capability's answer, carrying the capability it asked, the input, the JSON pointer its value was read from, the Cockpit page that holds the evidence and the command that acts on it. No dashboard stores, counts or judges anything of its own (ADR 0088).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "dashboard.overview",
                title: "The Overview: four questions",
                description: "Is it healthy, what changed, what is broken, what needs action — each answered by cards read out of `health.report`, `release.version`, `plan.status`, `worktree.status`, `plan.next`, `continuity.state` and `peers.list` (this checkout's board), asked once each through the executor. Every card carries its source capability, input and RFC 6901 pointer, so asking the source the same question yields the same value; a source that cannot answer makes its cards `unknown`, never `ok`. Nothing here rebuilds canonical state or asks anything beyond this process and its checkout.",
                input: Empty,
                output: DashboardOverview,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_dashboard_overview"),
                    http: get("/api/v1/dashboard/overview"),
                    cli: Some(CliExposure { path: vec!["dashboard".into(), "overview".into()] }),
                },
                tags: ["dashboard", "overview", "introspection"],
                // every source carries its own cache policy; caching the composition would
                // serve a card older than the source a reader drills into
                cache: CachePolicy::Disabled,
                handler: overview,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    /// The next-version card names who answered: the contract, or the contract with the
    /// commits — and, when the contract cannot be measured, that no one did: the commit
    /// inference is never passed off as the answer.
    #[test]
    fn the_next_version_card_names_who_answered() {
        let v = Value::from("0.11.0");
        let by = |decided_by: &str| {
            next_version(
                &v,
                &serde_json::json!({ "bump": "major", "decided_by": decided_by }),
            )
        };
        let (status, contract) = by("contract");
        assert_eq!(status, HealthStatus::Ok);
        assert!(contract.starts_with("the version the public contract requires"));
        assert!(contract.contains("the commits imply a major (evidence)"));
        let (_, both) = by("contract_and_commits");
        assert!(both.starts_with("the contract requires no release over the last one"));
        // unmeasurable: no version is selected, and the card neither passes the commit
        // inference off as the answer nor reports `ok`
        let (status, undecided) = next_version(
            &Value::Null,
            &serde_json::json!({
                "bump": "major",
                "decided_by": "undecided",
                "contract_unreadable": "no release to compare with",
            }),
        );
        assert_eq!(status, HealthStatus::Unknown);
        assert!(undecided.starts_with("undecided: the public contract could not be measured"));
        assert!(undecided.contains("no release to compare with"));
        assert!(undecided.contains("needs an explicit target"));
        let (_, nothing) = next_version(
            &Value::Null,
            &serde_json::json!({ "decided_by": "contract" }),
        );
        assert_eq!(nothing, "nothing would be released");
    }

    /// The declaration is the only place the projections are named; a refactor that
    /// dropped one would still compile, and this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["dashboard.overview"]);
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(
            exposure.http.as_ref().unwrap().path,
            "/api/v1/dashboard/overview"
        );
        assert_eq!(
            exposure.cli.as_ref().unwrap().path,
            ["dashboard", "overview"]
        );
    }

    /// Every card answers one of the four questions, has an id of its own, and names a
    /// source that is not one of the expensive or refusing capabilities.
    #[test]
    fn every_card_is_declared_once_and_reads_a_cheap_source() {
        let mut seen = std::collections::BTreeSet::new();
        for spec in CARDS {
            assert!(seen.insert(spec.id), "{} is declared twice", spec.id);
            assert!(
                QUESTIONS.iter().any(|(q, _)| *q == spec.question),
                "{} answers no question",
                spec.id
            );
            assert!(!spec.pointers.is_empty(), "{} reads nowhere", spec.id);
            for p in spec.pointers {
                assert!(p.starts_with('/'), "{}: {p} is not a JSON pointer", spec.id);
            }
            serde_json::from_str::<Value>(spec.input)
                .unwrap_or_else(|e| panic!("{}: input does not parse: {e}", spec.id));
            for refused in ["delivery.", "release.analysis", "evidence."] {
                assert!(
                    !spec.capability.starts_with(refused),
                    "{} reads {}, which the Overview must not ask",
                    spec.id,
                    spec.capability
                );
            }
        }
        for (q, _) in QUESTIONS {
            assert!(
                CARDS.iter().any(|s| s.question == q),
                "the question {q} has no card"
            );
        }
    }

    /// Against a real registry: every source exists, every card's value is its source's
    /// value at its pointer, and a source that could not answer is unknown — never ok.
    #[test]
    fn every_card_is_its_sources_value_or_unknown() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        for spec in CARDS {
            assert!(
                ctx.registry.get(spec.capability).is_some(),
                "{} reads {}, which the registry does not hold",
                spec.id,
                spec.capability
            );
        }
        let o = overview(&ctx, Empty {}).expect("the overview answers");
        let ids: Vec<&str> = o.questions.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["healthy", "changed", "broken", "action"]);
        for card in o.questions.iter().flat_map(|q| &q.cards) {
            match ctx.execute(&card.source.capability, card.source.input.clone()) {
                Ok(answer) => assert_eq!(
                    card.source.measure.read(&answer, &card.source.pointer),
                    card.value,
                    "{} is not its source's value",
                    card.id
                ),
                Err(_) => {
                    assert_eq!(card.status, HealthStatus::Unknown, "{}", card.id);
                    assert!(card.value.is_null(), "{}", card.id);
                }
            }
        }
    }

    /// A source that refuses turns its cards unknown with the reason, and the whole
    /// overview with them: absence of an answer is never reported as a pass.
    #[test]
    fn an_unanswered_source_is_unknown_with_its_reason() {
        let spec = &CARDS[0];
        let c = card(spec, &Err("no such thing".into()));
        assert_eq!(c.status, HealthStatus::Unknown);
        assert!(c.value.is_null());
        assert!(c.detail.contains("no such thing"), "{}", c.detail);
        assert!(
            c.detail.contains("unknown rather than a pass"),
            "{}",
            c.detail
        );
        assert_eq!(c.source.capability, spec.capability);
    }

    /// The fallback pointer is taken only when the first resolves to nothing, and the card
    /// names the pointer it actually read.
    #[test]
    fn a_card_names_the_pointer_it_read() {
        let spec = CARDS
            .iter()
            .find(|s| s.id == "next-issue")
            .expect("declared");
        let with = card(
            spec,
            &Ok(json!({ "issue": { "id": "I0001" }, "active_milestone": "M1" })),
        );
        assert_eq!(with.source.pointer, "/issue/id");
        assert_eq!(with.value, json!("I0001"));
        assert_eq!(with.status, HealthStatus::Ok);
        let without = card(
            spec,
            &Ok(json!({ "reason": "run plan blocked", "active_milestone": "M1" })),
        );
        assert_eq!(without.source.pointer, "/reason");
        assert_eq!(without.value, json!("run plan blocked"));
        assert_eq!(without.status, HealthStatus::Warn);
    }
}
