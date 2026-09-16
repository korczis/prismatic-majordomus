//! Intent realization: which work realises which intent, through which sessions and providers,
//! how each link is known, and what the intent still lacks.
//!
//! An intent (ADR 0070) says what must become true; the plan says which issues realise it
//! (ADR 0073). Neither says who is doing that work, in which episode, under which provider, or
//! which handover carried it from one session to the next. That lineage already exists as
//! records — the ledger stamps every task, handover and plan transition with its episode, the
//! provider events name the provider of each episode, a closed session record lists the issues
//! it moved, and a peer's claim names what it is working on and where. This module joins them
//! to the intents. It stores nothing: every link is derived on read, so a session that ends or
//! a provider that changes leaves the intent exactly where it was.
//!
//! **Every link says how it is known**, because a join that flattened a guess into a fact would
//! make inferred lineage read as declared intent:
//!
//! | provenance | link                                                              |
//! |------------|-------------------------------------------------------------------|
//! | `declared` | the work names the issue itself: a claim or a task title citing it |
//! | `observed` | the ledger or a session record shows the episode moving the issue  |
//! | `derived`  | the branch the work ran on names the issue                          |
//! | `inferred` | nothing stronger exists and an open issue's scope overlaps the work |
//!
//! And for each intent it answers what remains: the criteria without current evidence with the
//! issues serving each, and the drift between closed work and unproven reality — every milestone
//! DONE while a criterion's evidence is stale or failing is closed work that reality contradicts,
//! reported as `closed_work_contradicted` rather than hidden behind a closed plan. An intent that
//! was satisfied and regresses shows exactly this: its stage falls back to `verifying` and the
//! criterion that stopped holding is named.
//!
//! ```
//! use majordomus_cli::intent_realization::IntentLinkProvenance;
//! // declared is the strongest link and inferred the weakest; a join keeps the strongest
//! assert!(IntentLinkProvenance::Declared < IntentLinkProvenance::Inferred);
//! assert_eq!(IntentLinkProvenance::Observed.as_str(), "observed");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::builtin::continuity::{document, read_task};
use crate::index::Index;
use crate::intent::{
    IntentEvidenceState, IntentFinding, IntentStage, IntentView, Intents, WARN,
};
use crate::intent_plan::{CoverageStrength, CriterionCoverage, IntentCoverage};
use crate::ledger::Entry;
use crate::peers::Peer;
use crate::plan::{overlap, Plan};
use crate::worktree::state::issue_of;

/// The command that shows every realization finding again.
pub const REPRODUCE: &str = "majordomus intent realization";

/// Where the untracked half of the layer lives, relative to the repository root.
const STATE_DIR: &str = ".ai/local/state";

// ---------------------------------------------------------------- vocabulary

/// How a link from a piece of work to an issue is known, strongest first.
///
/// ```
/// use majordomus_cli::intent_realization::IntentLinkProvenance;
/// assert_eq!(
///     serde_json::to_string(&IntentLinkProvenance::Inferred).unwrap(),
///     "\"inferred\""
/// );
/// assert!(IntentLinkProvenance::Observed < IntentLinkProvenance::Derived);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntentLinkProvenance {
    /// The work names the issue itself.
    Declared,
    /// A recorded event shows the work's episode moving the issue.
    Observed,
    /// The branch the work ran on names the issue, by the worktree naming rule.
    Derived,
    /// Nothing stronger exists, and an open issue's scope overlaps the work's scope.
    Inferred,
}

impl IntentLinkProvenance {
    /// The word every surface prints.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::IntentLinkProvenance;
    /// assert_eq!(IntentLinkProvenance::Declared.as_str(), "declared");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentLinkProvenance::Declared => "declared",
            IntentLinkProvenance::Observed => "observed",
            IntentLinkProvenance::Derived => "derived",
            IntentLinkProvenance::Inferred => "inferred",
        }
    }
}

/// Which record a unit of work was read from.
///
/// ```
/// use majordomus_cli::intent_realization::IntentWorkKind;
/// assert_eq!(serde_json::to_string(&IntentWorkKind::PeerClaim).unwrap(), "\"peer_claim\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntentWorkKind {
    /// A task of this checkout's ledger: the durable unit a handover continues, across every
    /// episode that worked on it.
    Task,
    /// A claim a peer announced on the shared server's board.
    PeerClaim,
    /// A closed session record of the shared layer whose episode this checkout's ledger does
    /// not hold — another machine's or another checkout's work.
    SessionRecord,
}

/// The fact a link was read from.
///
/// ```
/// use majordomus_cli::intent_realization::IntentLinkVia;
/// assert_eq!(serde_json::to_string(&IntentLinkVia::BranchIssue).unwrap(), "\"branch_issue\"");
/// assert_eq!(IntentLinkVia::ScopeOverlap.provenance().as_str(), "inferred");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntentLinkVia {
    /// The claim or the task's own words cite the issue by id.
    NamedIssue,
    /// A plan transition in the work's episode, or the session record's `issues`.
    MovedIssue,
    /// The branch name carries the issue id.
    BranchIssue,
    /// An open issue's scope overlaps the work's scope.
    ScopeOverlap,
}

