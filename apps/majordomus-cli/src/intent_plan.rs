//! Intent coverage: which work makes which intent criterion true, derived from the plan.
//!
//! An intent names the milestones that realise it and the criteria that settle it; an issue
//! names, in `serves`, the `<intent>#<criterion>` it exists to make true. Neither side stores
//! the join. This module derives it on every read and refuses the shapes that mean the plan
//! does not carry the intent (ADR 0072):
//!
//! - a criterion no live issue serves is **uncovered** — nothing will make it true;
//! - an issue under an intent's milestone that serves nothing has **no purpose** under it;
//! - an issue that serves a criterion of an intent whose milestones do not include its own
//!   milestone serves it from **outside** the intent's plan;
//! - a reference to an intent or a criterion that does not exist is a **dangling** link.
//!
//! Three weaker shapes are warnings, because they are sometimes right and always worth a
//! look: a criterion whose every serving issue requires no evidence is only **weakly**
//! covered; a milestone an intent names that serves none of its criteria **contributes
//! nothing** yet; two live issues serving one criterion over overlapping scope are
//! **duplicate work**.
//!
//! An issue under a milestone no intent names is maintenance work. It is reported with that
//! origin, never invented an intent, and never a finding.
//!
//! The derivation is pure over an [`IntentOutline`] per intent and the derived [`Plan`], so a
//! test can walk every branch without an index, and the intent engine converts its own record
//! into an outline rather than this module reading intent files a second way.
//!
//! ```
//! use majordomus_cli::intent_plan::{coverage, CoverageStrength, IntentOutline};
//! use majordomus_cli::plan::Plan;
//! # let plan: Plan = serde_json::from_value(serde_json::json!({
//! #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
//! #                 "active_milestone": ""},
//! #     "statuses": {"issue": [], "milestone": []},
//! #     "milestones": [], "waves": [], "edges": [], "milestone_edges": [], "findings": [],
//! #     "issues": [{"id": "I1", "milestone": "m", "status": "READY", "wave": 1,
//! #         "priority": "p1", "profile": "implementation", "parallel_safe": true,
//! #         "title": "t", "slug": "", "depends_on": [], "blocked_by": [], "dependents": [],
//! #         "scope": ["src"], "serves": ["x#a"], "objective": "", "evidence_have": 0,
//! #         "evidence_need": 1, "started_at": "", "verified_at": "", "completed_at": ""}]
//! # })).unwrap();
//! let intent = IntentOutline {
//!     id: "x".into(),
//!     criteria: vec!["a".into(), "b".into()],
//!     milestones: vec!["m".into()],
//!     ..Default::default()
//! };
//! let covered = coverage(&[intent], &plan);
//! // I1 serves x#a, so that criterion is carried; x#b is not, and that is a failure.
//! assert_eq!(covered.criterion("x", "a").unwrap().strength, CoverageStrength::Covered);
//! assert_eq!(covered.criterion("x", "b").unwrap().strength, CoverageStrength::Uncovered);
//! assert_eq!(covered.failures(), 1);
//! ```

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::intent::{IntentFinding, FAIL, REPRODUCE, WARN};
use crate::plan::{overlap, Plan, PlanIssue};

/// What coverage needs of one intent: its identity, its criterion ids, its milestones, and
/// whether it is retired (cancelled or superseded) and so owes nothing.
///
/// The intent engine builds one of these from each [`crate::intent::IntentRecord`], so the
/// intent files are read one way and this module never reads them a second time.
///
/// ```
/// use majordomus_cli::intent_plan::IntentOutline;
/// let outline = IntentOutline {
///     id: "intent-planning".into(),
///     criteria: vec!["criteria-are-covered".into()],
///     milestones: vec!["intent-planning".into()],
///     ..Default::default()
/// };
/// assert!(!outline.retired);
/// assert!(outline.observed_satisfied.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntentOutline {
    /// The intent's identity.
    pub id: String,
    /// Its satisfaction criterion ids, in declaration order.
    pub criteria: Vec<String>,
    /// The milestone ids that realise it.
    pub milestones: Vec<String>,
    /// Cancelled or superseded: no coverage is required of it.
    pub retired: bool,
    /// The criteria its recorded gap observed already satisfied, with observations behind
    /// them (`intent_review::observed_satisfied`): no work is asked of these.
    pub observed_satisfied: Vec<String>,
}

