//! The opposition to an intent's plan, executed (ADR 0112).
//!
//! A critique was a record somebody wrote, and the only thing asked of it was that it exist
//! and have no blocking finding open. That proves a file was written. This module is the
//! review as something the tool does:
//!
//! - **the structural half is derived, every time.** Whatever the coverage, the plan and the
//!   gap review report about one intent is that intent's structural opposition: a failure
//!   there is a blocking finding, a warning an advisory one. Nothing is re-derived here and
//!   nothing of it is stored, so a review cannot outlive a plan the checks would now reject.
//! - **the recorded half is what a reviewer found**, kept in the one critique record, each
//!   finding with its resolution.
//! - **one disposition** is derived from both, by [`disposition`], and stored nowhere.
//! - **the stamp** says which plan the review was run over: [`reviewed_plan`] hashes what a
//!   review judges, and a critique carries the value it was stamped with. A stamp that is
//!   not the plan's current revision is a review of another plan.
//!
//! The answer is also the brief: everything a reviewing session needs to challenge the plan
//! — statement, invariants, criteria, the serving issues with their links, the gap's
//! conditions, and what is already found — bounded to one intent.
//!
//! No advisor, model or network is needed for any of it. A stamp is evidence that the
//! command ran and that the record was not edited carelessly afterwards; it is not proof
//! against an author determined to forge one, and nothing here claims otherwise. What makes
//! the review *executed* is that the structural half is derived again at every gate.
//!
//! ```
//! use majordomus_cli::intent_opposition::{disposition, OppositionDisposition, OppositionFinding};
//! // nothing found: the plan is accepted as it stands
//! assert_eq!(disposition(&[], &[]), OppositionDisposition::Accept);
//! let blocking = OppositionFinding::structural("FAIL", "criterion_uncovered", "x#a", "no work");
//! assert_eq!(disposition(&[blocking], &[]), OppositionDisposition::Reject);
//! ```

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::intent::{IntentCriterion, IntentStage, IntentView, Intents};
use crate::intent_review::{CritiqueRecord, GapRecord, ResolutionState};
use crate::plan::{Plan, PlanIssue};
use crate::policy::sha256_hex;

/// The findings of `intent validate` that are about the review itself. They are left out of
/// the structural half: a review that listed "this plan was not reviewed" among its own
/// findings would reject itself for existing.
const ABOUT_THE_REVIEW: [&str; 5] = [
    "plan_not_critiqued",
    "executing_without_critique",
    "executing_with_open_blocker",
    "critique_stale",
    "critique_not_stamped",
];

/// What a review concludes about a plan. Derived from the findings by [`disposition`];
/// never written to a record.
///
/// ```
/// use majordomus_cli::intent_opposition::OppositionDisposition;
/// assert_eq!(
///     serde_json::to_string(&OppositionDisposition::AcceptWithRequiredChanges).unwrap(),
///     "\"accept_with_required_changes\""
/// );
/// assert!(OppositionDisposition::Accept.permits_execution());
/// assert!(!OppositionDisposition::Reject.permits_execution());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OppositionDisposition {
    /// No blocking finding of either half.
    Accept,
    /// A reviewer found something blocking, and each such finding is planned into work or
    /// rejected with a reason: the plan stands with the changes it has taken on.
    AcceptWithRequiredChanges,
    /// A structural finding is blocking, or a recorded blocking finding is still open.
    Reject,
}

impl OppositionDisposition {
    /// The word the command line prints, the ledger event carries and the JSON of every
    /// surface spells: one spelling, so a reader can grep a refusal back to its cause.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionDisposition;
    /// assert_eq!(OppositionDisposition::Reject.as_str(), "reject");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            OppositionDisposition::Accept => "accept",
            OppositionDisposition::AcceptWithRequiredChanges => "accept_with_required_changes",
            OppositionDisposition::Reject => "reject",
        }
    }

    /// Whether work serving the intent may start under this disposition.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionDisposition;
    /// assert!(OppositionDisposition::AcceptWithRequiredChanges.permits_execution());
    /// ```
    pub fn permits_execution(self) -> bool {
        self != OppositionDisposition::Reject
    }
}