impl IntentLinkVia {
    /// The provenance this kind of fact carries. One mapping, so no reader can grade the same
    /// fact two ways.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::{IntentLinkProvenance, IntentLinkVia};
    /// assert_eq!(IntentLinkVia::MovedIssue.provenance(), IntentLinkProvenance::Observed);
    /// ```
    pub fn provenance(self) -> IntentLinkProvenance {
        match self {
            IntentLinkVia::NamedIssue => IntentLinkProvenance::Declared,
            IntentLinkVia::MovedIssue => IntentLinkProvenance::Observed,
            IntentLinkVia::BranchIssue => IntentLinkProvenance::Derived,
            IntentLinkVia::ScopeOverlap => IntentLinkProvenance::Inferred,
        }
    }
}

// ---------------------------------------------------------------- the work, as recorded

/// One execution episode of a piece of work, and the provider that ran it when a record says.
/// A provider is never guessed: an episode opened by hand has none.
///
/// ```
/// use majordomus_cli::intent_realization::IntentEpisode;
/// let e = IntentEpisode {
///     session: "s-1".into(),
///     provider: "codex".into(),
///     provider_session: String::new(),
/// };
/// assert_eq!(e.provider, "codex");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct IntentEpisode {
    /// The episode id, `s-...`; empty for a peer's connection, which is not an episode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session: String,
    /// The provider, as its own events or the peer's client name it; empty when unknown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// The provider's own session identity, when recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
}

/// One unit of work as its records describe it, before it is joined to any intent.
///
/// ```
/// use majordomus_cli::intent_realization::{IntentWorkKind, IntentWorkUnit};
/// let unit = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
/// assert!(unit.episodes.is_empty());
/// assert_eq!(unit.id, "t-1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentWorkUnit {
    /// Which record it was read from.
    pub kind: IntentWorkKind,
    /// Its identity in that record: a task id, a session id, or `<peer>/<claim>`.
    pub id: String,
    /// What it says it is, in its own words.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Where it stands: a task's outcome, a session's, or `attached`/`departed` for a claim.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub outcome: String,
    /// The branches it ran on, in first-seen order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<String>,
    /// The paths it claims.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// Every episode that worked on it, in first-seen order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<IntentEpisode>,
    /// The handovers that carried it from one episode to the next, repository-relative.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handovers: Vec<String>,
    /// Issue ids its own words cite.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_issues: Vec<String>,
    /// Issue ids a recorded event shows it moving.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved_issues: Vec<String>,
}

impl IntentWorkUnit {
    /// An empty unit of a kind and id, for a reader to fill.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::{IntentWorkKind, IntentWorkUnit};
    /// let u = IntentWorkUnit::new(IntentWorkKind::PeerClaim, "p3/x");
    /// assert_eq!(u.kind, IntentWorkKind::PeerClaim);
    /// ```
    pub fn new(kind: IntentWorkKind, id: &str) -> IntentWorkUnit {
        IntentWorkUnit {
            kind,
            id: id.to_string(),
            title: String::new(),
            outcome: String::new(),
            branches: Vec::new(),
            scope: Vec::new(),
            episodes: Vec::new(),
            handovers: Vec::new(),
            named_issues: Vec::new(),
            moved_issues: Vec::new(),
        }
    }

    /// The providers that ran it, each once, in first-seen order.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::{IntentEpisode, IntentWorkKind, IntentWorkUnit};
    /// let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
    /// for p in ["claude-code", "codex", "claude-code", ""] {
    ///     u.episodes.push(IntentEpisode {
    ///         session: String::new(), provider: p.into(), provider_session: String::new() });
    /// }
    /// assert_eq!(u.providers(), ["claude-code", "codex"]);
    /// ```
    pub fn providers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in &self.episodes {
            if !e.provider.is_empty() && !out.contains(&e.provider) {
                out.push(e.provider.clone());
            }
        }
        out
    }
}

fn push_once(list: &mut Vec<String>, value: &str) {
    if !value.is_empty() && !list.iter().any(|v| v == value) {
        list.push(value.to_string());
    }
}

// ---------------------------------------------------------------- the join

/// One link from a piece of work to an intent: through which issue and milestone, serving
/// which of its criteria, read from which fact, known how.
///
/// ```
/// use majordomus_cli::intent::IntentStage;
/// use majordomus_cli::intent_realization::{IntentLink, IntentLinkProvenance, IntentLinkVia};
/// let l = IntentLink {
///     intent: "intent-lifecycle".into(), stage: IntentStage::Executing,
///     milestone: "intent-lifecycle".into(), issue: "I1900".into(),
///     criteria: vec!["stage-is-derived".into()],
///     via: IntentLinkVia::BranchIssue, provenance: IntentLinkProvenance::Derived,
/// };
/// assert_eq!(l.provenance, l.via.provenance());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentLink {
    /// The intent.
    pub intent: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// The milestone that links the issue to it.
    pub milestone: String,
    /// The issue that links the work to that milestone.
    pub issue: String,
    /// The criteria of this intent the issue declares it serves; empty when it declares none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub criteria: Vec<String>,
    /// The fact the link was read from.
    pub via: IntentLinkVia,
    /// How the link is known, from `via`.
    pub provenance: IntentLinkProvenance,
}

