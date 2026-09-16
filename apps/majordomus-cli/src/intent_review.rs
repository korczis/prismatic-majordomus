//! The two planning records an intent is held to before execution: the gap and the critique.
//!
//! Majordomus does not reason about what is missing or what a plan overlooked. A worker does —
//! a person, Claude, Codex, Gemini — and writes what it found into a typed record, so that
//! planning starts from a recorded analysis rather than from a prompt, and so that the
//! analysis can be refused (ADR 0073):
//!
//! - a **gap** (`.ai/repo/project/gaps/<intent>.yaml`, `majordomus.gap/v1`) states, at one
//!   commit, the observations made of the repository, the state of every criterion of the
//!   intent against them (`satisfied`, `missing`, `conflicting` or `unknown`), the risks, and
//!   the work it suggests;
//! - a **critique** (`.ai/repo/project/critiques/<intent>.yaml`, `majordomus.critique/v1`)
//!   records the adversarial pass over the plan: each finding has a class, a subject, whether
//!   it blocks execution, and a resolution — `open`, `planned` into an issue that serves the
//!   intent, or `rejected` with a reason.
//!
//! What this module decides is only whether the records hold together and whether the plan
//! honours them. A gap must answer every criterion and cite an observation for every answer
//! that is not `unknown`. A criterion the gap observed `satisfied` needs no planned work; every
//! other criterion still does (that is the coverage's question). A finding planned into an
//! issue must name an issue that exists and serves the intent. And no issue serving an intent
//! may have started while the intent has no critique, or while a blocking finding is open.
//!
//! Every function here is pure over parsed records, the intent outlines and the plan.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::index::Index;
use crate::intent::{IntentFinding, FAIL, REPRODUCE, WARN};
use crate::intent_plan::IntentOutline;
use crate::plan::Plan;

/// The kind of a gap record.
pub const GAP: &str = "gap";
/// The kind of a critique record.
pub const CRITIQUE: &str = "critique";

// ---------------------------------------------------------------- the records as authored

/// The state of one criterion as the gap observed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConditionState {
    /// Already true at the observed commit.
    Satisfied,
    /// Not true; work is needed.
    Missing,
    /// Something in the repository contradicts it.
    Conflicting,
    /// Not determined. Never read as satisfied.
    Unknown,
}

impl ConditionState {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "satisfied" => Self::Satisfied,
            "missing" => Self::Missing,
            "conflicting" => Self::Conflicting,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

/// One observation of the current state.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct GapObservation {
    /// Unique within the gap.
    pub id: String,
    /// What was observed.
    pub statement: String,
    /// Where: `file:<path>`, `command:<cmd>`, `test:<path>`, `url:<url>`.
    pub source: String,
}

/// One criterion's state against the observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GapCondition {
    /// The criterion id of the intent.
    pub criterion: String,
    /// The state, or `None` when the record spells one this module does not know.
    pub state: Option<ConditionState>,
    /// As written, so a finding can quote an unknown state.
    pub state_text: String,
    /// Why.
    pub because: String,
    /// The observation ids it rests on.
    pub observations: Vec<String>,
}

/// One suggested work item.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct GapWork {
    /// Unique within the gap.
    pub id: String,
    /// One line.
    pub title: String,
    /// The criterion ids it would make true.
    pub serves: Vec<String>,
    /// The issue it became, once planned.
    pub issue: String,
}

/// A gap record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GapRecord {
    /// The intent it analyses; also the file name.
    pub intent: String,
    /// Repository-relative path.
    pub source: String,
    /// The commit the observations were made on.
    pub observed_at: String,
    /// Who or what recorded it.
    pub recorded_by: String,
    /// The observations.
    pub observations: Vec<GapObservation>,
    /// One per criterion.
    pub conditions: Vec<GapCondition>,
    /// Risks, as prose.
    pub risks: Vec<String>,
    /// Suggested work.
    pub work: Vec<GapWork>,
}

/// The class of a critique finding: the question the adversarial pass asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueClass {
    /// Does the plan miss something the intent requires?
    MissedRequirement,
    /// Which assumption is unproven?
    UnprovenAssumption,
    /// Which criterion lacks sufficient work?
    InsufficientWork,
    /// Which work item is unnecessary?
    UnnecessaryWork,
    /// What could regress?
    RegressionRisk,
    /// Which related surface must also change?
    SurfaceMissing,
    /// Which deployment or runtime verification is required?
    DeliveryVerification,
}