/// Where a critique stands against the plan it is about.
///
/// ```
/// use majordomus_cli::intent_opposition::OppositionReviewState;
/// assert_eq!(serde_json::to_string(&OppositionReviewState::NotStamped).unwrap(), "\"not_stamped\"");
/// assert_eq!(OppositionReviewState::judge(None, "abc"), OppositionReviewState::None);
/// assert_eq!(OppositionReviewState::judge(Some(""), "abc"), OppositionReviewState::NotStamped);
/// assert_eq!(OppositionReviewState::judge(Some("abc"), "abc"), OppositionReviewState::Current);
/// assert_eq!(OppositionReviewState::judge(Some("old"), "abc"), OppositionReviewState::Stale);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OppositionReviewState {
    /// No critique record exists for the intent.
    None,
    /// A critique exists and nobody stamped it: it says nothing of which plan it reviewed.
    NotStamped,
    /// The critique was stamped against the plan as it now stands.
    Current,
    /// The critique was stamped against a plan that has since changed.
    Stale,
}

impl OppositionReviewState {
    /// The state of a critique whose stamp is `stamped` (absent when there is no critique)
    /// against the plan's current revision.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionReviewState;
    /// assert_eq!(OppositionReviewState::judge(Some("a"), "b").as_str(), "stale");
    /// ```
    pub fn judge(stamped: Option<&str>, current: &str) -> Self {
        match stamped {
            None => OppositionReviewState::None,
            Some("") => OppositionReviewState::NotStamped,
            Some(s) if s == current => OppositionReviewState::Current,
            Some(_) => OppositionReviewState::Stale,
        }
    }

    /// The word a briefing, a binding and the JSON of every surface use for where a
    /// critique's stamp stands against the plan.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionReviewState;
    /// assert_eq!(OppositionReviewState::Current.as_str(), "current");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            OppositionReviewState::None => "none",
            OppositionReviewState::NotStamped => "not_stamped",
            OppositionReviewState::Current => "current",
            OppositionReviewState::Stale => "stale",
        }
    }
}

/// One finding of the opposition, of either half.
///
/// ```
/// use majordomus_cli::intent_opposition::OppositionFinding;
/// let f = OppositionFinding::structural("WARN", "criterion_weakly_covered", "x#a", "no evidence required");
/// assert_eq!(f.origin, "structural");
/// assert!(!f.blocking, "a warning advises");
/// assert!(!f.is_open_blocker());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OppositionFinding {
    /// `structural` — derived now from the plan — or `recorded` — written by a reviewer.
    pub origin: String,
    /// The code of the derivation that made it, or the reviewer's id for it.
    pub id: String,
    /// The class a reviewer gave it; empty for a structural finding, whose code says it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub class: String,
    /// What it is about: `<intent>#<criterion>`, an issue, a milestone or the intent.
    pub subject: String,
    /// The finding.
    pub finding: String,
    /// Whether it stands in the way of execution.
    pub blocking: bool,
    /// `open`, `planned` or `rejected` for a recorded finding; empty for a structural one,
    /// which is resolved by changing the plan and by nothing a record can say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resolution: String,
    /// The issue that carries it, when planned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issue: String,
    /// Why, when rejected or planned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub because: String,
    /// Who found it, when the record says.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Who resolved it, when the record says.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resolved_by: String,
}

impl OppositionFinding {
    /// A structural finding from a derivation's own finding: blocking when that derivation
    /// calls it a failure.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionFinding;
    /// let f = OppositionFinding::structural("FAIL", "criterion_uncovered", "x#a", "no work");
    /// assert!(f.blocking && f.is_open_blocker());
    /// ```
    pub fn structural(level: &str, code: &str, subject: &str, message: &str) -> Self {
        OppositionFinding {
            origin: "structural".into(),
            id: code.into(),
            class: String::new(),
            subject: subject.into(),
            finding: message.into(),
            blocking: level == "FAIL",
            resolution: String::new(),
            issue: String::new(),
            because: String::new(),
            source: String::new(),
            resolved_by: String::new(),
        }
    }