/// How well a criterion is carried by the plan: whether work exists for it, whether that work
/// is gated by evidence, and whether the recorded gap already saw it true.
///
/// ```
/// use majordomus_cli::intent_plan::CoverageStrength;
/// // the wire spelling every surface answers with
/// assert_eq!(serde_json::to_string(&CoverageStrength::Weak).unwrap(), "\"weak\"");
/// // the order is the one a reader ranks by: uncovered is the worst
/// assert!(CoverageStrength::Uncovered < CoverageStrength::Covered);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStrength {
    /// No live issue serves it.
    Uncovered,
    /// No live issue serves it, and none needs to: the recorded gap observed it already true.
    /// Whether it is *met* is still the evidence's question, never this one's.
    Observed,
    /// Every live issue serving it requires no evidence, so nothing gates its completion.
    Weak,
    /// At least one live issue serving it requires evidence before it is DONE.
    Covered,
}

/// One issue as coverage sees it: which milestone carries it, the status the plan derived for
/// it, and how much evidence it owes before it may be DONE.
///
/// ```
/// use majordomus_cli::intent_plan::CoveringIssue;
/// let issue = CoveringIssue {
///     id: "I1901".into(),
///     milestone: "intent-planning".into(),
///     status: "READY".into(),
///     evidence_need: 2,
/// };
/// // an issue that owes no evidence covers a criterion only weakly
/// assert!(issue.evidence_need > 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoveringIssue {
    /// The issue id.
    pub id: String,
    /// Its milestone.
    pub milestone: String,
    /// Its derived status.
    pub status: String,
    /// How many evidence tokens it requires before it may be DONE.
    pub evidence_need: u32,
}

/// One criterion of one intent, and the work that carries it: the live issues that serve it,
/// the milestones those issues sit under, and the strength derived from them.
///
/// ```
/// use majordomus_cli::intent_plan::{CoverageStrength, CriterionCoverage};
/// let c = CriterionCoverage {
///     intent: "intent-planning".into(),
///     criterion: "gap-is-recorded".into(),
///     strength: CoverageStrength::Uncovered,
///     issues: vec![],
///     milestones: vec![],
/// };
/// // nothing serves it, so nothing in the plan will make it true
/// assert_eq!(c.strength, CoverageStrength::Uncovered);
/// assert!(c.issues.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriterionCoverage {
    /// The intent.
    pub intent: String,
    /// The criterion id inside it.
    pub criterion: String,
    /// The strength derived from the serving issues.
    pub strength: CoverageStrength,
    /// Every live issue that serves it, in id order.
    pub issues: Vec<CoveringIssue>,
    /// The milestones those issues belong to, sorted and unique.
    pub milestones: Vec<String>,
}

/// Why an issue exists: because it serves a criterion, because it is maintenance under a
/// milestone no intent names, or because nobody said — which is a failure under an intent.
///
/// ```
/// use majordomus_cli::intent_plan::IssueOrigin;
/// assert_eq!(serde_json::to_string(&IssueOrigin::Maintenance).unwrap(), "\"maintenance\"");
/// // work under no intent is legitimate and says so; it is never given an invented intent
/// assert_ne!(IssueOrigin::Maintenance, IssueOrigin::Unexplained);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueOrigin {
    /// It serves at least one criterion of a declared intent.
    Intent,
    /// Its milestone is named by no intent: maintenance or operational work, stated as such.
    Maintenance,
    /// Its milestone realises an intent, but it names nothing it serves.
    Unexplained,
}