/// A unit of work with every intent it realises, or the reason it realises none.
///
/// ```
/// use majordomus_cli::intent_realization::{IntentRealizedWork, IntentWorkKind, IntentWorkUnit};
/// let w = IntentRealizedWork {
///     work: IntentWorkUnit::new(IntentWorkKind::Task, "t-1"),
///     links: vec![],
///     unlinked: Some("it names no issue".into()),
/// };
/// assert!(w.links.is_empty() && w.unlinked.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentRealizedWork {
    /// The work, as recorded.
    pub work: IntentWorkUnit,
    /// Every intent it realises, strongest link per issue.
    pub links: Vec<IntentLink>,
    /// When it realises none: the first link that is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlinked: Option<String>,
}

/// The ids in a text that are issues of the plan, each once, in order of appearance.
///
/// ```
/// use majordomus_cli::intent_realization::issue_tokens;
/// let ids = ["I1900".to_string(), "I0001".to_string()];
/// assert_eq!(issue_tokens("finish I1900; not XI1900 or I19000", &ids), ["I1900"]);
/// ```
pub fn issue_tokens(text: &str, ids: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        if ids.iter().any(|i| i == word) {
            push_once(&mut out, word);
        }
    }
    out
}

/// Join one unit of work to the intents, through the plan.
///
/// The candidate issues are, strongest first: those its words cite, those a recorded event shows
/// it moving, the one its branch names. Only when none of those exists are the open issues whose
/// scope overlaps its scope taken, and they are marked `inferred`. Each issue is followed to its
/// milestone and the milestone to the intents naming it, keeping the strongest link per issue.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_realization::{link, IntentWorkKind, IntentWorkUnit};
/// # let plan: majordomus_cli::plan::Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let intents = Intents { intents: vec![], findings: vec![] };
/// let w = link(IntentWorkUnit::new(IntentWorkKind::Task, "t-1"), &intents, &plan);
/// // nothing to follow is itself the answer, and it says why
/// assert!(w.links.is_empty());
/// assert!(w.unlinked.unwrap().contains("names no issue"));
/// ```
pub fn link(unit: IntentWorkUnit, intents: &Intents, plan: &Plan) -> IntentRealizedWork {
    let ids: Vec<String> = plan.issues.iter().map(|i| i.id.clone()).collect();
    let mut best: BTreeMap<String, IntentLinkVia> = BTreeMap::new();
    let mut offer = |issue: &str, via: IntentLinkVia| {
        if !ids.iter().any(|i| i == issue) {
            return;
        }
        let slot = best.entry(issue.to_string()).or_insert(via);
        if via.provenance() < slot.provenance() {
            *slot = via;
        }
    };
    for i in &unit.named_issues {
        offer(i, IntentLinkVia::NamedIssue);
    }
    for i in &unit.moved_issues {
        offer(i, IntentLinkVia::MovedIssue);
    }
    for b in &unit.branches {
        if let Some(i) = issue_of(b, &ids) {
            offer(&i, IntentLinkVia::BranchIssue);
        }
    }
    if best.is_empty() {
        for i in &plan.issues {
            if matches!(i.status.as_str(), "DONE" | "CANCELLED" | "SUPERSEDED") {
                continue;
            }
            if i.scope.iter().any(|s| unit.scope.iter().any(|p| overlap(s, p))) {
                best.insert(i.id.clone(), IntentLinkVia::ScopeOverlap);
            }
        }
    }

    let mut links = Vec::new();
    let mut unserved: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (issue, via) in &best {
        let Some(pi) = plan.issue(issue) else {
            continue;
        };
        let serving = intents.serving(&pi.milestone);
        if serving.is_empty() {
            unserved.entry(pi.milestone.clone()).or_default().push(issue.clone());
        }
        for intent in serving {
            let prefix = format!("{}#", intent.id);
            links.push(IntentLink {
                intent: intent.id.clone(),
                stage: intent.stage,
                milestone: pi.milestone.clone(),
                issue: issue.clone(),
                criteria: pi
                    .serves
                    .iter()
                    .filter_map(|s| s.strip_prefix(&prefix).map(str::to_string))
                    .collect(),
                via: *via,
                provenance: via.provenance(),
            });
        }
    }
    links.sort_by(|a, b| {
        (a.provenance, &a.intent, &a.issue).cmp(&(b.provenance, &b.intent, &b.issue))
    });

    let unlinked = if !links.is_empty() {
        None
    } else if best.is_empty() {
        let branches = if unit.branches.is_empty() {
            "it ran on no recorded branch".to_string()
        } else {
            format!("its branch {} names none", unit.branches.join(", "))
        };
        let scope = if unit.scope.is_empty() {
            "it claims no scope".to_string()
        } else {
            format!("no open issue's scope covers {}", unit.scope.join(", "))
        };
        Some(format!(
            "it names no issue, no recorded event shows it moving one, {branches}, and {scope}"
        ))
    } else {
        let named: Vec<String> = unserved
            .iter()
            .map(|(m, issues)| format!("{m} ({})", issues.join(", ")))
            .collect();
        Some(format!(
            "its issues belong to milestones no intent names: {}",
            named.join("; ")
        ))
    };
    IntentRealizedWork {
        work: unit,
        links,
        unlinked,
    }
}

// ---------------------------------------------------------------- per intent