impl CritiqueClass {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "missed_requirement" => Self::MissedRequirement,
            "unproven_assumption" => Self::UnprovenAssumption,
            "insufficient_work" => Self::InsufficientWork,
            "unnecessary_work" => Self::UnnecessaryWork,
            "regression_risk" => Self::RegressionRisk,
            "surface_missing" => Self::SurfaceMissing,
            "delivery_verification" => Self::DeliveryVerification,
            _ => return None,
        })
    }
}

/// How a critique finding was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionState {
    /// Not yet resolved.
    Open,
    /// Carried by an issue.
    Planned,
    /// Judged not to apply, with a reason.
    Rejected,
}

impl ResolutionState {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "open" => Self::Open,
            "planned" => Self::Planned,
            "rejected" => Self::Rejected,
            _ => return None,
        })
    }
}

/// One critique finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CritiqueFinding {
    /// Unique within the critique.
    pub id: String,
    /// The class, or `None` for a class this module does not know.
    pub class: Option<CritiqueClass>,
    /// As written.
    pub class_text: String,
    /// `<intent>#<criterion>`, an issue id or a milestone id.
    pub subject: String,
    /// The finding.
    pub finding: String,
    /// Whether execution may not start while it is open.
    pub blocking: bool,
    /// The resolution, or `None` for a state this module does not know.
    pub resolution: Option<ResolutionState>,
    /// As written.
    pub resolution_text: String,
    /// The issue that carries it, when planned.
    pub issue: String,
    /// Why, when rejected.
    pub because: String,
}

/// A critique record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CritiqueRecord {
    /// The intent whose plan it reviews; also the file name.
    pub intent: String,
    /// Repository-relative path.
    pub source: String,
    /// The commit the plan was reviewed at.
    pub reviewed_at: String,
    /// Who or what reviewed it.
    pub reviewed_by: String,
    /// The findings.
    pub findings: Vec<CritiqueFinding>,
}