/// The answer to "why does this issue exist?": the criteria it declares it serves, the intents
/// whose milestones contain it, and which of the three origins that adds up to.
///
/// ```
/// use majordomus_cli::intent_plan::{IssueOrigin, IssuePurpose};
/// let purpose = IssuePurpose {
///     issue: "I1901".into(),
///     milestone: "intent-planning".into(),
///     origin: IssueOrigin::Intent,
///     serves: vec!["intent-planning#criteria-are-covered".into()],
///     intents: vec!["intent-planning".into()],
/// };
/// assert_eq!(purpose.origin, IssueOrigin::Intent);
/// assert_eq!(purpose.serves.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssuePurpose {
    /// The issue id.
    pub issue: String,
    /// Its milestone.
    pub milestone: String,
    /// Which kind of reason it has.
    pub origin: IssueOrigin,
    /// The `<intent>#<criterion>` references it declares, as declared.
    pub serves: Vec<String>,
    /// The intents whose milestones include this issue's milestone, sorted.
    pub intents: Vec<String>,
}

/// The whole derived coverage: every criterion of every live intent, every issue with the
/// reason it exists, and every finding the two produced. Nothing here is stored; it is derived
/// on each read from the intents and the plan.
///
/// ```
/// use majordomus_cli::intent_plan::{IntentCoverage, IssueOrigin, IssuePurpose};
/// let coverage = IntentCoverage {
///     criteria: vec![],
///     issues: vec![IssuePurpose {
///         issue: "I0007".into(),
///         milestone: "ops".into(),
///         origin: IssueOrigin::Maintenance,
///         serves: vec![],
///         intents: vec![],
///     }],
///     findings: vec![],
/// };
/// assert_eq!(coverage.failures(), 0);
/// assert_eq!(coverage.purpose("I0007").unwrap().origin, IssueOrigin::Maintenance);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentCoverage {
    /// Every criterion of every live intent, in intent then declaration order.
    pub criteria: Vec<CriterionCoverage>,
    /// Every issue of the plan, in id order, with the reason it exists.
    pub issues: Vec<IssuePurpose>,
    /// Every finding, in derivation order.
    pub findings: Vec<IntentFinding>,
}

impl IntentCoverage {
    /// How many of the findings are failures rather than warnings: the count that decides
    /// whether `majordomus intent validate` exits 10.
    ///
    /// ```
    /// use majordomus_cli::intent_plan::IntentCoverage;
    /// let clean = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
    /// assert_eq!(clean.failures(), 0);
    /// ```
    pub fn failures(&self) -> usize {
        self.findings.iter().filter(|f| f.level == FAIL).count()
    }

    /// The purpose of one issue by id, or `None` when the plan holds no such issue — the
    /// answer to "why does this issue exist?".
    ///
    /// ```
    /// use majordomus_cli::intent_plan::IntentCoverage;
    /// let coverage = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
    /// assert!(coverage.purpose("I0001").is_none());
    /// ```
    pub fn purpose(&self, issue: &str) -> Option<&IssuePurpose> {
        self.issues.iter().find(|p| p.issue == issue)
    }

    /// The coverage of one `<intent>#<criterion>`, or `None` when no live intent declares it
    /// — the answer to "which work carries this criterion?".
    ///
    /// ```
    /// use majordomus_cli::intent_plan::IntentCoverage;
    /// let coverage = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
    /// assert!(coverage.criterion("intent-planning", "gap-is-recorded").is_none());
    /// ```
    pub fn criterion(&self, intent: &str, criterion: &str) -> Option<&CriterionCoverage> {
        self.criteria
            .iter()
            .find(|c| c.intent == intent && c.criterion == criterion)
    }
}