/// A criterion an intent still lacks, with the issues that declare they serve it.
///
/// ```
/// use majordomus_cli::intent::IntentEvidenceState;
/// use majordomus_cli::intent_realization::IntentUnmetCriterion;
/// let c = IntentUnmetCriterion {
///     id: "surfaces-agree".into(), criterion: "the surfaces agree".into(),
///     evidence: "test".into(), state: IntentEvidenceState::Stale,
///     reproduce: None, issues: vec!["I1900".into()],
/// };
/// assert_eq!(c.state, IntentEvidenceState::Stale);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentUnmetCriterion {
    /// The criterion id.
    pub id: String,
    /// What must be observably true.
    pub criterion: String,
    /// The kind of evidence that settles it.
    pub evidence: String,
    /// Why it is not met.
    pub state: IntentEvidenceState,
    /// The command that produces its evidence, when one is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// The issues declaring they serve it, in plan order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
}

/// A unit of work as one intent sees it: which record, how it stands, and the strongest link.
///
/// ```
/// use majordomus_cli::intent_realization::{IntentLinkProvenance, IntentWorkKind, IntentWorkRef};
/// let r = IntentWorkRef {
///     kind: IntentWorkKind::Task, id: "t-1".into(), outcome: "active".into(),
///     provenance: IntentLinkProvenance::Observed, providers: vec!["codex".into()],
///     handovers: 1,
/// };
/// assert_eq!(r.handovers, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentWorkRef {
    /// Which record.
    pub kind: IntentWorkKind,
    /// Its identity there.
    pub id: String,
    /// Where it stands.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub outcome: String,
    /// The strongest of its links to this intent.
    pub provenance: IntentLinkProvenance,
    /// The providers that ran it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// How many handovers carried it.
    pub handovers: usize,
}

/// One intent, realised: how far reality is from it, the work realising it and by whom, and the
/// drift between closed work and current evidence.
///
/// ```
/// use majordomus_cli::intent::IntentStage;
/// use majordomus_cli::intent_realization::IntentRealizationView;
/// let v = IntentRealizationView {
///     intent: "x".into(), title: "X".into(), stage: IntentStage::Verifying,
///     criteria: 2, met: 1, unmet: vec![], work: vec![], providers: vec![], findings: vec![],
/// };
/// assert!(v.met < v.criteria);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentRealizationView {
    /// The intent.
    pub intent: String,
    /// Its title.
    pub title: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// How many criteria it declares.
    pub criteria: usize,
    /// How many have current evidence.
    pub met: usize,
    /// Every criterion without current evidence.
    pub unmet: Vec<IntentUnmetCriterion>,
    /// Every unit of work linked to it, strongest link first.
    pub work: Vec<IntentWorkRef>,
    /// Every provider that ran work linked to it, each once.
    pub providers: Vec<String>,
    /// Drift between the plan's closure and the evidence.
    pub findings: Vec<IntentFinding>,
}

fn warn(out: &mut Vec<IntentFinding>, code: &str, subject: &str, message: String) {
    out.push(IntentFinding {
        level: WARN.into(),
        code: code.into(),
        subject: subject.into(),
        message,
        reproduce: REPRODUCE.into(),
    });
}

fn state_words(state: IntentEvidenceState) -> &'static str {
    match state {
        IntentEvidenceState::Current => "current evidence",
        IntentEvidenceState::Stale => "evidence recorded against a source that has changed since",
        IntentEvidenceState::Failing => "a latest recorded run that did not pass",
        IntentEvidenceState::NotRun => "no recorded run",
        IntentEvidenceState::NotDerivable => {
            "evidence of a kind the ledger cannot derive, so it is never met by itself"
        }
        IntentEvidenceState::Unresolved => "a reference that names nothing this repository holds",
    }
}

/// The drift of one intent: the plan's closure against the evidence's verdict.
///
/// * `closed_work_contradicted` — every milestone is DONE and a criterion's recorded evidence
///   says no: stale against a source that has changed, or failing. A satisfied intent whose
///   reality regresses lands here.
/// * `closed_work_unproven` — every milestone is DONE and a criterion has never had evidence.
/// * `criterion_closed_unmet` — every issue declaring it serves a criterion is DONE, and the
///   criterion is not met, while the intent is still being executed.
///
/// ```
/// use majordomus_cli::intent::{IntentCriterion, IntentEvidenceState, IntentStage, IntentView};
/// use majordomus_cli::intent_realization::drift;
/// # let plan: majordomus_cli::plan::Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let view = IntentView {
///     id: "x".into(), title: "X".into(), statement: String::new(), invariants: vec![],
///     stage: IntentStage::Verifying, milestones: vec![], met: 0,
///     satisfaction: vec![IntentCriterion {
///         id: "c".into(), criterion: "c".into(), evidence: "test".into(),
///         reference: "t".into(), state: IntentEvidenceState::Failing, met: false,
///         reproduce: None }],
///     governance: vec![], non_goals: vec![], superseded_by: None, source: String::new(),
/// };
/// let found = drift(&view, &plan);
/// assert_eq!(found[0].code, "closed_work_contradicted");
/// ```
pub fn drift(view: &IntentView, plan: &Plan) -> Vec<IntentFinding> {
    let mut out = Vec::new();
    for c in view.satisfaction.iter().filter(|c| !c.met) {
        let key = format!("{}#{}", view.id, c.id);
        let serving: Vec<&crate::plan::PlanIssue> =
            plan.issues.iter().filter(|i| i.serves.contains(&key)).collect();
        match view.stage {
            IntentStage::Verifying
                if matches!(
                    c.state,
                    IntentEvidenceState::Stale | IntentEvidenceState::Failing
                ) =>
            {
                warn(
                    &mut out,
                    "closed_work_contradicted",
                    &view.id,
                    format!(
                        "every milestone is DONE and criterion `{}` has {}: the work closed, and \
                         the evidence says the intent is not true",
                        c.id,
                        state_words(c.state)
                    ),
                );
            }
            IntentStage::Verifying => warn(
                &mut out,
                "closed_work_unproven",
                &view.id,
                format!(
                    "every milestone is DONE and criterion `{}` has {}: closing the work did not \
                     prove the intent",
                    c.id,
                    state_words(c.state)
                ),
            ),
            IntentStage::Planned | IntentStage::Executing
                if !serving.is_empty() && serving.iter().all(|i| i.status == "DONE") =>
            {
                let ids: Vec<&str> = serving.iter().map(|i| i.id.as_str()).collect();
                warn(
                    &mut out,
                    "criterion_closed_unmet",
                    &view.id,
                    format!(
                        "every issue serving criterion `{}` ({}) is DONE and it has {}",
                        c.id,
                        ids.join(", "),
                        state_words(c.state)
                    ),
                );
            }
            _ => {}
        }
    }
    out
}

