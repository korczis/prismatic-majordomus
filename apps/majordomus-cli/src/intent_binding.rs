//! The binding: what a piece of work serves, resolved once, before it starts (ADR 0111).
//!
//! [`crate::intent::Intents::preflight`] answers which intent an issue, or some paths, serve.
//! A task needs slightly more than that and nothing different: it may name an **intent**
//! instead of an issue, it may give an **exemption** the policy declares instead of either,
//! paths that reach more than one intent are an **ambiguity** rather than two answers, and
//! the worker who resumes the task later needs to know whether what it was bound to has
//! **moved**. This module is that layer and only that layer: every link is still judged by
//! the preflight, which is still judged by the coverage `intent validate` reports from.
//!
//! **Nothing here is stored and nothing here writes.** A task record keeps what its worker
//! named — an issue id, an intent id, an exemption with its reason — and two pins. The
//! pins are hashes compared on a later read; the intent, its stage, its verdict and its
//! criteria are derived again every time, as rule `project.an-intent-outlives-its-sessions`
//! requires.
//!
//! | pin                 | hashes                                                             | moves when |
//! |---------------------|--------------------------------------------------------------------|------------|
//! | `plan_revision`     | each served intent's statement, invariants, non-goals and criteria; the issues resolved with what each serves; each critique's findings and their resolutions | the intent, a link or the critique is edited |
//! | `evidence_standing` | the evidence state of each served criterion                         | a criterion's evidence changes state |
//!
//! Two, because they answer different questions and fail differently: an ordinary commit can
//! make evidence stale without anyone having touched what the work is for, and a worker
//! told "your intent changed" on every commit stops reading the line.
//!
//! ```
//! use majordomus_cli::intent_binding::{BindingCause, BindingRequest, BindingStanding};
//! // naming nothing is not a binding
//! assert!(BindingRequest::default().names_nothing());
//! assert_eq!(BindingStanding::Exempt.as_str(), "exempt");
//! assert_eq!(
//!     serde_json::to_string(&BindingCause::AmbiguousIntent).unwrap(),
//!     "\"ambiguous_intent\""
//! );
//! ```

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::intent::{
    IntentPreflight, IntentPreflightCause, IntentPreflightIntent, IntentPreflightIssue,
    IntentPreflightVerdict, IntentStage, Intents, RepositoryEvidence,
};
use crate::intent_review::{CritiqueRecord, GapRecord};
use crate::plan::{overlap, Plan, PlanIssue};

use crate::index::Index;
use crate::policy::{sha256_hex, BindingMode, IntentPolicy};

/// Where a piece of work stands against the intents: the one word a start decides on.
///
/// ```
/// use majordomus_cli::intent_binding::BindingStanding;
/// assert_eq!(serde_json::to_string(&BindingStanding::Bound).unwrap(), "\"bound\"");
/// assert!(BindingStanding::Refused.refuses());
/// assert!(!BindingStanding::Maintenance.refuses());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BindingStanding {
    /// The work serves at least one criterion of a live intent, through links that hold, and
    /// each such intent's plan was critiqued with no blocking finding open.
    Bound,
    /// The issues the work resolves to sit under milestones no live intent names: derived
    /// maintenance, as `intent validate` allows.
    Maintenance,
    /// The worker gave an exemption class the policy declares, with a reason.
    Exempt,
    /// The work may not start as bound work; every reason is in `refusals`.
    Refused,
}

impl BindingStanding {
    /// The word every surface and the task record use.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingStanding;
    /// assert_eq!(BindingStanding::Maintenance.as_str(), "maintenance");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            BindingStanding::Bound => "bound",
            BindingStanding::Maintenance => "maintenance",
            BindingStanding::Exempt => "exempt",
            BindingStanding::Refused => "refused",
        }
    }

    /// Whether a start that requires binding must not proceed.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingStanding;
    /// assert!(!BindingStanding::Exempt.refuses());
    /// ```
    pub fn refuses(self) -> bool {
        self == BindingStanding::Refused
    }
}