/// Is `token` an `<intent-id>#<criterion-id>` reference: two lowercase slugs joined by one
/// `#`? The same pattern the issue schema declares for `serves`.
///
/// ```
/// use majordomus_cli::intent_plan::serves_token;
/// assert!(serves_token("intent-lifecycle#stage-is-derived"));
/// assert!(!serves_token("intent-lifecycle"));
/// assert!(!serves_token("Intent#x"));
/// assert!(!serves_token("a#b#c"));
/// ```
pub fn serves_token(token: &str) -> bool {
    let slug = |s: &str| {
        s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    matches!(token.split_once('#'), Some((i, c)) if slug(i) && slug(c))
}

fn live(i: &PlanIssue) -> bool {
    i.status != "CANCELLED"
}

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

/// Derive the coverage of `intents` by `plan`.
///
/// ```
/// use majordomus_cli::intent_plan::{coverage, CoverageStrength, IntentOutline, IssueOrigin};
/// use majordomus_cli::plan::Plan;
/// let plan: Plan = serde_json::from_value(serde_json::json!({
///     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
///                 "active_milestone": ""},
///     "statuses": {"issue": [], "milestone": []},
///     "milestones": [], "waves": [], "edges": [], "milestone_edges": [], "findings": [],
///     "issues": [{"id": "I1", "milestone": "m", "status": "READY", "wave": 1,
///         "priority": "p1", "profile": "implementation", "parallel_safe": true,
///         "title": "t", "slug": "", "depends_on": [], "blocked_by": [], "dependents": [],
///         "scope": ["src"], "serves": ["x#works"], "objective": "", "evidence_have": 0,
///         "evidence_need": 1, "started_at": "", "verified_at": "", "completed_at": ""}]
/// })).unwrap();
/// let x = IntentOutline { id: "x".into(), criteria: vec!["works".into()],
///                         milestones: vec!["m".into()], ..Default::default() };
/// let c = coverage(&[x], &plan);
/// assert_eq!(c.failures(), 0);
/// assert_eq!(c.criterion("x", "works").unwrap().strength, CoverageStrength::Covered);
/// assert_eq!(c.purpose("I1").unwrap().origin, IssueOrigin::Intent);
/// ```
pub fn coverage(intents: &[IntentOutline], plan: &Plan) -> IntentCoverage {
    let mut f = Findings(Vec::new());
    let by_id: BTreeMap<&str, &IntentOutline> =
        intents.iter().map(|i| (i.id.as_str(), i)).collect();

    // milestone -> the live intents that name it
    let mut realises: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for i in intents.iter().filter(|i| !i.retired) {
        for m in &i.milestones {
            realises
                .entry(m.as_str())
                .or_default()
                .insert(i.id.as_str());
        }
    }

    // --- every issue: its purpose, and the links it declares
    let mut serving: BTreeMap<(String, String), Vec<&PlanIssue>> = BTreeMap::new();
    let mut purposes = Vec::with_capacity(plan.issues.len());
    for issue in &plan.issues {
        let under: Vec<String> = realises
            .get(issue.milestone.as_str())
            .map(|s| s.iter().map(|x| (*x).to_string()).collect())
            .unwrap_or_default();
        let mut serves_live = false;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for token in &issue.serves {
            if !serves_token(token) {
                f.push(
                    FAIL,
                    "malformed_serves",
                    &issue.id,
                    format!("serves {token}, which is not <intent-id>#<criterion-id>"),
                );
                continue;
            }
            if !seen.insert(token.as_str()) {
                f.push(
                    WARN,
                    "duplicate_serves",
                    &issue.id,
                    format!("serves {token} more than once"),
                );
                continue;
            }
            let Some((iid, cid)) = token.split_once('#') else {
                continue;
            };
            let Some(intent) = by_id.get(iid) else {
                f.push(
                    FAIL,
                    "serves_unknown_intent",
                    &issue.id,
                    format!("serves {token}, but no intent is called {iid}"),
                );
                continue;
            };
            if !intent.criteria.iter().any(|c| c == cid) {
                f.push(
                    FAIL,
                    "serves_unknown_criterion",
                    &issue.id,
                    format!("serves {token}, but intent {iid} declares no criterion {cid}"),
                );
                continue;
            }
            if intent.retired {
                continue; // a retired intent owes nothing and is owed nothing
            }
            if !intent.milestones.contains(&issue.milestone) {
                f.push(
                    FAIL,
                    "serves_outside_milestone",
                    &issue.id,
                    format!(
                        "serves {token} from milestone {}, which intent {iid} does not name",
                        issue.milestone
                    ),
                );
                continue;
            }
            if live(issue) {
                serves_live = true;
                serving
                    .entry((iid.to_string(), cid.to_string()))
                    .or_default()
                    .push(issue);
            }
        }
        let origin = if serves_live {
            IssueOrigin::Intent
        } else if under.is_empty() {
            IssueOrigin::Maintenance
        } else {
            IssueOrigin::Unexplained
        };
        if origin == IssueOrigin::Unexplained && live(issue) && issue.serves.is_empty() {
            f.push(
                FAIL,
                "issue_without_purpose",
                &issue.id,
                format!(
                    "is under milestone {}, which realises {}, but serves none of its criteria",
                    issue.milestone,
                    under.join(", ")
                ),
            );
        }
        purposes.push(IssuePurpose {
            issue: issue.id.clone(),
            milestone: issue.milestone.clone(),
            origin,
            serves: issue.serves.clone(),
            intents: under,
        });
    }
    purposes.sort_by(|a, b| a.issue.cmp(&b.issue));

    // --- every criterion of every live intent
    let mut criteria = Vec::new();
    for intent in intents.iter().filter(|i| !i.retired) {
        // An intent nobody has planned yet is a different state from a plan that misses a
        // criterion: the first is where every intent starts, the second is the defect this
        // exists to catch. So an intent whose milestones carry no live issue is reported
        // once, as a warning, and its criteria are not each refused for want of work.
        let planned = plan
            .issues
            .iter()
            .filter(|i| live(i))
            .any(|i| intent.milestones.contains(&i.milestone));
        if !planned {
            f.push(
                WARN,
                "intent_not_planned",
                &intent.id,
                "no live issue sits under any milestone it names; nothing is planned for it yet"
                    .into(),
            );
        }
        for cid in &intent.criteria {
            let key = (intent.id.clone(), cid.clone());
            let mut issues: Vec<&PlanIssue> = serving.get(&key).cloned().unwrap_or_default();
            issues.sort_by(|a, b| a.id.cmp(&b.id));
            issues.dedup_by(|a, b| a.id == b.id);
            let subject = format!("{}#{cid}", intent.id);
            let strength = if issues.is_empty() && intent.observed_satisfied.contains(cid) {
                CoverageStrength::Observed
            } else if issues.is_empty() {
                CoverageStrength::Uncovered
            } else if issues.iter().all(|i| i.evidence_need == 0) {
                CoverageStrength::Weak
            } else {
                CoverageStrength::Covered
            };
            match strength {
                // Only once work exists under the intent: see `intent_not_planned` above.
                CoverageStrength::Uncovered if planned => f.push(
                    FAIL,
                    "criterion_uncovered",
                    &subject,
                    "no live issue serves it; nothing in the plan will make it true".into(),
                ),
                CoverageStrength::Uncovered => {}
                CoverageStrength::Weak => f.push(
                    WARN,
                    "criterion_weakly_covered",
                    &subject,
                    "every issue serving it requires no evidence, so nothing gates it".into(),
                ),
                CoverageStrength::Covered | CoverageStrength::Observed => {}
            }
            for (n, a) in issues.iter().enumerate() {
                for b in &issues[n + 1..] {
                    if a.scope
                        .iter()
                        .any(|x| b.scope.iter().any(|y| overlap(x, y)))
                    {
                        f.push(
                            WARN,
                            "duplicate_work",
                            &subject,
                            format!("{} and {} both serve it over overlapping scope", a.id, b.id),
                        );
                    }
                }
            }
            let milestones: BTreeSet<String> = issues.iter().map(|i| i.milestone.clone()).collect();
            criteria.push(CriterionCoverage {
                intent: intent.id.clone(),
                criterion: cid.clone(),
                strength,
                issues: issues
                    .iter()
                    .map(|i| CoveringIssue {
                        id: i.id.clone(),
                        milestone: i.milestone.clone(),
                        status: i.status.clone(),
                        evidence_need: i.evidence_need,
                    })
                    .collect(),
                milestones: milestones.into_iter().collect(),
            });
        }
        // --- milestones the intent names that carry none of its criteria
        for m in &intent.milestones {
            let contributes = serving
                .iter()
                .any(|((iid, _), is)| iid == &intent.id && is.iter().any(|i| &i.milestone == m));
            if !contributes {
                f.push(
                    WARN,
                    "milestone_contributes_nothing",
                    m,
                    format!("is named by intent {}, but no live issue under it serves any of its criteria", intent.id),
                );
            }
        }
    }

    IntentCoverage {
        criteria,
        issues: purposes,
        findings: f.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanProject, PlanVocabulary};

    fn issue(id: &str, milestone: &str, serves: &[&str], scope: &[&str], need: u32) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: milestone.into(),
            status: "READY".into(),
            wave: 1,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: id.into(),
            slug: String::new(),
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            scope: scope.iter().map(|s| (*s).into()).collect(),
            serves: serves.iter().map(|s| (*s).into()).collect(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: need,
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

    fn intent(id: &str, criteria: &[&str], milestones: &[&str]) -> IntentOutline {
        IntentOutline {
            id: id.into(),
            criteria: criteria.iter().map(|s| (*s).into()).collect(),
            milestones: milestones.iter().map(|s| (*s).into()).collect(),
            ..Default::default()
        }
    }

    fn codes(c: &IntentCoverage) -> Vec<(&str, &str)> {
        c.findings
            .iter()
            .map(|f| (f.code.as_str(), f.subject.as_str()))
            .collect()
    }

    #[test]
    fn every_criterion_carried_by_evidence_gated_work_is_clean() {
        let c = coverage(
            &[intent("x", &["a", "b"], &["m1", "m2"])],
            &plan(vec![
                issue("I1", "m1", &["x#a"], &["src/a"], 1),
                issue("I2", "m2", &["x#b"], &["src/b"], 2),
            ]),
        );
        assert!(c.findings.is_empty(), "{:?}", c.findings);
        assert_eq!(c.criterion("x", "b").unwrap().milestones, ["m2"]);
    }

    #[test]
    fn an_intent_nobody_has_planned_is_a_warning_not_a_refusal() {
        let c = coverage(&[intent("x", &["a"], &["m1"])], &plan(vec![]));
        assert_eq!(
            codes(&c),
            [
                ("intent_not_planned", "x"),
                ("milestone_contributes_nothing", "m1")
            ]
        );
        assert_eq!(c.failures(), 0);
        assert_eq!(
            c.criterion("x", "a").unwrap().strength,
            CoverageStrength::Uncovered
        );
    }

    #[test]
    fn a_criterion_nothing_serves_is_uncovered_and_refused() {
        let c = coverage(
            &[intent("x", &["a", "b"], &["m1"])],
            &plan(vec![issue("I1", "m1", &["x#a"], &["src"], 1)]),
        );
        assert_eq!(codes(&c), [("criterion_uncovered", "x#b")]);
        assert_eq!(c.failures(), 1);
        assert_eq!(
            c.criterion("x", "b").unwrap().strength,
            CoverageStrength::Uncovered
        );
    }

    #[test]
    fn a_criterion_the_gap_observed_true_asks_for_no_work() {
        let mut x = intent("x", &["a", "b"], &["m1"]);
        x.observed_satisfied = vec!["b".into()];
        let c = coverage(&[x], &plan(vec![issue("I1", "m1", &["x#a"], &["src"], 1)]));
        assert!(c.findings.is_empty(), "{:?}", c.findings);
        assert_eq!(
            c.criterion("x", "b").unwrap().strength,
            CoverageStrength::Observed
        );
    }

    #[test]
    fn a_cancelled_issue_covers_nothing() {
        let mut gone = issue("I1", "m1", &["x#a"], &["src"], 1);
        gone.status = "CANCELLED".into();
        let c = coverage(&[intent("x", &["a"], &["m1"])], &plan(vec![gone]));
        // the only issue is cancelled, so nothing is planned: a warning, not a refusal
        assert!(codes(&c).contains(&("intent_not_planned", "x")));
        assert_eq!(c.failures(), 0);
        // a cancelled issue is not asked for a purpose either
        assert!(!codes(&c).iter().any(|(k, _)| *k == "issue_without_purpose"));
    }

    #[test]
    fn work_under_an_intent_that_serves_nothing_has_no_purpose() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![
                issue("I1", "m1", &["x#a"], &["src/a"], 1),
                issue("I2", "m1", &[], &["src/b"], 1),
            ]),
        );
        assert_eq!(codes(&c), [("issue_without_purpose", "I2")]);
        assert_eq!(c.purpose("I2").unwrap().origin, IssueOrigin::Unexplained);
        assert_eq!(c.purpose("I2").unwrap().intents, ["x"]);
    }

    #[test]
    fn work_under_no_intent_is_maintenance_and_not_a_finding() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![
                issue("I1", "m1", &["x#a"], &["src"], 1),
                issue("I9", "ops", &[], &["ops"], 0),
            ]),
        );
        assert!(c.findings.is_empty(), "{:?}", c.findings);
        assert_eq!(c.purpose("I9").unwrap().origin, IssueOrigin::Maintenance);
    }

    #[test]
    fn a_malformed_or_repeated_reference_is_named() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![issue("I1", "m1", &["x#a", "x#a", "x"], &["src"], 1)]),
        );
        assert_eq!(
            codes(&c),
            [("duplicate_serves", "I1"), ("malformed_serves", "I1")]
        );
        assert_eq!(c.criterion("x", "a").unwrap().issues.len(), 1);
    }

    #[test]
    fn a_dangling_reference_is_refused_by_name() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![
                issue("I1", "m1", &["x#a", "x#nope"], &["src"], 1),
                issue("I2", "m1", &["y#a"], &["src2"], 1),
            ]),
        );
        let got = codes(&c);
        assert!(got.contains(&("serves_unknown_criterion", "I1")), "{got:?}");
        assert!(got.contains(&("serves_unknown_intent", "I2")), "{got:?}");
    }

    #[test]
    fn serving_from_a_milestone_the_intent_does_not_name_is_refused() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![issue("I1", "elsewhere", &["x#a"], &["src"], 1)]),
        );
        let got = codes(&c);
        assert!(got.contains(&("serves_outside_milestone", "I1")), "{got:?}");
        assert!(got.contains(&("intent_not_planned", "x")), "{got:?}");
        assert!(
            got.contains(&("milestone_contributes_nothing", "m1")),
            "{got:?}"
        );
    }

    #[test]
    fn coverage_without_required_evidence_is_weak() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![issue("I1", "m1", &["x#a"], &["src"], 0)]),
        );
        assert_eq!(codes(&c), [("criterion_weakly_covered", "x#a")]);
        assert_eq!(c.failures(), 0);
    }

    #[test]
    fn two_issues_on_one_criterion_over_one_scope_are_duplicate_work() {
        let c = coverage(
            &[intent("x", &["a"], &["m1"])],
            &plan(vec![
                issue("I1", "m1", &["x#a"], &["src/lib"], 1),
                issue("I2", "m1", &["x#a"], &["src"], 1),
                issue("I3", "m1", &["x#a"], &["docs"], 1),
            ]),
        );
        assert_eq!(codes(&c), [("duplicate_work", "x#a")]);
        assert_eq!(c.criterion("x", "a").unwrap().issues.len(), 3);
    }

    #[test]
    fn a_retired_intent_owes_no_coverage() {
        let mut old = intent("x", &["a"], &["m1"]);
        old.retired = true;
        let c = coverage(
            &[old],
            &plan(vec![issue("I1", "m1", &["x#a"], &["src"], 1)]),
        );
        assert!(c.findings.is_empty(), "{:?}", c.findings);
        assert!(c.criteria.is_empty());
        assert_eq!(c.purpose("I1").unwrap().origin, IssueOrigin::Maintenance);
    }

    #[test]
    fn the_derivation_does_not_depend_on_the_order_records_are_read() {
        let intents = [
            intent("x", &["a", "b"], &["m1"]),
            intent("y", &["c"], &["m2"]),
        ];
        let issues = vec![
            issue("I1", "m1", &["x#a"], &["a"], 1),
            issue("I2", "m1", &["x#b"], &["b"], 0),
            issue("I3", "m2", &[], &["c"], 1),
        ];
        let forward = coverage(&intents, &plan(issues.clone()));
        let mut rev = issues;
        rev.reverse();
        let backward = coverage(&intents, &plan(rev));
        assert_eq!(forward.criteria, backward.criteria);
        assert_eq!(forward.issues, backward.issues);
        let sorted = |c: &IntentCoverage| {
            let mut v = c.findings.clone();
            v.sort_by(|a, b| (&a.code, &a.subject).cmp(&(&b.code, &b.subject)));
            v
        };
        assert_eq!(sorted(&forward), sorted(&backward));
    }
}