/// Every intent, realised, and every unit of work, joined: what `majordomus intent realization`
/// answers.
///
/// ```
/// use majordomus_cli::intent_realization::IntentRealization;
/// let r = IntentRealization { intents: vec![], work: vec![], orphans: 0, findings: vec![] };
/// assert_eq!(r.orphans, 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentRealization {
    /// Every intent, in identity order.
    pub intents: Vec<IntentRealizationView>,
    /// Every unit of work read, with its links.
    pub work: Vec<IntentRealizedWork>,
    /// How many units of work realise no intent.
    pub orphans: usize,
    /// Every finding: each intent's drift, then the live work that serves no intent.
    pub findings: Vec<IntentFinding>,
}

/// Join every unit of work to every intent. Pure: the records are already read.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// use majordomus_cli::intent_realization::{realize, IntentWorkKind, IntentWorkUnit};
/// # let plan: majordomus_cli::plan::Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let mut live = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
/// live.outcome = "active".into();
/// let r = realize(&Intents { intents: vec![], findings: vec![] }, &plan, vec![live]);
/// // live work that serves no intent is an orphan, and is named
/// assert_eq!(r.orphans, 1);
/// assert_eq!(r.findings[0].code, "work_serves_no_intent");
/// ```
pub fn realize(intents: &Intents, plan: &Plan, units: Vec<IntentWorkUnit>) -> IntentRealization {
    let work: Vec<IntentRealizedWork> = units.into_iter().map(|u| link(u, intents, plan)).collect();
    let mut findings = Vec::new();
    let mut views = Vec::with_capacity(intents.intents.len());
    for view in &intents.intents {
        let mut refs: Vec<IntentWorkRef> = work
            .iter()
            .filter_map(|w| {
                let provenance = w
                    .links
                    .iter()
                    .filter(|l| l.intent == view.id)
                    .map(|l| l.provenance)
                    .min()?;
                Some(IntentWorkRef {
                    kind: w.work.kind,
                    id: w.work.id.clone(),
                    outcome: w.work.outcome.clone(),
                    provenance,
                    providers: w.work.providers(),
                    handovers: w.work.handovers.len(),
                })
            })
            .collect();
        refs.sort_by(|a, b| (a.provenance, a.kind, &a.id).cmp(&(b.provenance, b.kind, &b.id)));
        let mut providers = Vec::new();
        for r in &refs {
            for p in &r.providers {
                push_once(&mut providers, p);
            }
        }
        let unmet = view
            .satisfaction
            .iter()
            .filter(|c| !c.met)
            .map(|c| {
                let key = format!("{}#{}", view.id, c.id);
                IntentUnmetCriterion {
                    id: c.id.clone(),
                    criterion: c.criterion.clone(),
                    evidence: c.evidence.clone(),
                    state: c.state,
                    reproduce: c.reproduce.clone(),
                    issues: plan
                        .issues
                        .iter()
                        .filter(|i| i.serves.contains(&key))
                        .map(|i| i.id.clone())
                        .collect(),
                }
            })
            .collect();
        let drifted = drift(view, plan);
        findings.extend(drifted.iter().cloned());
        views.push(IntentRealizationView {
            intent: view.id.clone(),
            title: view.title.clone(),
            stage: view.stage,
            criteria: view.satisfaction.len(),
            met: view.met,
            unmet,
            work: refs,
            providers,
            findings: drifted,
        });
    }
    let mut orphans = 0;
    for w in &work {
        let Some(why) = &w.unlinked else {
            continue;
        };
        orphans += 1;
        let live = match w.work.kind {
            IntentWorkKind::Task => w.work.outcome == "active",
            IntentWorkKind::PeerClaim => w.work.outcome == "attached",
            IntentWorkKind::SessionRecord => false,
        };
        if live {
            warn(
                &mut findings,
                "work_serves_no_intent",
                &w.work.id,
                format!(
                    "live work serves no declared intent: {why}. Maintenance may; work meant to \
                     realise an intent names its issue"
                ),
            );
        }
    }
    IntentRealization {
        intents: views,
        work,
        orphans,
        findings,
    }
}