/// Why a binding was refused, as a word a reader greps for. The first seven are the
/// preflight's own causes, carried through unchanged; the rest are what only a binding can
/// find.
///
/// ```
/// use majordomus_cli::intent::IntentPreflightCause;
/// use majordomus_cli::intent_binding::BindingCause;
/// // a preflight cause keeps its word
/// assert_eq!(
///     serde_json::to_string(&BindingCause::from(IntentPreflightCause::IntentNotCritiqued)).unwrap(),
///     serde_json::to_string(&IntentPreflightCause::IntentNotCritiqued).unwrap(),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BindingCause {
    /// The issue named is not an issue of the plan.
    UnknownIssue,
    /// No open issue's scope covers any of the paths.
    NoIssueCoversPaths,
    /// The issue's milestone realises a live intent and the issue serves none of its criteria.
    IssueServesNothing,
    /// The issue serves a criterion of an intent whose milestones do not include its own.
    ServesAnotherIntent,
    /// The issue serves an intent or a criterion that does not exist.
    ServesUnknownCriterion,
    /// An intent the work serves has no critique.
    IntentNotCritiqued,
    /// The critique of an intent the work serves has a blocking finding still open.
    OpenBlockingFinding,
    /// Nothing was named: no issue, no intent, no paths and no exemption.
    NothingNamed,
    /// The intent named is not an intent of this repository.
    UnknownIntent,
    /// The intent named is cancelled or superseded.
    IntentRetired,
    /// The intent named has no open issue serving it, so there is no work to be bound to.
    IntentHasNoOpenWork,
    /// The issue named serves none of the criteria of the intent named beside it.
    IssueOutsideIntent,
    /// Paths alone reached more than one intent; the worker must name the issue.
    AmbiguousIntent,
    /// The exemption class is not one the policy declares.
    UnknownExemption,
    /// An exemption was given with no reason.
    ExemptionWithoutReason,
    /// An exemption was given beside an issue or an intent: work is one or the other.
    ExemptionNamesWork,
}

impl From<IntentPreflightCause> for BindingCause {
    fn from(c: IntentPreflightCause) -> Self {
        match c {
            IntentPreflightCause::UnknownIssue => BindingCause::UnknownIssue,
            IntentPreflightCause::NoIssueCoversPaths => BindingCause::NoIssueCoversPaths,
            IntentPreflightCause::IssueServesNothing => BindingCause::IssueServesNothing,
            IntentPreflightCause::ServesAnotherIntent => BindingCause::ServesAnotherIntent,
            IntentPreflightCause::ServesUnknownCriterion => BindingCause::ServesUnknownCriterion,
            IntentPreflightCause::IntentNotCritiqued => BindingCause::IntentNotCritiqued,
            IntentPreflightCause::OpenBlockingFinding => BindingCause::OpenBlockingFinding,
        }
    }
}

/// One reason a binding was refused.
///
/// ```
/// use majordomus_cli::intent_binding::{BindingCause, BindingRefusal};
/// let r = BindingRefusal::new(None, BindingCause::NothingNamed, "name the work".into());
/// assert!(r.issue.is_none());
/// assert_eq!(r.cause, BindingCause::NothingNamed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BindingRefusal {
    /// The issue it is about; absent when it is about no one issue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The cause.
    pub cause: BindingCause,
    /// What is wrong, in one sentence.
    pub message: String,
}

impl BindingRefusal {
    /// A refusal about `issue`, or about no one issue.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::{BindingCause, BindingRefusal};
    /// let r = BindingRefusal::new(Some("I0001"), BindingCause::UnknownIssue, "x".into());
    /// assert_eq!(r.issue.as_deref(), Some("I0001"));
    /// ```
    pub fn new(issue: Option<&str>, cause: BindingCause, message: String) -> Self {
        BindingRefusal {
            issue: issue.map(str::to_string),
            cause,
            message,
        }
    }
}