    /// Whether this finding rejects the plan: blocking, and — for a recorded one — not yet
    /// planned or rejected. A resolution this module cannot read counts as open: an answer
    /// nobody can parse has resolved nothing.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionFinding;
    /// let mut f = OppositionFinding::structural("FAIL", "c", "x", "m");
    /// f.origin = "recorded".into();
    /// f.resolution = "planned".into();
    /// assert!(!f.is_open_blocker());
    /// f.resolution = "open".into();
    /// assert!(f.is_open_blocker());
    /// ```
    pub fn is_open_blocker(&self) -> bool {
        self.blocking
            && (self.origin == "structural"
                || !matches!(self.resolution.as_str(), "planned" | "rejected"))
    }
}

/// The one function that says what a review concludes.
///
/// ```
/// use majordomus_cli::intent_opposition::{disposition, OppositionDisposition, OppositionFinding};
/// let mut found = OppositionFinding::structural("FAIL", "thin", "x#a", "one case is not enough");
/// found.origin = "recorded".into();
/// found.resolution = "open".into();
/// assert_eq!(disposition(&[], &[found.clone()]), OppositionDisposition::Reject);
/// found.resolution = "planned".into();
/// assert_eq!(disposition(&[], &[found]), OppositionDisposition::AcceptWithRequiredChanges);
/// ```
pub fn disposition(
    structural: &[OppositionFinding],
    recorded: &[OppositionFinding],
) -> OppositionDisposition {
    if structural
        .iter()
        .chain(recorded)
        .any(OppositionFinding::is_open_blocker)
    {
        OppositionDisposition::Reject
    } else if recorded.iter().any(|f| f.blocking) {
        OppositionDisposition::AcceptWithRequiredChanges
    } else {
        OppositionDisposition::Accept
    }
}

/// One issue serving the intent, as a reviewer needs it: its links and its place, and
/// nothing of its prose.
///
/// ```
/// use majordomus_cli::intent_opposition::OppositionIssue;
/// let i = OppositionIssue {
///     id: "I0001".into(), milestone: "m".into(), status: "READY".into(),
///     serves: vec!["x#a".into()], depends_on: vec![], scope: vec!["lib".into()],
///     evidence_need: 1, title: "The work".into(),
/// };
/// assert_eq!(i.serves, ["x#a"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OppositionIssue {
    /// The issue.
    pub id: String,
    /// Its milestone.
    pub milestone: String,
    /// Its derived status.
    pub status: String,
    /// The criteria it says it serves.
    pub serves: Vec<String>,
    /// What it depends on.
    pub depends_on: Vec<String>,
    /// Where it may write.
    pub scope: Vec<String>,
    /// How many evidence tokens it requires before it can be DONE.
    pub evidence_need: u32,
    /// One line.
    pub title: String,
}

/// The stamp a critique carries, with where it stands against the plan now.
///
/// ```
/// use majordomus_cli::intent_opposition::{OppositionReview, OppositionReviewState};
/// let none = OppositionReview::absent();
/// assert_eq!(none.state, OppositionReviewState::None);
/// assert!(none.source.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OppositionReview {
    /// `none`, `not_stamped`, `current` or `stale`.
    pub state: OppositionReviewState,
    /// The critique record; empty when there is none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The plan revision it was stamped against.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewed_revision: String,
    /// The commit it was reviewed at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewed_at: String,
    /// Who reviewed it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewed_by: String,
    /// The tool that stamped it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewed_with: String,
}