// ---------------------------------------------------------------- explain

/// Why an intent stands where it stands: the record, each sentence that explains the stage and
/// each criterion, the plan's coverage of its criteria, and the work realising it.
///
/// ```
/// use majordomus_cli::intent_realization::IntentExplanation;
/// // every sentence is derived; an explanation holds no authored status
/// let fields = serde_json::to_value(schemars::schema_for!(IntentExplanation)).unwrap();
/// assert!(fields["properties"]["because"].is_object());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentExplanation {
    /// The intent, derived.
    pub intent: IntentView,
    /// One sentence per fact the stage rests on: the stage, each criterion, the work.
    pub because: Vec<String>,
    /// The plan's coverage of each of its criteria.
    pub coverage: Vec<CriterionCoverage>,
    /// Its realization.
    pub realization: IntentRealizationView,
    /// Every unit of work linked to it, with its links.
    pub work: Vec<IntentRealizedWork>,
}

fn strength_words(s: CoverageStrength) -> &'static str {
    match s {
        CoverageStrength::Uncovered => "no live issue serves it",
        CoverageStrength::Observed => "the recorded gap observed it already true",
        CoverageStrength::Weak => "the issues serving it owe no evidence",
        CoverageStrength::Covered => "an issue serving it owes evidence before it is DONE",
    }
}

/// Explain one intent out of the derivations already made.
///
/// ```
/// use majordomus_cli::intent::{IntentStage, IntentView, Intents};
/// use majordomus_cli::intent_plan::IntentCoverage;
/// use majordomus_cli::intent_realization::{explain, realize};
/// # let plan: majordomus_cli::plan::Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let view = IntentView {
///     id: "x".into(), title: "X".into(), statement: String::new(), invariants: vec![],
///     stage: IntentStage::Declared, milestones: vec![], met: 0, satisfaction: vec![],
///     governance: vec![], non_goals: vec![], superseded_by: None, source: String::new(),
/// };
/// let intents = Intents { intents: vec![view.clone()], findings: vec![] };
/// let r = realize(&intents, &plan, vec![]);
/// let coverage = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
/// let e = explain(&view, &coverage, &r);
/// assert!(e.because[0].starts_with("declared"));
/// ```
pub fn explain(
    view: &IntentView,
    coverage: &IntentCoverage,
    realization: &IntentRealization,
) -> IntentExplanation {
    let mut because = Vec::new();
    let statuses: Vec<String> = view
        .milestones
        .iter()
        .map(|m| match &m.status {
            Some(s) => format!("{} is {s}", m.id),
            None => format!("{} does not resolve", m.id),
        })
        .collect();
    let total = view.satisfaction.len();
    because.push(match view.stage {
        IntentStage::Declared if view.milestones.is_empty() => {
            "declared: it names no milestone, so no work realises it".to_string()
        }
        IntentStage::Declared => format!("declared: {}", statuses.join(", ")),
        IntentStage::Planned => format!(
            "planned: every milestone resolves and none has started ({})",
            statuses.join(", ")
        ),
        IntentStage::Executing => format!("executing: {}", statuses.join(", ")),
        IntentStage::Verifying => format!(
            "verifying: every milestone is DONE and {} of {total} criteria have current \
             evidence; closed work is not a satisfied intent",
            view.met
        ),
        IntentStage::Satisfied => format!(
            "satisfied: every milestone is DONE and all {total} criteria have current evidence, \
             re-derived on this read"
        ),
        IntentStage::Cancelled => "cancelled: the record says so".to_string(),
        IntentStage::Superseded => format!(
            "superseded by {}",
            view.superseded_by.clone().unwrap_or_default()
        ),
    });
    let mine: Vec<CriterionCoverage> = coverage
        .criteria
        .iter()
        .filter(|c| c.intent == view.id)
        .cloned()
        .collect();
    for c in &view.satisfaction {
        let mut s = if c.met {
            format!("`{}` is met: its {} `{}` has current evidence", c.id, c.evidence, c.reference)
        } else {
            format!(
                "`{}` is not met: its {} `{}` has {}",
                c.id,
                c.evidence,
                c.reference,
                state_words(c.state)
            )
        };
        if let Some(cov) = mine.iter().find(|k| k.criterion == c.id) {
            let ids: Vec<&str> = cov.issues.iter().map(|i| i.id.as_str()).collect();
            s.push_str(&format!("; {}", strength_words(cov.strength)));
            if !ids.is_empty() {
                s.push_str(&format!(" ({})", ids.join(", ")));
            }
        }
        if !c.met {
            if let Some(r) = &c.reproduce {
                s.push_str(&format!("; reproduce: {r}"));
            }
        }
        because.push(s);
    }
    let rv = realization
        .intents
        .iter()
        .find(|v| v.intent == view.id)
        .cloned()
        .unwrap_or_else(|| IntentRealizationView {
            intent: view.id.clone(),
            title: view.title.clone(),
            stage: view.stage,
            criteria: total,
            met: view.met,
            unmet: Vec::new(),
            work: Vec::new(),
            providers: Vec::new(),
            findings: Vec::new(),
        });
    because.push(if rv.work.is_empty() {
        "no recorded task, session or live claim realises it".to_string()
    } else {
        let handovers: usize = rv.work.iter().map(|w| w.handovers).sum();
        format!(
            "{} unit(s) of work realise it across {} handover(s){}",
            rv.work.len(),
            handovers,
            if rv.providers.is_empty() {
                String::new()
            } else {
                format!(", run by {}", rv.providers.join(", "))
            }
        )
    });
    for f in &rv.findings {
        because.push(format!("{}: {}", f.code, f.message));
    }
    let work = realization
        .work
        .iter()
        .filter(|w| w.links.iter().any(|l| l.intent == view.id))
        .cloned()
        .collect();
    IntentExplanation {
        intent: view.clone(),
        because,
        coverage: mine,
        realization: rv,
        work,
    }
}