/// What a worker named when it asked what its work serves. Every field is optional and the
/// combinations are judged, not assumed: an issue alone, an intent alone, an issue beside
/// the intent it should serve, paths alone, or an exemption with its reason.
///
/// ```
/// use majordomus_cli::intent_binding::BindingRequest;
/// let by_issue = BindingRequest { issue: Some("I0001".into()), ..Default::default() };
/// assert!(!by_issue.names_nothing());
/// // blanks are what a query string or a shell variable leaves behind, and name nothing
/// let blank = BindingRequest { issue: Some("  ".into()), ..Default::default() }.trimmed();
/// assert!(blank.names_nothing());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct BindingRequest {
    /// The issue the work executes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The intent the work serves, when the worker names it directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The paths the work will touch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// The exemption class, when the work is under no intent on purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemption: Option<String>,
    /// Why the exemption applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
}

impl BindingRequest {
    /// The request with every blank value dropped and every value trimmed.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingRequest;
    /// let r = BindingRequest { intent: Some(" x ".into()), paths: vec!["".into(), " lib ".into()], ..Default::default() }.trimmed();
    /// assert_eq!(r.intent.as_deref(), Some("x"));
    /// assert_eq!(r.paths, ["lib"]);
    /// ```
    pub fn trimmed(self) -> Self {
        let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        BindingRequest {
            issue: clean(self.issue),
            intent: clean(self.intent),
            paths: self
                .paths
                .iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect(),
            exemption: clean(self.exemption),
            because: clean(self.because),
        }
    }

    /// Whether the request names no issue, no intent, no path and no exemption.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingRequest;
    /// assert!(BindingRequest::default().names_nothing());
    /// ```
    pub fn names_nothing(&self) -> bool {
        self.issue.is_none()
            && self.intent.is_none()
            && self.paths.is_empty()
            && self.exemption.is_none()
    }
}

/// The exemption a binding accepted: the class as the policy declares it, and the reason
/// the worker gave.
///
/// ```
/// use majordomus_cli::intent_binding::BindingExemption;
/// let e = BindingExemption {
///     class: "emergency".into(),
///     description: "Restoring a broken trunk".into(),
///     because: "master is red on case 500".into(),
/// };
/// assert_eq!(e.class, "emergency");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BindingExemption {
    /// The class id.
    pub class: String,
    /// What the policy says the class is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The worker's reason.
    pub because: String,
}

/// What a piece of work is bound to: its standing, what was named, the issues and intents
/// it resolved to with what each intent asks of the worker, the two pins, and every refusal.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_binding::{
///     bind, BindingCause, BindingRequest, BindingStanding, IntentBinding,
/// };
/// use majordomus_cli::plan::Plan;
/// use majordomus_cli::policy::IntentPolicy;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// let nothing: IntentBinding =
///     bind(&intents, &plan, &[], &[], &IntentPolicy::default(), BindingRequest::default());
/// assert_eq!(nothing.standing, BindingStanding::Refused);
/// assert_eq!(nothing.refusals[0].cause, BindingCause::NothingNamed);
/// // nothing was reached, so there is nothing to pin
/// assert!(nothing.plan_revision.is_empty() && nothing.evidence_standing.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentBinding {
    /// `bound`, `maintenance`, `exempt` or `refused`.
    pub standing: BindingStanding,
    /// What the worker named, trimmed: the request this answers.
    pub named: BindingRequest,
    /// The exemption accepted, when the standing is `exempt`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemption: Option<BindingExemption>,
    /// The issues the work resolved to, each with the preflight's verdict.
    pub issues: Vec<IntentPreflightIssue>,
    /// The intents the work serves, each with the criteria served, their evidence state,
    /// the invariants, the critique and the gap: what the worker is held to.
    pub intents: Vec<IntentPreflightIntent>,
    /// The governance of those intents, each entry once.
    pub governance: Vec<String>,
    /// The pin of what the work is for; empty when no intent was reached.
    pub plan_revision: String,
    /// The pin of where the served criteria's evidence stands; empty when no intent was
    /// reached.
    pub evidence_standing: String,
    /// What a worker should know and no gate refuses on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Every reason the binding was refused; empty unless the standing is `refused`.
    pub refusals: Vec<BindingRefusal>,
    /// When refused: every refusal's sentence, joined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