fn text(meta: &Value, key: &str) -> String {
    match meta.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn texts(meta: &Value, key: &str) -> Vec<String> {
    match meta.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn items<'a>(meta: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    meta.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

impl GapRecord {
    /// The record an object of kind `gap` carries.
    ///
    /// ```
    /// use majordomus_cli::intent_review::{ConditionState, GapRecord};
    /// let g = GapRecord::from_metadata(".ai/repo/project/gaps/x.yaml", &serde_json::json!({
    ///     "intent": "x", "observed_at": "abc",
    ///     "conditions": [{"criterion": "c", "state": "missing", "observations": ["o1"]}]
    /// }));
    /// assert_eq!(g.conditions[0].state, Some(ConditionState::Missing));
    /// ```
    pub fn from_metadata(source: &str, meta: &Value) -> GapRecord {
        GapRecord {
            intent: text(meta, "intent"),
            source: source.to_string(),
            observed_at: text(meta, "observed_at"),
            recorded_by: text(meta, "recorded_by"),
            observations: items(meta, "observations")
                .map(|o| GapObservation {
                    id: text(o, "id"),
                    statement: text(o, "statement"),
                    source: text(o, "source"),
                })
                .collect(),
            conditions: items(meta, "conditions")
                .map(|c| GapCondition {
                    criterion: text(c, "criterion"),
                    state: ConditionState::parse(&text(c, "state")),
                    state_text: text(c, "state"),
                    because: text(c, "because"),
                    observations: texts(c, "observations"),
                })
                .collect(),
            risks: texts(meta, "risks"),
            work: items(meta, "work")
                .map(|w| GapWork {
                    id: text(w, "id"),
                    title: text(w, "title"),
                    serves: texts(w, "serves"),
                    issue: text(w, "issue"),
                })
                .collect(),
        }
    }
}

/// Every object of `kind` in an index, read by `read`, in identity order.
fn all_of<T>(index: &Index, kind: &str, read: fn(&str, &Value) -> T) -> Vec<T> {
    let by_id: BTreeMap<&str, T> = index
        .objects
        .iter()
        .filter(|o| o.kind == kind)
        .map(|o| (o.identity.as_str(), read(&o.provenance.path, &o.metadata)))
        .collect();
    by_id.into_values().collect()
}

impl GapRecord {
    /// Every gap record of an index, in identity order.
    pub fn all(index: &Index) -> Vec<GapRecord> {
        all_of(index, GAP, GapRecord::from_metadata)
    }
}

impl CritiqueRecord {
    /// Every critique record of an index, in identity order.
    pub fn all(index: &Index) -> Vec<CritiqueRecord> {
        all_of(index, CRITIQUE, CritiqueRecord::from_metadata)
    }
}

impl CritiqueRecord {
    /// The record an object of kind `critique` carries.
    ///
    /// ```
    /// use majordomus_cli::intent_review::{CritiqueRecord, ResolutionState};
    /// let c = CritiqueRecord::from_metadata(".ai/repo/project/critiques/x.yaml", &serde_json::json!({
    ///     "intent": "x", "findings": [{"id": "f1", "class": "regression_risk",
    ///         "subject": "x#c", "blocking": true, "resolution": {"state": "open"}}]
    /// }));
    /// assert!(c.findings[0].blocking);
    /// assert_eq!(c.findings[0].resolution, Some(ResolutionState::Open));
    /// ```
    pub fn from_metadata(source: &str, meta: &Value) -> CritiqueRecord {
        CritiqueRecord {
            intent: text(meta, "intent"),
            source: source.to_string(),
            reviewed_at: text(meta, "reviewed_at"),
            reviewed_by: text(meta, "reviewed_by"),
            findings: items(meta, "findings")
                .map(|f| {
                    let res = f.get("resolution").cloned().unwrap_or(Value::Null);
                    CritiqueFinding {
                        id: text(f, "id"),
                        class: CritiqueClass::parse(&text(f, "class")),
                        class_text: text(f, "class"),
                        subject: text(f, "subject"),
                        finding: text(f, "finding"),
                        blocking: f.get("blocking").and_then(Value::as_bool).unwrap_or(false),
                        resolution: ResolutionState::parse(&text(&res, "state")),
                        resolution_text: text(&res, "state"),
                        issue: text(&res, "issue"),
                        because: text(&res, "because"),
                    }
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------- the judgement

struct Findings(Vec<IntentFinding>);

impl Findings {
    fn push(&mut self, level: &str, code: &str, subject: &str, message: String) {
        self.0.push(IntentFinding {
            level: level.into(),
            code: code.into(),
            subject: subject.into(),
            message,
            reproduce: REPRODUCE.into(),
        });
    }
}

/// The criteria a gap observed already satisfied, by intent: the ones coverage does not ask
/// work of. A condition counts only if it is `satisfied` and cites at least one observation
/// the gap declares.
///
/// ```
/// use majordomus_cli::intent_review::{observed_satisfied, GapRecord};
/// let g = GapRecord::from_metadata("g", &serde_json::json!({"intent": "x",
///     "observations": [{"id": "o1", "statement": "s", "source": "file:a"}],
///     "conditions": [{"criterion": "a", "state": "satisfied", "observations": ["o1"]},
///                    {"criterion": "b", "state": "satisfied", "observations": []}]}));
/// assert_eq!(observed_satisfied(&[g])["x"], ["a"]);
/// ```
pub fn observed_satisfied(gaps: &[GapRecord]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for g in gaps {
        let known: BTreeSet<&str> = g.observations.iter().map(|o| o.id.as_str()).collect();
        for c in &g.conditions {
            if c.state == Some(ConditionState::Satisfied)
                && !c.observations.is_empty()
                && c.observations.iter().all(|o| known.contains(o.as_str()))
            {
                out.entry(g.intent.clone())
                    .or_default()
                    .push(c.criterion.clone());
            }
        }
    }
    out
}

/// The findings of the gap and critique records against the intents and the plan.
pub fn review(
    intents: &[IntentOutline],
    plan: &Plan,
    gaps: &[GapRecord],
    critiques: &[CritiqueRecord],
) -> Vec<IntentFinding> {
    let mut f = Findings(Vec::new());
    let by_id: BTreeMap<&str, &IntentOutline> =
        intents.iter().map(|i| (i.id.as_str(), i)).collect();

    // --- the gap holds together, and answers every criterion
    for g in gaps {
        let Some(intent) = by_id.get(g.intent.as_str()) else {
            f.push(
                FAIL,
                "gap_unknown_intent",
                &g.source,
                format!("analyses intent {}, which does not exist", g.intent),
            );
            continue;
        };
        if g.observed_at.is_empty() {
            f.push(
                FAIL,
                "gap_without_commit",
                &g.intent,
                "names no observed_at commit; an observation of no tree cannot be checked".into(),
            );
        }
        let known: BTreeSet<&str> = g.observations.iter().map(|o| o.id.as_str()).collect();
        for o in &g.observations {
            if o.source.is_empty() {
                f.push(
                    FAIL,
                    "observation_without_source",
                    &format!("{}:{}", g.intent, o.id),
                    "names no source it was observed in".into(),
                );
            }
        }
        let mut answered: BTreeSet<&str> = BTreeSet::new();
        for c in &g.conditions {
            let subject = format!("{}#{}", g.intent, c.criterion);
            if !intent.criteria.contains(&c.criterion) {
                f.push(
                    FAIL,
                    "gap_unknown_criterion",
                    &subject,
                    format!(
                        "answers criterion {}, which intent {} does not declare",
                        c.criterion, g.intent
                    ),
                );
                continue;
            }
            if !answered.insert(c.criterion.as_str()) {
                f.push(
                    FAIL,
                    "gap_duplicate_condition",
                    &subject,
                    "is answered more than once".into(),
                );
            }
            match c.state {
                None => f.push(
                    FAIL,
                    "gap_unknown_state",
                    &subject,
                    format!(
                        "has state {:?}; expected satisfied, missing, conflicting or unknown",
                        c.state_text
                    ),
                ),
                Some(ConditionState::Unknown) => {}
                Some(_) if c.observations.is_empty() => f.push(
                    FAIL,
                    "condition_without_observation",
                    &subject,
                    format!("is {} with no observation behind it", c.state_text),
                ),
                Some(_) => {}
            }
            for o in &c.observations {
                if !known.contains(o.as_str()) {
                    f.push(
                        FAIL,
                        "gap_unknown_observation",
                        &subject,
                        format!("cites observation {o}, which the gap does not declare"),
                    );
                }
            }
        }
        for cid in &intent.criteria {
            if !answered.contains(cid.as_str()) {
                f.push(
                    FAIL,
                    "gap_criterion_unanswered",
                    &format!("{}#{cid}", g.intent),
                    "is not answered by the gap; unknown must be written, not left out".into(),
                );
            }
        }
        for w in &g.work {
            let subject = format!("{}:{}", g.intent, w.id);
            if w.serves.is_empty() {
                f.push(
                    FAIL,
                    "gap_work_serves_nothing",
                    &subject,
                    "suggests work that serves no criterion".into(),
                );
            }
            for s in &w.serves {
                if !intent.criteria.contains(s) {
                    f.push(
                        FAIL,
                        "gap_work_unknown_criterion",
                        &subject,
                        format!(
                            "serves criterion {s}, which intent {} does not declare",
                            g.intent
                        ),
                    );
                }
            }
            if !w.issue.is_empty() {
                match plan.issue(&w.issue) {
                    None => f.push(
                        FAIL,
                        "gap_work_unknown_issue",
                        &subject,
                        format!("was planned as {}, which is not an issue", w.issue),
                    ),
                    Some(i)
                        if !w
                            .serves
                            .iter()
                            .any(|s| i.serves.contains(&format!("{}#{s}", g.intent))) =>
                    {
                        f.push(
                            FAIL,
                            "gap_work_issue_serves_other",
                            &subject,
                            format!(
                                "was planned as {}, which serves none of {}",
                                w.issue,
                                w.serves.join(", ")
                            ),
                        )
                    }
                    Some(_) => {}
                }
            }
        }
    }

    // --- the critique holds together, and names issues that carry the intent
    let critiqued: BTreeSet<&str> = critiques.iter().map(|c| c.intent.as_str()).collect();
    let mut open_blocking: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for c in critiques {
        if !by_id.contains_key(c.intent.as_str()) {
            f.push(
                FAIL,
                "critique_unknown_intent",
                &c.source,
                format!("reviews intent {}, which does not exist", c.intent),
            );
            continue;
        }
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for x in &c.findings {
            let subject = format!("{}:{}", c.intent, x.id);
            if !ids.insert(x.id.as_str()) {
                f.push(
                    FAIL,
                    "critique_duplicate_finding",
                    &subject,
                    "is declared more than once".into(),
                );
            }
            if x.class.is_none() {
                f.push(
                    FAIL,
                    "critique_unknown_class",
                    &subject,
                    format!(
                        "has class {:?}, which is not a question the critique asks",
                        x.class_text
                    ),
                );
            }
            match x.resolution {
                None => f.push(
                    FAIL,
                    "critique_unknown_resolution",
                    &subject,
                    format!(
                        "has resolution {:?}; expected open, planned or rejected",
                        x.resolution_text
                    ),
                ),
                Some(ResolutionState::Open) if x.blocking => open_blocking
                    .entry(c.intent.as_str())
                    .or_default()
                    .push(x.id.as_str()),
                Some(ResolutionState::Open) => {}
                Some(ResolutionState::Rejected) if x.because.trim().is_empty() => {
                    f.push(FAIL, "rejected_without_reason", &subject, "is rejected with no reason; a dismissal nobody can read is not a resolution".into());
                }
                Some(ResolutionState::Rejected) => {}
                Some(ResolutionState::Planned) => match plan.issue(&x.issue) {
                    None => f.push(
                        FAIL,
                        "planned_into_unknown_issue",
                        &subject,
                        format!("is planned into {:?}, which is not an issue", x.issue),
                    ),
                    Some(i)
                        if !i
                            .serves
                            .iter()
                            .any(|s| s.split_once('#').is_some_and(|(iid, _)| iid == c.intent)) =>
                    {
                        f.push(
                            FAIL,
                            "planned_into_unrelated_issue",
                            &subject,
                            format!(
                                "is planned into {}, which serves no criterion of {}",
                                x.issue, c.intent
                            ),
                        )
                    }
                    Some(_) => {}
                },
            }
        }
    }

    // --- execution does not start before the plan was critiqued and its blockers resolved
    for intent in intents.iter().filter(|i| !i.retired) {
        let started: Vec<&str> = plan
            .issues
            .iter()
            .filter(|i| matches!(i.status.as_str(), "ACTIVE" | "VERIFY" | "DONE"))
            .filter(|i| {
                i.serves
                    .iter()
                    .any(|s| s.split_once('#').is_some_and(|(iid, _)| iid == intent.id))
            })
            .map(|i| i.id.as_str())
            .collect();
        if started.is_empty() {
            if !critiqued.contains(intent.id.as_str())
                && plan.issues.iter().any(|i| {
                    i.serves
                        .iter()
                        .any(|s| s.starts_with(&format!("{}#", intent.id)))
                })
            {
                f.push(WARN, "plan_not_critiqued", &intent.id, "has planned work but no critique; execution will be refused until one is recorded".into());
            }
            continue;
        }
        if !critiqued.contains(intent.id.as_str()) {
            f.push(
                FAIL,
                "executing_without_critique",
                &intent.id,
                format!(
                    "has started work ({}) but its plan was never critiqued",
                    started.join(", ")
                ),
            );
        }
        if let Some(open) = open_blocking.get(intent.id.as_str()) {
            f.push(
                FAIL,
                "executing_with_open_blocker",
                &intent.id,
                format!(
                    "has started work ({}) while blocking critique finding(s) {} are open",
                    started.join(", "),
                    open.join(", ")
                ),
            );
        }
    }

    f.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanIssue, PlanProject, PlanVocabulary};
    use serde_json::json;

    fn issue(id: &str, serves: &[&str], status: &str) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: "m1".into(),
            status: status.into(),
            wave: 1,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: id.into(),
            slug: String::new(),
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            scope: vec!["src".into()],
            serves: serves.iter().map(|s| (*s).into()).collect(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 1,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    fn plan(issues: Vec<PlanIssue>) -> Plan {
        Plan {
            project: PlanProject {
                name: "p".into(),
                repository: "o/p".into(),
                default_branch: "master".into(),
                active_milestone: String::new(),
            },
            statuses: PlanVocabulary {
                issue: Vec::new(),
                milestone: Vec::new(),
            },
            milestones: Vec::new(),
            issues,
            waves: Vec::new(),
            edges: Vec::new(),
            milestone_edges: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn x() -> Vec<IntentOutline> {
        vec![IntentOutline {
            id: "x".into(),
            criteria: vec!["a".into(), "b".into()],
            milestones: vec!["m1".into()],
            ..Default::default()
        }]
    }

    fn gap(v: serde_json::Value) -> GapRecord {
        GapRecord::from_metadata(".ai/repo/project/gaps/x.yaml", &v)
    }

    fn critique(v: serde_json::Value) -> CritiqueRecord {
        CritiqueRecord::from_metadata(".ai/repo/project/critiques/x.yaml", &v)
    }

    fn codes(f: &[IntentFinding]) -> Vec<(&str, &str)> {
        f.iter()
            .map(|x| (x.code.as_str(), x.subject.as_str()))
            .collect()
    }

    fn good_gap() -> GapRecord {
        gap(json!({
            "intent": "x", "observed_at": "c3f20da", "recorded_by": "claude-code",
            "observations": [{"id": "o1", "statement": "no link exists", "source": "file:src/plan.rs"}],
            "conditions": [
                {"criterion": "a", "state": "missing", "because": "no link", "observations": ["o1"]},
                {"criterion": "b", "state": "unknown", "because": "not measured"}
            ],
            "work": [{"id": "w1", "title": "add the link", "serves": ["a"], "issue": "I1"}]
        }))
    }

    #[test]
    fn a_complete_gap_and_resolved_critique_are_clean() {
        let p = plan(vec![
            issue("I1", &["x#a"], "ACTIVE"),
            issue("I2", &["x#b"], "READY"),
        ]);
        let c = critique(
            json!({"intent": "x", "reviewed_at": "c3f20da", "findings": [
            {"id": "f1", "class": "insufficient_work", "subject": "x#b", "blocking": true,
             "resolution": {"state": "planned", "issue": "I2"}},
            {"id": "f2", "class": "unnecessary_work", "subject": "I1", "blocking": false,
             "resolution": {"state": "rejected", "because": "I1 is the only writer"}}]}),
        );
        let f = review(&x(), &p, &[good_gap()], &[c]);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_gap_must_answer_every_criterion_and_cite_what_it_saw() {
        let g = gap(json!({"intent": "x", "observed_at": "abc",
            "observations": [{"id": "o1", "statement": "s", "source": ""}],
            "conditions": [{"criterion": "a", "state": "satisfied"},
                           {"criterion": "zz", "state": "missing", "observations": ["o1"]},
                           {"criterion": "a", "state": "done", "observations": ["o9"]}]}));
        let f = review(&x(), &plan(vec![]), &[g], &[]);
        let got = codes(&f);
        for want in [
            ("observation_without_source", "x:o1"),
            ("condition_without_observation", "x#a"),
            ("gap_unknown_criterion", "x#zz"),
            ("gap_duplicate_condition", "x#a"),
            ("gap_unknown_state", "x#a"),
            ("gap_unknown_observation", "x#a"),
            ("gap_criterion_unanswered", "x#b"),
        ] {
            assert!(got.contains(&want), "missing {want:?} in {got:?}");
        }
    }

    #[test]
    fn unknown_is_allowed_but_never_read_as_satisfied() {
        let g = good_gap();
        assert!(!observed_satisfied(std::slice::from_ref(&g)).contains_key("x"));
        let f = review(&x(), &plan(vec![issue("I1", &["x#a"], "READY")]), &[g], &[]);
        assert!(
            !codes(&f)
                .iter()
                .any(|(c, _)| c.starts_with("gap_") || c.starts_with("condition_")),
            "{f:?}"
        );
    }

    #[test]
    fn gap_work_must_serve_a_criterion_and_land_in_an_issue_that_serves_it() {
        let g = gap(json!({"intent": "x", "observed_at": "abc",
            "conditions": [{"criterion": "a", "state": "unknown"}, {"criterion": "b", "state": "unknown"}],
            "work": [{"id": "w1", "title": "t", "serves": []},
                     {"id": "w2", "title": "t", "serves": ["q"], "issue": "I404"},
                     {"id": "w3", "title": "t", "serves": ["a"], "issue": "I2"}]}));
        let p = plan(vec![issue("I2", &["x#b"], "READY")]);
        let findings_1 = review(&x(), &p, &[g], &[]);

        let got = codes(&findings_1);
        for want in [
            ("gap_work_serves_nothing", "x:w1"),
            ("gap_work_unknown_criterion", "x:w2"),
            ("gap_work_unknown_issue", "x:w2"),
            ("gap_work_issue_serves_other", "x:w3"),
        ] {
            assert!(got.contains(&want), "missing {want:?} in {got:?}");
        }
    }

    #[test]
    fn a_gap_of_no_intent_or_no_commit_is_refused() {
        let got = review(
            &x(),
            &plan(vec![]),
            &[
                gap(json!({"intent": "nope"})),
                gap(json!({"intent": "x",
            "conditions": [{"criterion": "a", "state": "unknown"}, {"criterion": "b", "state": "unknown"}]})),
            ],
            &[],
        );
        let got = codes(&got);
        assert!(
            got.contains(&("gap_unknown_intent", ".ai/repo/project/gaps/x.yaml")),
            "{got:?}"
        );
        assert!(got.contains(&("gap_without_commit", "x")), "{got:?}");
    }

    #[test]
    fn started_work_without_a_critique_is_refused_and_planned_work_is_warned() {
        let findings_2 = review(&x(), &plan(vec![issue("I1", &["x#a"], "ACTIVE")]), &[], &[]);

        let got = codes(&findings_2);
        assert_eq!(got, [("executing_without_critique", "x")]);
        let findings_3 = review(&x(), &plan(vec![issue("I1", &["x#a"], "READY")]), &[], &[]);

        let got = codes(&findings_3);
        assert_eq!(got, [("plan_not_critiqued", "x")]);
    }

    #[test]
    fn an_open_blocking_finding_refuses_execution_but_not_planning() {
        let c = critique(json!({"intent": "x", "findings": [
            {"id": "f1", "class": "regression_risk", "subject": "x#a", "blocking": true, "resolution": {"state": "open"}},
            {"id": "f2", "class": "surface_missing", "subject": "x#a", "blocking": false, "resolution": {"state": "open"}}]}));
        let ready = review(
            &x(),
            &plan(vec![issue("I1", &["x#a"], "READY")]),
            &[],
            std::slice::from_ref(&c),
        );
        assert!(ready.is_empty(), "{ready:?}");
        let findings_4 = review(
            &x(),
            &plan(vec![issue("I1", &["x#a"], "VERIFY")]),
            &[],
            &[c],
        );

        let active = codes(&findings_4);
        assert_eq!(active, [("executing_with_open_blocker", "x")]);
    }

    #[test]
    fn a_resolution_must_be_readable_and_land_in_the_intents_work() {
        let c = critique(json!({"intent": "x", "findings": [
            {"id": "f1", "class": "vibes", "subject": "x#a", "resolution": {"state": "rejected"}},
            {"id": "f1", "class": "regression_risk", "subject": "x#a", "resolution": {"state": "planned", "issue": "I404"}},
            {"id": "f3", "class": "regression_risk", "subject": "x#a", "resolution": {"state": "planned", "issue": "I9"}},
            {"id": "f4", "class": "regression_risk", "subject": "x#a", "resolution": {"state": "maybe"}}]}));
        let p = plan(vec![issue("I9", &[], "READY")]);
        let findings_5 = review(&x(), &p, &[], &[c, critique(json!({"intent": "ghost"}))]);

        let got = codes(&findings_5);
        for want in [
            ("critique_unknown_class", "x:f1"),
            ("rejected_without_reason", "x:f1"),
            ("critique_duplicate_finding", "x:f1"),
            ("planned_into_unknown_issue", "x:f1"),
            ("planned_into_unrelated_issue", "x:f3"),
            ("critique_unknown_resolution", "x:f4"),
            (
                "critique_unknown_intent",
                ".ai/repo/project/critiques/x.yaml",
            ),
        ] {
            assert!(got.contains(&want), "missing {want:?} in {got:?}");
        }
    }

    #[test]
    fn a_satisfied_condition_counts_only_with_declared_observations() {
        let g = gap(json!({"intent": "x", "observed_at": "abc",
            "observations": [{"id": "o1", "statement": "s", "source": "test:t"}],
            "conditions": [{"criterion": "a", "state": "satisfied", "observations": ["o1"]},
                           {"criterion": "b", "state": "satisfied", "observations": ["o2"]}]}));
        assert_eq!(observed_satisfied(&[g])["x"], ["a"]);
    }
}