// ---------------------------------------------------------------- reading the records

fn payload_str(e: &Entry, key: &str) -> String {
    e.payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// The tasks of a ledger, each with every episode that worked on it, the provider of each
/// episode where a record names one, the handovers that carried it, and the issues its episodes
/// moved. `providers` maps an episode to its provider and provider session, as the open episode
/// files name them; the ledger's own provider events add to it.
///
/// A plan transition is attributed to the task whose event last preceded it in the same episode,
/// so an episode that ran two tasks in turn does not credit the first with the second's issues.
///
/// ```
/// use majordomus_cli::intent_realization::tasks_from_ledger;
/// use majordomus_cli::ledger::Entry;
/// let line = |s: &str| serde_json::from_str::<Entry>(s).unwrap();
/// let env = r#""head":"h","by":"majordomus/0""#;
/// let entries = vec![
///     line(&format!(r#"{{"ts":"1","event":"task.started",{env},"branch":"m","session":"s-a",
///         "task_id":"t-1","scope":"lib docs"}}"#)),
///     line(&format!(r#"{{"ts":"2","event":"provider.event.received",{env},"branch":"m",
///         "session":"s-a","provider":"claude-code","provider_session":"x"}}"#)),
///     line(&format!(r#"{{"ts":"3","event":"task.handed_over",{env},"branch":"m","session":"s-a",
///         "task_id":"t-1","handover_path":"h.md"}}"#)),
///     line(&format!(r#"{{"ts":"4","event":"plan_start",{env},"branch":"m","session":"s-b",
///         "issue":"I0001"}}"#)),
///     line(&format!(r#"{{"ts":"5","event":"task.checkpoint",{env},"branch":"m","session":"s-b",
///         "task_id":"t-1"}}"#)),
///     line(&format!(r#"{{"ts":"6","event":"plan_done",{env},"branch":"m","session":"s-b",
///         "issue":"I0002"}}"#)),
/// ];
/// let tasks = tasks_from_ledger(&entries, &Default::default());
/// let t = &tasks[0];
/// assert_eq!(t.scope, ["lib", "docs"]);
/// assert_eq!(t.handovers, ["h.md"]);
/// assert_eq!(t.providers(), ["claude-code"]);
/// // I0001 moved before t-1 was resumed in s-b, so only I0002 is its own
/// assert_eq!(t.moved_issues, ["I0002"]);
/// assert_eq!(t.outcome, "active");
/// ```
pub fn tasks_from_ledger(
    entries: &[Entry],
    providers: &BTreeMap<String, (String, String)>,
) -> Vec<IntentWorkUnit> {
    let mut providers = providers.clone();
    // the episode's own start line names its provider since ADR 0075; a provider event names it
    // for episodes opened before that
    for e in entries.iter().filter(|e| {
        matches!(e.event.as_str(), "session.started" | "provider.event.received")
    }) {
        let provider = payload_str(e, "provider");
        if let (Some(s), false) = (&e.session, provider.is_empty()) {
            providers
                .entry(s.clone())
                .or_insert_with(|| (provider, payload_str(e, "provider_session")));
        }
    }
    let mut order: Vec<String> = Vec::new();
    let mut units: BTreeMap<String, IntentWorkUnit> = BTreeMap::new();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for e in entries {
        let task_id = payload_str(e, "task_id");
        if e.event.starts_with("task.") && !task_id.is_empty() {
            let unit = units.entry(task_id.clone()).or_insert_with(|| {
                order.push(task_id.clone());
                IntentWorkUnit::new(IntentWorkKind::Task, &task_id)
            });
            push_once(&mut unit.branches, &e.branch);
            if let Some(s) = &e.session {
                current.insert(s.clone(), task_id.clone());
                if !unit.episodes.iter().any(|x| &x.session == s) {
                    let (p, ps) = providers.get(s).cloned().unwrap_or_default();
                    unit.episodes.push(IntentEpisode {
                        session: s.clone(),
                        provider: p,
                        provider_session: ps,
                    });
                }
            }
            match e.event.as_str() {
                "task.started" => {
                    unit.outcome = "active".into();
                    for p in payload_str(e, "scope").split_whitespace() {
                        push_once(&mut unit.scope, p);
                    }
                }
                "task.checkpoint" => unit.outcome = "active".into(),
                "task.handed_over" => {
                    unit.outcome = "handed_over".into();
                    push_once(&mut unit.handovers, &payload_str(e, "handover_path"));
                }
                "task.finished" => {
                    let o = payload_str(e, "outcome");
                    unit.outcome = if o.is_empty() { "finished".into() } else { o };
                }
                _ => {}
            }
        } else if e.event.starts_with("plan_") {
            let issue = payload_str(e, "issue");
            let owner = e.session.as_ref().and_then(|s| current.get(s));
            if let (Some(t), false) = (owner, issue.is_empty()) {
                if let Some(unit) = units.get_mut(t) {
                    push_once(&mut unit.moved_issues, &issue);
                }
            }
        }
    }
    order
        .into_iter()
        .filter_map(|id| units.remove(&id))
        .collect()
}

/// Every unit of work this checkout can see, and the ledger lines it could not read.
///
/// The tasks of this checkout's ledger, titled from the active task or its archive; the closed
/// session records of the shared layer whose episode no task here already holds; and every claim
/// on the peer board. Nothing is written.
///
/// ```
/// use majordomus_cli::intent_realization::gather;
/// # fn demo(index: &majordomus_cli::index::Index) {
/// let (units, skipped) = gather(std::path::Path::new("/nonexistent"), index, &[]);
/// // a checkout whose lifecycle never ran still has the shared layer's session records
/// assert_eq!(skipped, 0);
/// let _ = units;
/// # }
/// ```
pub fn gather(root: &Path, index: &Index, peers: &[Peer]) -> (Vec<IntentWorkUnit>, usize) {
    let dir = root.join(STATE_DIR);
    let (entries, skipped) = crate::ledger::read(root);

    let mut providers: BTreeMap<String, (String, String)> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(dir.join("sessions-open")) {
        for f in rd.flatten() {
            if let Some(d) = document(&f.path()) {
                if let Some(s) = d.get("session_id") {
                    providers.insert(
                        s.clone(),
                        (
                            d.get("provider").cloned().unwrap_or_default(),
                            d.get("provider_session").cloned().unwrap_or_default(),
                        ),
                    );
                }
            }
        }
    }
    let mut units = tasks_from_ledger(&entries, &providers);
    let ids: Vec<String> = crate::plan::Plan::build(index)
        .issues
        .iter()
        .map(|i| i.id.clone())
        .collect();

    // the active task first: it may predate this ledger's reader, and it carries the title
    let active = read_task(&dir.join("current.yaml"));
    if let Some(a) = &active {
        if !units.iter().any(|u| u.id == a.id) {
            let mut u = IntentWorkUnit::new(IntentWorkKind::Task, &a.id);
            u.outcome = if a.outcome.is_empty() { "active".into() } else { a.outcome.clone() };
            units.push(u);
        }
    }
    for u in &mut units {
        let record = match &active {
            Some(a) if a.id == u.id => Some(a.clone()),
            _ => read_task(&dir.join("archive").join(format!("{}.yaml", u.id))),
        };
        if let Some(r) = record {
            u.title = r.task.clone();
            for p in &r.scope {
                push_once(&mut u.scope, p);
            }
        }
        for i in issue_tokens(&u.title, &ids) {
            push_once(&mut u.named_issues, &i);
        }
    }

    let held: BTreeSet<String> = units
        .iter()
        .flat_map(|u| u.episodes.iter().map(|e| e.session.clone()))
        .collect();
    let strings = |v: Option<&serde_json::Value>| -> Vec<String> {
        v.and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    for o in index.objects.iter().filter(|o| o.kind == "session") {
        let m = &o.metadata;
        let sid = m.get("session_id").and_then(|v| v.as_str()).unwrap_or_default();
        if sid.is_empty() || held.contains(sid) {
            continue;
        }
        let mut u = IntentWorkUnit::new(IntentWorkKind::SessionRecord, sid);
        u.title = m.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        u.outcome = m.get("outcome").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        push_once(
            &mut u.branches,
            m.get("branch").and_then(|v| v.as_str()).unwrap_or_default(),
        );
        u.episodes.push(IntentEpisode {
            session: sid.to_string(),
            provider: String::new(),
            provider_session: String::new(),
        });
        // the paths its episode left changed are what it touched: an observation, still only
        // ever an `inferred` link, because touching a path is not serving the issue that owns it
        for p in strings(m.get("changed_files")) {
            push_once(&mut u.scope, &p);
        }
        u.handovers = strings(m.get("handovers"));
        u.moved_issues = strings(m.get("issues"));
        units.push(u);
    }

    for p in peers {
        for c in &p.claims {
            let id = match &c.name {
                Some(n) => format!("{}/{n}", p.id),
                None => p.id.to_string(),
            };
            let mut u = IntentWorkUnit::new(IntentWorkKind::PeerClaim, &id);
            u.title = c.intent.clone();
            u.outcome = if p.attached { "attached" } else { "departed" }.into();
            u.scope = c.scope.clone();
            if let Some(b) = p.checkout.as_ref().and_then(|k| k.branch.as_deref()) {
                push_once(&mut u.branches, b);
            }
            u.episodes.push(IntentEpisode {
                session: String::new(),
                provider: p.client.name.clone(),
                provider_session: String::new(),
            });
            let words = format!("{} {}", c.name.clone().unwrap_or_default(), c.intent);
            u.named_issues = issue_tokens(&words, &ids);
            units.push(u);
        }
    }
    (units, skipped)
}