impl IntentBinding {
    fn refused(named: BindingRequest, refusals: Vec<BindingRefusal>) -> Self {
        let refusal = Some(
            refusals
                .iter()
                .map(|r| r.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        );
        IntentBinding {
            standing: BindingStanding::Refused,
            named,
            exemption: None,
            issues: Vec::new(),
            intents: Vec::new(),
            governance: Vec::new(),
            plan_revision: String::new(),
            evidence_standing: String::new(),
            notes: Vec::new(),
            refusals,
            refusal,
        }
    }
}

/// How a binding moved between two reads: what a resumed worker is told.
///
/// ```
/// use majordomus_cli::intent_binding::BindingDrift;
/// let d = BindingDrift::between("a", "s1", "a", "s2");
/// assert!(!d.plan_changed && d.evidence_changed);
/// assert_eq!(d.as_str(), "evidence_moved");
/// assert_eq!(BindingDrift::between("a", "s", "a", "s").as_str(), "unchanged");
/// assert_eq!(BindingDrift::between("a", "s", "b", "s").as_str(), "plan_changed");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BindingDrift {
    /// The intent, a link or the critique was edited since the pin was taken.
    pub plan_changed: bool,
    /// A served criterion's evidence changed state since the pin was taken.
    pub evidence_changed: bool,
}

impl BindingDrift {
    /// Compare the pins a record carries with the pins the binding answers now.
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingDrift;
    /// assert!(BindingDrift::between("a", "s", "b", "t").plan_changed);
    /// ```
    pub fn between(pinned_plan: &str, pinned_evidence: &str, plan: &str, evidence: &str) -> Self {
        BindingDrift {
            plan_changed: pinned_plan != plan,
            evidence_changed: pinned_evidence != evidence,
        }
    }

    /// One word: `unchanged`, `evidence_moved`, or `plan_changed` (which outranks the other,
    /// because a worker whose criteria were rewritten must re-read them before anything).
    ///
    /// ```
    /// use majordomus_cli::intent_binding::BindingDrift;
    /// assert_eq!(BindingDrift::between("a", "s", "b", "t").as_str(), "plan_changed");
    /// ```
    pub fn as_str(self) -> &'static str {
        match (self.plan_changed, self.evidence_changed) {
            (true, _) => "plan_changed",
            (false, true) => "evidence_moved",
            (false, false) => "unchanged",
        }
    }
}