impl OppositionReview {
    /// The review of an intent nobody has critiqued.
    ///
    /// ```
    /// use majordomus_cli::intent_opposition::OppositionReview;
    /// assert_eq!(OppositionReview::absent().state.as_str(), "none");
    /// ```
    pub fn absent() -> Self {
        OppositionReview {
            state: OppositionReviewState::None,
            source: String::new(),
            reviewed_revision: String::new(),
            reviewed_at: String::new(),
            reviewed_by: String::new(),
            reviewed_with: String::new(),
        }
    }
}

/// The opposition to one intent's plan: the brief a reviewer works from, and the verdict
/// derived from what is found.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_opposition::{oppose, IntentOpposition};
/// use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// // an intent this repository does not hold has no opposition to derive
/// let none: Option<IntentOpposition> = oppose(&intents, &plan, &[], &[], "absent");
/// assert!(none.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentOpposition {
    /// The intent.
    pub intent: String,
    /// One line.
    pub title: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// What must become true.
    pub statement: String,
    /// What must stay true.
    pub invariants: Vec<String>,
    /// What is deliberately not required.
    pub non_goals: Vec<String>,
    /// Every criterion, with the state of its evidence now.
    pub criteria: Vec<IntentCriterion>,
    /// Every live issue serving the intent.
    pub issues: Vec<OppositionIssue>,
    /// The recorded gap's conditions, as `<criterion> <state>`; empty when no gap is recorded.
    pub gap: Vec<String>,
    /// The revision of the plan a review judges: what a stamp is compared with.
    pub reviewed_plan: String,
    /// The critique's stamp and where it stands.
    pub review: OppositionReview,
    /// What the derivations find about this intent's plan now.
    pub structural: Vec<OppositionFinding>,
    /// What a reviewer recorded, each finding with its resolution.
    pub recorded: Vec<OppositionFinding>,
    /// `accept`, `accept_with_required_changes` or `reject`.
    pub disposition: OppositionDisposition,
    /// The findings that reject the plan, as `<origin> <id> <subject>`; empty unless the
    /// disposition is `reject`.
    pub rejecting: Vec<String>,
}

fn serves_intent(i: &PlanIssue, intent: &str) -> bool {
    i.serves
        .iter()
        .filter_map(|t| t.split_once('#'))
        .any(|(iid, _)| iid == intent)
}

fn serving<'a>(plan: &'a Plan, intent: &str) -> Vec<&'a PlanIssue> {
    plan.issues
        .iter()
        .filter(|i| i.status != "CANCELLED" && serves_intent(i, intent))
        .collect()
}

/// The revision of the plan a review judges. It hashes what a reviewer reads to challenge
/// the plan — the statement, the invariants, the non-goals, each criterion with its
/// evidence kind and reference, each live serving issue's own milestone, links,
/// dependencies, scope and required evidence, and the gap's conditions — and nothing that
/// changes without the plan changing: no status, no wave, no title, no critique.
///
/// ```
/// use majordomus_cli::intent_opposition::reviewed_plan;
/// // a function of content: the same inputs are the same revision
/// assert_eq!(reviewed_plan(None, &[], None), reviewed_plan(None, &[], None));
/// assert_eq!(reviewed_plan(None, &[], None).len(), 64);
/// ```
pub fn reviewed_plan(
    intent: Option<&IntentView>,
    issues: &[&PlanIssue],
    gap: Option<&GapRecord>,
) -> String {
    let set = |v: &[String]| v.iter().cloned().collect::<BTreeSet<String>>();
    let intent = intent.map(|i| {
        let criteria: BTreeMap<&str, serde_json::Value> = i
            .satisfaction
            .iter()
            .map(|c| {
                (
                    c.id.as_str(),
                    json!({ "criterion": c.criterion, "evidence": c.evidence, "ref": c.reference }),
                )
            })
            .collect();
        json!({
            "id": i.id,
            "statement": i.statement,
            "invariants": i.invariants,
            "non_goals": i.non_goals,
            "criteria": criteria,
        })
    });
    let issues: BTreeMap<&str, serde_json::Value> = issues
        .iter()
        .map(|i| {
            (
                i.id.as_str(),
                json!({
                    "milestone": i.milestone,
                    "serves": set(&i.serves),
                    "depends_on": set(&i.depends_on),
                    "scope": set(&i.scope),
                    "evidence_need": i.evidence_need,
                }),
            )
        })
        .collect();
    let gap: BTreeMap<&str, &str> = gap
        .map(|g| {
            g.conditions
                .iter()
                .map(|c| (c.criterion.as_str(), c.state_text.as_str()))
                .collect()
        })
        .unwrap_or_default();
    sha256_hex(&json!({ "intent": intent, "issues": issues, "gap": gap }).to_string())
}

/// The plan revision of `intent` as it stands, or `None` when this repository holds no such
/// intent. What a gate compares a critique's stamp with.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_opposition::revision_of;
/// use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// assert!(revision_of(&intents, &plan, &[], "absent").is_none());
/// ```
pub fn revision_of(
    intents: &Intents,
    plan: &Plan,
    gaps: &[GapRecord],
    intent: &str,
) -> Option<String> {
    let view = intents.intent(intent)?;
    Some(reviewed_plan(
        Some(view),
        &serving(plan, intent),
        gaps.iter().find(|g| g.intent == intent),
    ))
}

/// Whether a finding of the intent engine or the plan is about `intent`: its subject is the
/// intent, one of its criteria, one of the issues serving it, or one of its milestones.
fn concerns(subject: &str, view: &IntentView, issues: &[&PlanIssue]) -> bool {
    // a subject may carry a detail after a colon (`x:f1`); the part before it is what it is of
    let head = subject.split(':').next().unwrap_or(subject);
    let of = head.split('#').next().unwrap_or(head);
    of == view.id || issues.iter().any(|i| i.id == of) || view.milestones.iter().any(|m| m.id == of)
}

/// The opposition to `intent`'s plan, or `None` when this repository holds no such intent.
///
/// The structural half is taken from what [`Intents::build`] already derived — the record's
/// own findings, the coverage and the gap and critique review — and from the plan's
/// findings; this function selects the ones about this intent and derives none.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_opposition::oppose;
/// use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// assert!(oppose(&intents, &plan, &[], &[], "absent").is_none());
/// ```
pub fn oppose(
    intents: &Intents,
    plan: &Plan,
    gaps: &[GapRecord],
    critiques: &[CritiqueRecord],
    intent: &str,
) -> Option<IntentOpposition> {
    let view = intents.intent(intent)?;
    let issues = serving(plan, intent);
    let gap = gaps.iter().find(|g| g.intent == intent);
    let critique = critiques.iter().find(|c| c.intent == intent);
    let revision = reviewed_plan(Some(view), &issues, gap);

    // one finding per (code, subject): the intent engine and the plan may each say the same
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut structural = Vec::new();
    let derived = intents
        .findings
        .iter()
        .map(|f| {
            (
                f.level.as_str(),
                f.code.as_str(),
                f.subject.as_str(),
                f.message.as_str(),
            )
        })
        .chain(plan.findings.iter().map(|f| {
            (
                f.level.as_str(),
                f.code.as_str(),
                f.subject.as_str(),
                f.message.as_str(),
            )
        }));
    for (level, code, subject, message) in derived {
        if ABOUT_THE_REVIEW.contains(&code) || !concerns(subject, view, &issues) {
            continue;
        }
        if seen.insert((code.to_string(), subject.to_string())) {
            structural.push(OppositionFinding::structural(level, code, subject, message));
        }
    }

    let recorded: Vec<OppositionFinding> = critique
        .map(|c| {
            c.findings
                .iter()
                .map(|f| OppositionFinding {
                    origin: "recorded".into(),
                    id: f.id.clone(),
                    class: f.class_text.clone(),
                    subject: f.subject.clone(),
                    finding: f.finding.clone(),
                    blocking: f.blocking,
                    resolution: match f.resolution {
                        Some(ResolutionState::Open) => "open".into(),
                        Some(ResolutionState::Planned) => "planned".into(),
                        Some(ResolutionState::Rejected) => "rejected".into(),
                        None => f.resolution_text.clone(),
                    },
                    issue: f.issue.clone(),
                    because: f.because.clone(),
                    source: f.source.clone(),
                    resolved_by: f.resolved_by.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let review = match critique {
        None => OppositionReview::absent(),
        Some(c) => OppositionReview {
            state: OppositionReviewState::judge(Some(&c.reviewed_revision), &revision),
            source: c.source.clone(),
            reviewed_revision: c.reviewed_revision.clone(),
            reviewed_at: c.reviewed_at.clone(),
            reviewed_by: c.reviewed_by.clone(),
            reviewed_with: c.reviewed_with.clone(),
        },
    };
    let disposition = disposition(&structural, &recorded);
    let rejecting = structural
        .iter()
        .chain(&recorded)
        .filter(|f| f.is_open_blocker())
        .map(|f| format!("{} {} {}", f.origin, f.id, f.subject))
        .collect();

    Some(IntentOpposition {
        intent: view.id.clone(),
        title: view.title.clone(),
        stage: view.stage,
        statement: view.statement.clone(),
        invariants: view.invariants.clone(),
        non_goals: view.non_goals.clone(),
        criteria: view.satisfaction.clone(),
        issues: issues
            .iter()
            .map(|i| OppositionIssue {
                id: i.id.clone(),
                milestone: i.milestone.clone(),
                status: i.status.clone(),
                serves: i.serves.clone(),
                depends_on: i.depends_on.clone(),
                scope: i.scope.clone(),
                evidence_need: i.evidence_need,
                title: i.title.clone(),
            })
            .collect(),
        gap: gap
            .map(|g| {
                g.conditions
                    .iter()
                    .map(|c| format!("{} {}", c.criterion, c.state_text))
                    .collect()
            })
            .unwrap_or_default(),
        reviewed_plan: revision,
        review,
        structural,
        recorded,
        disposition,
        rejecting,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(blocking: bool, resolution: &str) -> OppositionFinding {
        OppositionFinding {
            origin: "recorded".into(),
            id: "f".into(),
            class: "insufficient_work".into(),
            subject: "x#a".into(),
            finding: "thin".into(),
            blocking,
            resolution: resolution.into(),
            issue: String::new(),
            because: String::new(),
            source: String::new(),
            resolved_by: String::new(),
        }
    }

    #[test]
    fn a_structural_failure_rejects_whatever_the_record_says() {
        let fail = OppositionFinding::structural("FAIL", "criterion_uncovered", "x#a", "no work");
        assert_eq!(
            disposition(&[fail], &[recorded(true, "planned")]),
            OppositionDisposition::Reject
        );
    }

    #[test]
    fn a_warning_and_an_advisory_finding_reject_nothing() {
        let warn = OppositionFinding::structural("WARN", "criterion_weakly_covered", "x#a", "m");
        assert_eq!(
            disposition(&[warn], &[recorded(false, "open")]),
            OppositionDisposition::Accept
        );
    }

    #[test]
    fn a_resolution_nobody_can_read_has_resolved_nothing() {
        assert_eq!(
            disposition(&[], &[recorded(true, "sort of")]),
            OppositionDisposition::Reject
        );
        assert_eq!(
            disposition(&[], &[recorded(true, "rejected")]),
            OppositionDisposition::AcceptWithRequiredChanges
        );
    }

    #[test]
    fn a_review_is_stale_exactly_when_its_stamp_is_not_the_plans() {
        assert_eq!(
            OppositionReviewState::judge(Some("a"), "a"),
            OppositionReviewState::Current
        );
        assert_eq!(
            OppositionReviewState::judge(Some("a"), "b"),
            OppositionReviewState::Stale
        );
        assert_eq!(
            OppositionReviewState::judge(Some(""), "b"),
            OppositionReviewState::NotStamped
        );
        assert_eq!(
            OppositionReviewState::judge(None, "b"),
            OppositionReviewState::None
        );
    }
}