/// What `request` is bound to. One derivation: every link is judged by the preflight, and
/// this adds only what a task needs beside it — an intent given by name, a declared
/// exemption, ambiguity, the scope note and the two pins.
///
/// The order of judgement is fixed, so the same request always meets the same first refusal:
/// an exemption is judged alone (and refuses to stand beside named work); then an issue, an
/// intent, or paths, in that precedence, choose the issues; then the preflight judges them.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_binding::{bind, BindingCause, BindingRequest, BindingStanding};
/// use majordomus_cli::plan::Plan;
/// use majordomus_cli::policy::{ExemptionClass, IntentPolicy};
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// let policy = IntentPolicy {
///     exemptions: vec![ExemptionClass { id: "emergency".into(), description: String::new() }],
///     ..Default::default()
/// };
/// let ask = |exemption: &str, because: Option<&str>| {
///     bind(&intents, &plan, &[], &[], &policy, BindingRequest {
///         exemption: Some(exemption.into()),
///         because: because.map(str::to_string),
///         ..Default::default()
///     })
/// };
/// // a declared class with a reason is the only exemption there is
/// assert_eq!(ask("emergency", Some("master is red")).standing, BindingStanding::Exempt);
/// assert_eq!(ask("emergency", None).refusals[0].cause, BindingCause::ExemptionWithoutReason);
/// assert_eq!(ask("whim", Some("because")).refusals[0].cause, BindingCause::UnknownExemption);
/// ```
pub fn bind(
    intents: &Intents,
    plan: &Plan,
    gaps: &[GapRecord],
    critiques: &[CritiqueRecord],
    policy: &IntentPolicy,
    request: BindingRequest,
) -> IntentBinding {
    let named = request.trimmed();
    if named.names_nothing() {
        return IntentBinding::refused(
            named,
            vec![BindingRefusal::new(
                None,
                BindingCause::NothingNamed,
                "nothing was named: give the issue the work executes, the intent it serves, \
                 the paths it will touch, or an exemption the policy declares with its reason"
                    .into(),
            )],
        );
    }
    if let Some(class) = named.exemption.clone() {
        return exempt(policy, named, &class);
    }

    let mut early: Vec<BindingRefusal> = Vec::new();
    let mut by_paths = false;
    let issues: Vec<&PlanIssue> = match (named.issue.as_deref(), named.intent.as_deref()) {
        (Some(id), wanted) => match plan.issue(id) {
            Some(i) => {
                if let Some(intent) = wanted {
                    judge_named_intent(intents, intent, &mut early);
                    if early.is_empty() && !serves_intent(i, intent) {
                        early.push(BindingRefusal::new(
                            Some(id),
                            BindingCause::IssueOutsideIntent,
                            format!("{id} serves no criterion of intent {intent}"),
                        ));
                    }
                }
                vec![i]
            }
            None => {
                early.push(BindingRefusal::new(
                    Some(id),
                    BindingCause::UnknownIssue,
                    format!("`{id}` is not an issue under .ai/repo/project/issues/"),
                ));
                Vec::new()
            }
        },
        (None, Some(intent)) => {
            judge_named_intent(intents, intent, &mut early);
            let open: Vec<&PlanIssue> = plan
                .issues
                .iter()
                .filter(|i| is_open(i) && serves_intent(i, intent))
                .collect();
            if early.is_empty() && open.is_empty() {
                early.push(BindingRefusal::new(
                    None,
                    BindingCause::IntentHasNoOpenWork,
                    format!(
                        "no open issue serves intent {intent}; work under it needs an issue \
                         that names the criterion it serves"
                    ),
                ));
            }
            open
        }
        (None, None) => {
            by_paths = true;
            let covering: Vec<&PlanIssue> = plan
                .issues
                .iter()
                .filter(|i| is_open(i) && covers(i, &named.paths))
                .collect();
            if covering.is_empty() {
                early.push(BindingRefusal::new(
                    None,
                    BindingCause::NoIssueCoversPaths,
                    format!(
                        "no open issue's scope covers {}; work that names no issue serves no \
                         declared intent",
                        named.paths.join(", ")
                    ),
                ));
            }
            covering
        }
    };
    if !early.is_empty() {
        return IntentBinding::refused(named, early);
    }

    let pre = intents.preflight_issues(plan, gaps, critiques, &issues);
    let mut refusals: Vec<BindingRefusal> = pre
        .refusals
        .iter()
        .map(|r| BindingRefusal::new(r.issue.as_deref(), r.cause.into(), r.message.clone()))
        .collect();
    if by_paths && pre.intents.len() > 1 {
        refusals.push(BindingRefusal::new(
            None,
            BindingCause::AmbiguousIntent,
            format!(
                "the paths reach {} intents ({}); name the issue the work executes",
                pre.intents.len(),
                pre.intents
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }

    let mut notes = Vec::new();
    if !by_paths && !named.paths.is_empty() {
        let outside: Vec<&str> = named
            .paths
            .iter()
            .filter(|p| !issues.iter().any(|i| covers(i, std::slice::from_ref(p))))
            .map(String::as_str)
            .collect();
        if !outside.is_empty() {
            notes.push(format!(
                "{} lie outside the scope of {}",
                outside.join(", "),
                issues
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    let standing = if !refusals.is_empty() {
        BindingStanding::Refused
    } else if pre.verdict == IntentPreflightVerdict::Serves {
        BindingStanding::Bound
    } else {
        BindingStanding::Maintenance
    };
    let refusal = (!refusals.is_empty()).then(|| {
        refusals
            .iter()
            .map(|r| r.message.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    });
    let plan_revision = plan_revision(&pre, &issues, critiques);
    let evidence_standing = evidence_standing(&pre);
    IntentBinding {
        standing,
        named,
        exemption: None,
        issues: pre.issues,
        intents: pre.intents,
        governance: pre.governance,
        plan_revision,
        evidence_standing,
        notes,
        refusals,
        refusal,
    }
}

/// Why the start transition of `issue` is refused, or `None` when it is not this module's
/// to refuse: the policy does not say `intent.binding: required`, or the issue's binding is
/// bound or maintenance. The sentence is the one `majordomus plan start` prints in the shell
/// engine, which asks the same capability; cases 133 and 962 hold the two equal.
///
/// A task is one way work begins and a transition is the other. Gating only the first would
/// leave a door beside the gate, so [`crate::plan::check`] asks this before it lets an issue
/// become ACTIVE.
///
/// ```
/// use majordomus_cli::intent_binding::start_refusal;
/// use majordomus_cli::plan::Plan;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
/// let plan = Plan::build(&index);
/// // a repository whose policy does not require binding refuses nothing here
/// assert_eq!(start_refusal(&index, &plan, "I0001"), None);
/// ```
pub fn start_refusal(index: &Index, plan: &Plan, issue: &str) -> Option<String> {
    let root = std::path::PathBuf::from(&index.repository.root);
    let policy = crate::repository::Repository::open(&root)
        .ok()
        .and_then(|repo| crate::policy::LoadedPolicy::load(&repo).ok())
        .map(|loaded| loaded.policy.intent)
        .unwrap_or_default();
    if policy.binding != BindingMode::Required {
        return None;
    }
    let evidence = match RepositoryEvidence::load(index) {
        Ok(e) => e,
        Err(e) => {
            return Some(format!(
                "{issue} may not start: the evidence ledger could not be read ({e}); the \
                 binding is unknown, and unknown is never a pass"
            ))
        }
    };
    let intents = Intents::build(index, plan, &evidence);
    let binding = bind(
        &intents,
        plan,
        &GapRecord::all(index),
        &CritiqueRecord::all(index),
        &policy,
        BindingRequest {
            issue: Some(issue.to_string()),
            ..Default::default()
        },
    );
    binding
        .refusal
        .filter(|_| binding.standing.refuses())
        .map(|why| format!("{issue} may not start: {why}"))
}

fn exempt(policy: &IntentPolicy, named: BindingRequest, class: &str) -> IntentBinding {
    let mut refusals = Vec::new();
    if named.issue.is_some() || named.intent.is_some() {
        refusals.push(BindingRefusal::new(
            named.issue.as_deref(),
            BindingCause::ExemptionNamesWork,
            format!(
                "exemption {class} was given beside named work; a task serves what it names \
                 or is exempt, never both"
            ),
        ));
    }
    let declared = policy.exemption(class);
    if declared.is_none() {
        let known: Vec<&str> = policy.exemptions.iter().map(|c| c.id.as_str()).collect();
        refusals.push(BindingRefusal::new(
            None,
            BindingCause::UnknownExemption,
            if known.is_empty() {
                format!(
                    "`{class}` is not an exemption: the policy declares none under \
                     intent.exemptions"
                )
            } else {
                format!(
                    "`{class}` is not an exemption the policy declares ({})",
                    known.join(", ")
                )
            },
        ));
    }
    if named.because.is_none() {
        refusals.push(BindingRefusal::new(
            None,
            BindingCause::ExemptionWithoutReason,
            format!("exemption {class} needs a reason: say why this work serves no intent"),
        ));
    }
    match (declared, named.because.clone()) {
        (Some(c), Some(because)) if refusals.is_empty() => IntentBinding {
            standing: BindingStanding::Exempt,
            exemption: Some(BindingExemption {
                class: c.id.clone(),
                description: c.description.clone(),
                because,
            }),
            named,
            issues: Vec::new(),
            intents: Vec::new(),
            governance: Vec::new(),
            plan_revision: String::new(),
            evidence_standing: String::new(),
            notes: Vec::new(),
            refusals: Vec::new(),
            refusal: None,
        },
        _ => IntentBinding::refused(named, refusals),
    }
}

fn judge_named_intent(intents: &Intents, id: &str, refusals: &mut Vec<BindingRefusal>) {
    match intents.intent(id) {
        None => refusals.push(BindingRefusal::new(
            None,
            BindingCause::UnknownIntent,
            format!("`{id}` is not an intent under .ai/repo/project/intents/"),
        )),
        Some(view) if matches!(view.stage, IntentStage::Cancelled | IntentStage::Superseded) => {
            refusals.push(BindingRefusal::new(
                None,
                BindingCause::IntentRetired,
                format!(
                    "intent {id} is {}; work is not started under a retired intent",
                    view.stage.as_str()
                ),
            ))
        }
        Some(_) => {}
    }
}

fn is_open(i: &PlanIssue) -> bool {
    !matches!(i.status.as_str(), "DONE" | "CANCELLED")
}

fn covers(i: &PlanIssue, paths: &[String]) -> bool {
    i.scope.iter().any(|s| paths.iter().any(|p| overlap(s, p)))
}

fn serves_intent(i: &PlanIssue, intent: &str) -> bool {
    i.serves
        .iter()
        .filter_map(|t| t.split_once('#'))
        .any(|(iid, _)| iid == intent)
}

/// The pin of what the work is for. Everything a worker would have to re-read if it
/// changed, and nothing that changes without anyone editing the plan: no stage, no evidence
/// state, no status of any issue.
fn plan_revision(
    pre: &IntentPreflight,
    issues: &[&PlanIssue],
    critiques: &[CritiqueRecord],
) -> String {
    if pre.intents.is_empty() {
        return String::new();
    }
    // keyed maps, so the hash never depends on the order a record lists things in
    let held: BTreeMap<&str, serde_json::Value> = pre
        .intents
        .iter()
        .map(|i| {
            let critique = critiques.iter().find(|c| c.intent == i.id).map(|c| {
                let findings: BTreeMap<&str, serde_json::Value> = c
                    .findings
                    .iter()
                    .map(|f| {
                        let finding = json!({
                            "class": f.class_text,
                            "subject": f.subject,
                            "finding": f.finding,
                            "blocking": f.blocking,
                            "resolution": f.resolution_text,
                            "issue": f.issue,
                        });
                        (f.id.as_str(), finding)
                    })
                    .collect();
                json!({ "reviewed_at": c.reviewed_at, "findings": findings })
            });
            let intent = json!({
                "statement": i.statement,
                "invariants": i.invariants,
                "non_goals": i.non_goals,
                "criteria": i.criteria.iter().map(|c| json!({
                    "id": c.id,
                    "criterion": c.criterion,
                    "evidence": c.evidence,
                    "ref": c.reference,
                })).collect::<Vec<_>>(),
                "critique": critique,
            });
            (i.id.as_str(), intent)
        })
        .collect();
    let links: BTreeMap<&str, serde_json::Value> = issues
        .iter()
        .map(|i| {
            let serves: BTreeSet<&str> = i.serves.iter().map(String::as_str).collect();
            (
                i.id.as_str(),
                json!({ "milestone": i.milestone, "serves": serves }),
            )
        })
        .collect();
    sha256_hex(&json!({ "intents": held, "links": links }).to_string())
}

/// The pin of where the served criteria's evidence stands: one state word per criterion.
fn evidence_standing(pre: &IntentPreflight) -> String {
    if pre.intents.is_empty() {
        return String::new();
    }
    let states: BTreeSet<String> = pre
        .intents
        .iter()
        .flat_map(|i| {
            i.criteria
                .iter()
                .map(move |c| format!("{}#{}={}", i.id, c.id, c.state.as_str()))
        })
        .collect();
    sha256_hex(&states.into_iter().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ExemptionClass;

    fn empty_plan() -> Plan {
        serde_json::from_value(json!({
            "project": {"name": "p", "repository": "o/p", "default_branch": "master",
                        "active_milestone": ""},
            "statuses": {"issue": [], "milestone": []},
            "milestones": [], "issues": [], "waves": [], "edges": [],
            "milestone_edges": [], "findings": []
        }))
        .unwrap()
    }

    fn none() -> Intents {
        Intents {
            intents: vec![],
            findings: vec![],
        }
    }

    fn policy(classes: &[&str]) -> IntentPolicy {
        IntentPolicy {
            exemptions: classes
                .iter()
                .map(|c| ExemptionClass {
                    id: (*c).into(),
                    description: String::new(),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn an_exemption_beside_named_work_is_refused_even_when_declared() {
        let b = bind(
            &none(),
            &empty_plan(),
            &[],
            &[],
            &policy(&["emergency"]),
            BindingRequest {
                issue: Some("I0001".into()),
                exemption: Some("emergency".into()),
                because: Some("red trunk".into()),
                ..Default::default()
            },
        );
        assert_eq!(b.standing, BindingStanding::Refused);
        assert_eq!(b.refusals[0].cause, BindingCause::ExemptionNamesWork);
        assert!(b.exemption.is_none());
    }

    #[test]
    fn every_fault_of_an_exemption_is_named_at_once() {
        let b = bind(
            &none(),
            &empty_plan(),
            &[],
            &[],
            &policy(&[]),
            BindingRequest {
                exemption: Some("whim".into()),
                ..Default::default()
            },
        );
        let causes: Vec<BindingCause> = b.refusals.iter().map(|r| r.cause).collect();
        assert_eq!(
            causes,
            [
                BindingCause::UnknownExemption,
                BindingCause::ExemptionWithoutReason
            ]
        );
        assert!(b.refusal.unwrap().contains("declares none"));
    }

    #[test]
    fn an_unknown_intent_and_an_unknown_issue_are_different_refusals() {
        let ask = |r: BindingRequest| {
            bind(
                &none(),
                &empty_plan(),
                &[],
                &[],
                &IntentPolicy::default(),
                r,
            )
        };
        let by_intent = ask(BindingRequest {
            intent: Some("absent".into()),
            ..Default::default()
        });
        assert_eq!(by_intent.refusals[0].cause, BindingCause::UnknownIntent);
        let by_issue = ask(BindingRequest {
            issue: Some("I9999".into()),
            ..Default::default()
        });
        assert_eq!(by_issue.refusals[0].cause, BindingCause::UnknownIssue);
        let by_paths = ask(BindingRequest {
            paths: vec!["README.md".into()],
            ..Default::default()
        });
        assert_eq!(by_paths.refusals[0].cause, BindingCause::NoIssueCoversPaths);
    }

    #[test]
    fn drift_names_the_plan_before_the_evidence() {
        assert_eq!(
            BindingDrift::between("p1", "e1", "p2", "e2").as_str(),
            "plan_changed"
        );
        assert_eq!(
            BindingDrift::between("p1", "e1", "p1", "e2").as_str(),
            "evidence_moved"
        );
        assert_eq!(
            BindingDrift::between("p1", "e1", "p1", "e1").as_str(),
            "unchanged"
        );
    }
}
