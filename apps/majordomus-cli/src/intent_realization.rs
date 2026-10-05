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
    IntentEvidenceState, IntentFinding, IntentStage, IntentVerdictState, IntentView, Intents, WARN,
};
use crate::intent_plan::{CoverageStrength, CriterionCoverage, IntentCoverage};
use crate::ledger::Entry;
use crate::order::{canonical, OrderKey, Ordered};
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
    /// The word every surface prints for how this link is known.
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

impl IntentWorkKind {
    /// The word every surface prints for the record this work came from.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::IntentWorkKind;
    /// assert_eq!(IntentWorkKind::SessionRecord.as_str(), "session_record");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentWorkKind::Task => "task",
            IntentWorkKind::PeerClaim => "peer_claim",
            IntentWorkKind::SessionRecord => "session_record",
        }
    }
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
    /// The word every surface prints for the fact this link was read from.
    ///
    /// ```
    /// use majordomus_cli::intent_realization::IntentLinkVia;
    /// assert_eq!(IntentLinkVia::ScopeOverlap.as_str(), "scope_overlap");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentLinkVia::NamedIssue => "named_issue",
            IntentLinkVia::MovedIssue => "moved_issue",
            IntentLinkVia::BranchIssue => "branch_issue",
            IntentLinkVia::ScopeOverlap => "scope_overlap",
        }
    }

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

/// A unit of work's links read strongest first, then by intent, then by issue.
impl Ordered for IntentLink {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.intent, &self.issue).ranked(rank(self.provenance, None))
    }
}

/// The explicit position a link's provenance, and then the record a piece of work came from,
/// give it: provenance first, because the strongest link is the one a reader reads first.
fn rank(provenance: IntentLinkProvenance, kind: Option<IntentWorkKind>) -> i64 {
    let kind = kind.map_or(0, |k| k as i64 + 1);
    provenance as i64 * 8 + kind
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
        // the strongest fact an issue is offered by is its link, whatever the order offered
        let slot = best.entry(issue.to_string()).or_insert(via);
        *slot = std::cmp::min_by_key(*slot, via, |v| v.provenance());
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
            if i.scope
                .iter()
                .any(|s| unit.scope.iter().any(|p| overlap(s, p)))
            {
                best.insert(i.id.clone(), IntentLinkVia::ScopeOverlap);
            }
        }
    }

    let mut links = Vec::new();
    let mut unserved: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // every candidate is an issue of the plan: `offer` and the scope walk only take those
    for (pi, via) in best
        .iter()
        .filter_map(|(id, via)| plan.issue(id).map(|pi| (pi, via)))
    {
        let issue = &pi.id;
        let serving = intents.serving(&pi.milestone);
        if serving.is_empty() {
            unserved
                .entry(pi.milestone.clone())
                .or_default()
                .push(issue.clone());
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
    canonical(&mut links);

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

/// The work realising an intent reads strongest link first, then by the record it came from,
/// then by its identity.
impl Ordered for IntentWorkRef {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.id, &self.id).ranked(rank(self.provenance, Some(self.kind)))
    }
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
/// And the stage against the verdict, once per intent (ADR 0107):
///
/// * `evidence_ahead_of_plan` ([`EVIDENCE_AHEAD_OF_PLAN`]) — the verdict is `satisfied` while
///   the stage is `planned` or `executing`: the evidence settles the intent before the plan
///   closes it.
/// * `closed_work_not_satisfied` ([`CLOSED_WORK_NOT_SATISFIED`]) — every milestone is DONE (the
///   stage is `verifying`) and the verdict is `unsatisfied` or `unknown`, naming the criteria
///   that hold it back.
///
/// ```
/// use majordomus_cli::intent::{
///     verdict, IntentCriterion, IntentEvidenceState, IntentStage, IntentView,
/// };
/// use majordomus_cli::intent_realization::drift;
/// # let plan: majordomus_cli::plan::Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "issues": [], "waves": [], "edges": [],
/// #     "milestone_edges": [], "findings": []})).unwrap();
/// let satisfaction = vec![IntentCriterion {
///     id: "c".into(), criterion: "c".into(), evidence: "test".into(),
///     reference: "t".into(), state: IntentEvidenceState::Failing, met: false,
///     reproduce: None }];
/// let view = IntentView {
///     id: "x".into(), title: "X".into(), statement: String::new(), invariants: vec![],
///     stage: IntentStage::Verifying, milestones: vec![], met: 0,
///     verdict: verdict(&satisfaction), satisfaction,
///     governance: vec![], non_goals: vec![], superseded_by: None, source: String::new(),
/// };
/// let found = drift(&view, &plan);
/// assert_eq!(found[0].code, "closed_work_contradicted");
/// assert_eq!(found[1].code, "closed_work_not_satisfied");
/// ```
pub fn drift(view: &IntentView, plan: &Plan) -> Vec<IntentFinding> {
    let mut out = Vec::new();
    for c in view.satisfaction.iter().filter(|c| !c.met) {
        let key = format!("{}#{}", view.id, c.id);
        let serving: Vec<&crate::plan::PlanIssue> = plan
            .issues
            .iter()
            .filter(|i| i.serves.contains(&key))
            .collect();
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
    verdict_drift(view, &mut out);
    out
}

/// The drift code of an intent whose verdict is `satisfied` while its stage is `planned` or
/// `executing`: the evidence settles it before the plan closes it.
pub const EVIDENCE_AHEAD_OF_PLAN: &str = "evidence_ahead_of_plan";

/// The drift code of an intent whose milestones are all DONE (stage `verifying`) and whose
/// verdict is not `satisfied`.
pub const CLOSED_WORK_NOT_SATISFIED: &str = "closed_work_not_satisfied";

/// Where the stage and the verdict disagree in a way ADR 0107 names, one finding per intent.
fn verdict_drift(view: &IntentView, out: &mut Vec<IntentFinding>) {
    match (view.stage, view.verdict.state) {
        (IntentStage::Planned | IntentStage::Executing, IntentVerdictState::Satisfied) => {
            // planned or executing means every milestone resolved (one that does not holds the
            // stage at declared), so each carries a status and none is skipped here
            let open: Vec<String> = view
                .milestones
                .iter()
                .filter_map(|m| m.status.as_deref().map(|s| (m.id.as_str(), s)))
                .filter(|(_, s)| !matches!(*s, "DONE" | "CANCELLED" | "SUPERSEDED"))
                .map(|(id, s)| format!("{id} is {s}"))
                .collect();
            warn(
                out,
                EVIDENCE_AHEAD_OF_PLAN,
                &view.id,
                format!(
                    "the verdict is satisfied — every criterion has current evidence — and the \
                     stage is {} ({}): the evidence settles the intent before the plan closes it",
                    view.stage.as_str(),
                    open.join(", ")
                ),
            );
        }
        (IntentStage::Verifying, state) if state != IntentVerdictState::Satisfied => {
            let held = if view.verdict.reasons.is_empty() {
                "no criterion is declared, so no evidence can settle it".to_string()
            } else {
                let named: Vec<String> = view
                    .verdict
                    .reasons
                    .iter()
                    .map(|r| format!("`{}` ({} {})", r.criterion, r.evidence, r.state.as_str()))
                    .collect();
                format!("held back by {}", named.join(", "))
            };
            warn(
                out,
                CLOSED_WORK_NOT_SATISFIED,
                &view.id,
                format!(
                    "every milestone is DONE and the verdict is {}: {held}",
                    state.as_str()
                ),
            );
        }
        _ => {}
    }
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
        canonical(&mut refs);
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
/// use majordomus_cli::intent::{verdict, IntentStage, IntentView, Intents};
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
///     verdict: verdict(&[]),
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
            format!(
                "`{}` is met: its {} `{}` has current evidence",
                c.id, c.evidence, c.reference
            )
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
        matches!(
            e.event.as_str(),
            "session.started" | "provider.event.received"
        )
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
            // `current` only ever names a task `units` holds
            if let (Some(unit), false) = (owner.and_then(|t| units.get_mut(t)), issue.is_empty()) {
                push_once(&mut unit.moved_issues, &issue);
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
            u.outcome = if a.outcome.is_empty() {
                "active".into()
            } else {
                a.outcome.clone()
            };
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
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    for o in index.objects.iter().filter(|o| o.kind == "session") {
        let m = &o.metadata;
        let sid = m
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if sid.is_empty() || held.contains(sid) {
            continue;
        }
        let mut u = IntentWorkUnit::new(IntentWorkKind::SessionRecord, sid);
        u.title = m
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        u.outcome = m
            .get("outcome")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{verdict, IntentCriterion, IntentMilestone};
    use crate::intent_plan::CoveringIssue;
    use crate::plan::{PlanIssue, PlanProject, PlanVocabulary};
    use serde_json::json;

    fn issue(
        id: &str,
        milestone: &str,
        status: &str,
        scope: &[&str],
        serves: &[&str],
    ) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: milestone.into(),
            status: status.into(),
            wave: 0,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: id.into(),
            slug: id.into(),
            depends_on: vec![],
            blocked_by: vec![],
            dependents: vec![],
            scope: scope.iter().map(|s| s.to_string()).collect(),
            serves: serves.iter().map(|s| s.to_string()).collect(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
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
                issue: vec![],
                milestone: vec![],
            },
            milestones: vec![],
            issues,
            waves: vec![],
            edges: vec![],
            milestone_edges: vec![],
            findings: vec![],
        }
    }

    fn criterion(id: &str, state: IntentEvidenceState) -> IntentCriterion {
        IntentCriterion {
            id: id.into(),
            criterion: format!("{id} holds"),
            evidence: "test".into(),
            reference: format!("suite:{id}"),
            state,
            met: state == IntentEvidenceState::Current,
            reproduce: None,
        }
    }

    fn view(id: &str, stage: IntentStage, milestones: &[(&str, Option<&str>)]) -> IntentView {
        IntentView {
            id: id.into(),
            title: id.to_uppercase(),
            statement: String::new(),
            invariants: vec![],
            stage,
            milestones: milestones
                .iter()
                .map(|(m, s)| IntentMilestone {
                    id: m.to_string(),
                    resolved: s.is_some(),
                    status: s.map(str::to_string),
                })
                .collect(),
            satisfaction: vec![],
            met: 0,
            verdict: crate::intent::verdict(&[]),
            governance: vec![],
            non_goals: vec![],
            superseded_by: None,
            source: String::new(),
        }
    }

    fn intents(views: Vec<IntentView>) -> Intents {
        Intents {
            intents: views,
            findings: vec![],
        }
    }

    fn entry(ts: &str, event: &str, session: Option<&str>, payload: serde_json::Value) -> Entry {
        Entry {
            ts: ts.into(),
            event: event.into(),
            head: "h".into(),
            branch: "feature/I0001-x".into(),
            by: "majordomus/0".into(),
            session: session.map(str::to_string),
            payload: payload
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }

    fn codes(f: &[IntentFinding]) -> Vec<&str> {
        f.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn every_word_is_the_one_serde_writes() {
        for p in [
            IntentLinkProvenance::Declared,
            IntentLinkProvenance::Observed,
            IntentLinkProvenance::Derived,
            IntentLinkProvenance::Inferred,
        ] {
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
        }
        for k in [
            IntentWorkKind::Task,
            IntentWorkKind::PeerClaim,
            IntentWorkKind::SessionRecord,
        ] {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        for v in [
            IntentLinkVia::NamedIssue,
            IntentLinkVia::MovedIssue,
            IntentLinkVia::BranchIssue,
            IntentLinkVia::ScopeOverlap,
        ] {
            assert_eq!(serde_json::to_value(v).unwrap(), v.as_str());
        }
        // each fact grades to exactly one provenance, and the four are four
        let graded: BTreeSet<IntentLinkProvenance> = [
            IntentLinkVia::NamedIssue,
            IntentLinkVia::MovedIssue,
            IntentLinkVia::BranchIssue,
            IntentLinkVia::ScopeOverlap,
        ]
        .iter()
        .map(|v| v.provenance())
        .collect();
        assert_eq!(graded.len(), 4);
    }

    #[test]
    fn a_link_keeps_the_strongest_fact_per_issue_and_the_criteria_it_serves() {
        let p = plan(vec![
            issue("I0001", "m1", "ACTIVE", &["lib"], &["x#a", "y#b", "x#c"]),
            issue("I0002", "m2", "READY", &["docs"], &[]),
        ]);
        let i = intents(vec![
            view("x", IntentStage::Executing, &[("m1", Some("ACTIVE"))]),
            view("y", IntentStage::Planned, &[("m2", Some("READY"))]),
        ]);
        let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
        // the same issue offered by an event and by the branch: the event's link is kept
        u.branches = vec!["feature/I0001-work".into()];
        u.moved_issues = vec!["I0001".into(), "I9999".into()];
        u.named_issues = vec!["I0002".into()];
        let w = link(u, &i, &p);
        assert_eq!(w.unlinked, None);
        let got: Vec<(&str, &str, IntentLinkVia)> = w
            .links
            .iter()
            .map(|l| (l.intent.as_str(), l.issue.as_str(), l.via))
            .collect();
        // declared sorts before observed; I9999 is no issue of the plan and links nothing
        assert_eq!(
            got,
            [
                ("y", "I0002", IntentLinkVia::NamedIssue),
                ("x", "I0001", IntentLinkVia::MovedIssue),
            ]
        );
        assert_eq!(w.links[1].criteria, ["a", "c"]);
        assert!(w.links[0].criteria.is_empty());
        assert!(w.links.iter().all(|l| l.provenance == l.via.provenance()));

        // only the branch names it: derived
        let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-2");
        u.branches = vec!["feature/I0001-work".into()];
        let w = link(u, &i, &p);
        assert_eq!(w.links[0].provenance, IntentLinkProvenance::Derived);
    }

    #[test]
    fn a_scope_overlap_is_inferred_and_only_from_open_issues() {
        let p = plan(vec![
            issue("I0001", "m1", "ACTIVE", &["lib"], &[]),
            issue("I0002", "m1", "DONE", &["lib"], &[]),
            issue("I0003", "m1", "CANCELLED", &["lib"], &[]),
        ]);
        let i = intents(vec![view("x", IntentStage::Executing, &[("m1", None)])]);
        let mut u = IntentWorkUnit::new(IntentWorkKind::PeerClaim, "p1/c");
        u.scope = vec!["lib/a.rs".into()];
        let w = link(u, &i, &p);
        let issues: Vec<&str> = w.links.iter().map(|l| l.issue.as_str()).collect();
        assert_eq!(issues, ["I0001"]);
        assert_eq!(w.links[0].provenance, IntentLinkProvenance::Inferred);
    }

    #[test]
    fn unlinked_work_says_which_link_is_missing() {
        let p = plan(vec![issue(
            "I0001",
            "orphan-milestone",
            "ACTIVE",
            &["lib"],
            &[],
        )]);
        let i = intents(vec![view("x", IntentStage::Executing, &[("m1", None)])]);

        let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
        u.branches = vec!["feature/no-issue".into()];
        u.scope = vec!["docs".into()];
        let why = link(u, &i, &p).unlinked.unwrap();
        assert!(
            why.contains("its branch feature/no-issue names none"),
            "{why}"
        );
        assert!(why.contains("no open issue's scope covers docs"), "{why}");

        let why = link(IntentWorkUnit::new(IntentWorkKind::Task, "t-2"), &i, &p)
            .unlinked
            .unwrap();
        assert!(why.contains("it ran on no recorded branch"), "{why}");
        assert!(why.contains("it claims no scope"), "{why}");

        // the issue exists, but no intent names its milestone
        let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-3");
        u.named_issues = vec!["I0001".into()];
        let w = link(u, &i, &p);
        assert!(w.links.is_empty());
        assert_eq!(
            w.unlinked.unwrap(),
            "its issues belong to milestones no intent names: orphan-milestone (I0001)"
        );
    }

    #[test]
    fn drift_names_closed_work_reality_contradicts_or_never_proved() {
        let p = plan(vec![
            issue("I0001", "m1", "DONE", &[], &["x#a"]),
            issue("I0002", "m1", "DONE", &[], &["x#a"]),
            issue("I0003", "m1", "ACTIVE", &[], &["x#b"]),
        ]);
        let mut v = view("x", IntentStage::Verifying, &[("m1", Some("DONE"))]);
        v.satisfaction = vec![
            criterion("a", IntentEvidenceState::Stale),
            criterion("b", IntentEvidenceState::Failing),
            criterion("c", IntentEvidenceState::NotRun),
            criterion("d", IntentEvidenceState::Current),
        ];
        v.verdict = verdict(&v.satisfaction);
        let found = drift(&v, &p);
        assert_eq!(
            codes(&found),
            [
                "closed_work_contradicted",
                "closed_work_contradicted",
                "closed_work_unproven",
                "closed_work_not_satisfied"
            ]
        );
        assert!(found[0].message.contains("a source that has changed"));
        assert!(found[1].message.contains("did not pass"));
        assert!(found[2].message.contains("no recorded run"));
        assert!(found[3].message.contains(
            "the verdict is unsatisfied: held back by `a` (test stale), `b` (test failing), \
             `c` (test not_run)"
        ));
        assert!(found
            .iter()
            .all(|f| f.level == WARN && f.reproduce == REPRODUCE));

        // still executing: only a criterion every serving issue closed is drift
        v.stage = IntentStage::Executing;
        let found = drift(&v, &p);
        assert_eq!(codes(&found), ["criterion_closed_unmet"]);
        assert!(found[0].message.contains("(I0001, I0002)"));
        assert!(found[0].message.contains("criterion `a`"));

        // a planned intent is held the same way: closing every issue that serves a criterion
        // does not meet it
        v.stage = IntentStage::Planned;
        assert_eq!(codes(&drift(&v, &p)), ["criterion_closed_unmet"]);

        // a declared intent has no closure to drift from
        v.stage = IntentStage::Declared;
        assert!(drift(&v, &p).is_empty());
    }

    #[test]
    fn drift_names_where_the_stage_and_the_verdict_disagree() {
        let p = plan(vec![]);
        let all_current = vec![
            criterion("a", IntentEvidenceState::Current),
            criterion("b", IntentEvidenceState::Current),
        ];
        // the evidence settles the intent while the plan has not closed it
        for (stage, status) in [
            (IntentStage::Executing, "ACTIVE"),
            (IntentStage::Planned, "READY"),
        ] {
            let mut v = view("x", stage, &[("m1", Some("DONE")), ("m2", Some(status))]);
            v.satisfaction = all_current.clone();
            v.met = 2;
            v.verdict = verdict(&v.satisfaction);
            let found = drift(&v, &p);
            assert_eq!(codes(&found), [EVIDENCE_AHEAD_OF_PLAN], "{stage:?}");
            assert!(
                found[0]
                    .message
                    .contains(&format!("stage is {} (m2 is {status})", stage.as_str())),
                "{}",
                found[0].message
            );
            assert_eq!(found[0].level, WARN);
            assert_eq!(found[0].reproduce, REPRODUCE);
        }

        // closed work the ledger cannot settle: unknown, named with each criterion's own state
        let mut v = view("x", IntentStage::Verifying, &[("m1", Some("DONE"))]);
        let mut cmd = criterion("ships", IntentEvidenceState::NotDerivable);
        cmd.evidence = "command".into();
        v.satisfaction = vec![criterion("a", IntentEvidenceState::Current), cmd];
        v.met = 1;
        v.verdict = verdict(&v.satisfaction);
        let found = drift(&v, &p);
        assert_eq!(
            codes(&found),
            ["closed_work_unproven", CLOSED_WORK_NOT_SATISFIED]
        );
        assert!(found[1]
            .message
            .ends_with("the verdict is unknown: held back by `ships` (command not_derivable)"));

        // where the two agree there is nothing to report
        for stage in [
            IntentStage::Satisfied,
            IntentStage::Declared,
            IntentStage::Cancelled,
            IntentStage::Superseded,
        ] {
            let mut v = view("x", stage, &[("m1", Some("DONE"))]);
            v.satisfaction = all_current.clone();
            v.met = 2;
            v.verdict = verdict(&v.satisfaction);
            assert!(drift(&v, &p).is_empty(), "{stage:?}");
        }
        let mut v = view("x", IntentStage::Executing, &[("m1", Some("ACTIVE"))]);
        v.satisfaction = vec![criterion("a", IntentEvidenceState::NotRun)];
        v.verdict = verdict(&v.satisfaction);
        assert!(
            drift(&v, &p).is_empty(),
            "an unsatisfied verdict on open work is no drift"
        );
    }

    #[test]
    fn every_evidence_state_and_coverage_strength_has_its_own_words() {
        let states = [
            IntentEvidenceState::Current,
            IntentEvidenceState::Stale,
            IntentEvidenceState::Failing,
            IntentEvidenceState::NotRun,
            IntentEvidenceState::NotDerivable,
            IntentEvidenceState::Unresolved,
        ];
        let words: BTreeSet<&str> = states.iter().map(|s| state_words(*s)).collect();
        assert_eq!(words.len(), states.len());
        assert!(state_words(IntentEvidenceState::NotDerivable).contains("never met by itself"));
        assert!(state_words(IntentEvidenceState::Unresolved).contains("names nothing"));
        let strengths = [
            CoverageStrength::Uncovered,
            CoverageStrength::Observed,
            CoverageStrength::Weak,
            CoverageStrength::Covered,
        ];
        let words: BTreeSet<&str> = strengths.iter().map(|s| strength_words(*s)).collect();
        assert_eq!(words.len(), strengths.len());
    }

    #[test]
    fn realize_joins_work_to_intents_and_names_live_orphans_only() {
        let p = plan(vec![issue("I0001", "m1", "ACTIVE", &["lib"], &["x#a"])]);
        let mut v = view("x", IntentStage::Executing, &[("m1", Some("ACTIVE"))]);
        let mut a = criterion("a", IntentEvidenceState::NotRun);
        a.reproduce = Some("test/run.sh a".into());
        v.satisfaction = vec![a, criterion("b", IntentEvidenceState::Current)];
        v.met = 1;
        v.verdict = verdict(&v.satisfaction);
        let i = intents(vec![v]);

        let mut t = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
        t.named_issues = vec!["I0001".into()];
        t.handovers = vec!["h1.md".into(), "h2.md".into()];
        for p in ["claude-code", "codex"] {
            t.episodes.push(IntentEpisode {
                session: format!("s-{p}"),
                provider: p.into(),
                provider_session: String::new(),
            });
        }
        let mut claim = IntentWorkUnit::new(IntentWorkKind::PeerClaim, "p1/c");
        claim.scope = vec!["lib".into()];
        claim.episodes.push(IntentEpisode {
            session: String::new(),
            provider: "codex".into(),
            provider_session: String::new(),
        });
        let mut live_task = IntentWorkUnit::new(IntentWorkKind::Task, "t-orphan");
        live_task.outcome = "active".into();
        let mut finished_task = IntentWorkUnit::new(IntentWorkKind::Task, "t-old");
        finished_task.outcome = "completed".into();
        let mut live_claim = IntentWorkUnit::new(IntentWorkKind::PeerClaim, "p2/c");
        live_claim.outcome = "attached".into();
        let mut gone_claim = IntentWorkUnit::new(IntentWorkKind::PeerClaim, "p3/c");
        gone_claim.outcome = "departed".into();
        let mut record = IntentWorkUnit::new(IntentWorkKind::SessionRecord, "s-r");
        record.outcome = "active".into();

        let r = realize(
            &i,
            &p,
            vec![
                t,
                claim,
                live_task,
                finished_task,
                live_claim,
                gone_claim,
                record,
            ],
        );
        assert_eq!(r.orphans, 5);
        assert_eq!(
            r.findings
                .iter()
                .map(|f| f.subject.as_str())
                .collect::<Vec<_>>(),
            ["t-orphan", "p2/c"],
            "only live work is named; a finished task, a departed claim and a closed session \
             record are history"
        );
        let x = &r.intents[0];
        assert_eq!((x.criteria, x.met), (2, 1));
        assert_eq!(x.unmet.len(), 1);
        assert_eq!(x.unmet[0].issues, ["I0001"]);
        assert_eq!(x.unmet[0].reproduce.as_deref(), Some("test/run.sh a"));
        let work: Vec<(&str, IntentLinkProvenance, usize)> = x
            .work
            .iter()
            .map(|w| (w.id.as_str(), w.provenance, w.handovers))
            .collect();
        assert_eq!(
            work,
            [
                ("t-1", IntentLinkProvenance::Declared, 2),
                ("p1/c", IntentLinkProvenance::Inferred, 0),
            ]
        );
        assert_eq!(x.providers, ["claude-code", "codex"]);
    }

    #[test]
    fn explain_gives_one_sentence_per_fact_for_every_stage() {
        let r = IntentRealization {
            intents: vec![],
            work: vec![],
            orphans: 0,
            findings: vec![],
        };
        let none = IntentCoverage {
            criteria: vec![],
            issues: vec![],
            findings: vec![],
        };
        let cases = [
            (
                IntentStage::Declared,
                vec![],
                "declared: it names no milestone",
            ),
            (
                IntentStage::Declared,
                vec![("m1", None)],
                "declared: m1 does not resolve",
            ),
            (
                IntentStage::Planned,
                vec![("m1", Some("READY"))],
                "planned: every milestone resolves and none has started (m1 is READY)",
            ),
            (
                IntentStage::Executing,
                vec![("m1", Some("ACTIVE"))],
                "executing: m1 is ACTIVE",
            ),
            (
                IntentStage::Verifying,
                vec![],
                "verifying: every milestone is DONE and 0 of 0",
            ),
            (
                IntentStage::Satisfied,
                vec![],
                "satisfied: every milestone is DONE and all 0",
            ),
            (
                IntentStage::Cancelled,
                vec![],
                "cancelled: the record says so",
            ),
            (IntentStage::Superseded, vec![], "superseded by y"),
        ];
        for (stage, milestones, first) in cases {
            let mut v = view("x", stage, &milestones);
            v.superseded_by = Some("y".into());
            let e = explain(&v, &none, &r);
            assert!(
                e.because[0].starts_with(first),
                "{stage:?}: {}",
                e.because[0]
            );
            // an intent the realization never saw is realised by nothing, and says so
            assert_eq!(
                e.because.last().unwrap(),
                "no recorded task, session or live claim realises it"
            );
            assert_eq!(e.realization.intent, "x");
        }
    }

    #[test]
    fn explain_reads_each_criterion_with_its_coverage_and_the_work() {
        let p = plan(vec![
            issue("I0001", "m1", "DONE", &[], &["x#a"]),
            issue("I0002", "m1", "DONE", &[], &["x#b"]),
        ]);
        let mut v = view("x", IntentStage::Verifying, &[("m1", Some("DONE"))]);
        let mut b = criterion("b", IntentEvidenceState::Failing);
        b.reproduce = Some("test/run.sh b".into());
        let mut c = criterion("c", IntentEvidenceState::NotRun);
        c.reproduce = None;
        let mut met = criterion("a", IntentEvidenceState::Current);
        met.reproduce = Some("never printed for a met criterion".into());
        v.satisfaction = vec![met, b, c];
        v.met = 1;
        v.verdict = verdict(&v.satisfaction);
        let mut t = IntentWorkUnit::new(IntentWorkKind::Task, "t-1");
        t.moved_issues = vec!["I0001".into()];
        t.handovers = vec!["h.md".into()];
        t.episodes.push(IntentEpisode {
            session: "s-1".into(),
            provider: "codex".into(),
            provider_session: String::new(),
        });
        let mut quiet = IntentWorkUnit::new(IntentWorkKind::Task, "t-2");
        quiet.moved_issues = vec!["I0002".into()];
        let unrelated = IntentWorkUnit::new(IntentWorkKind::Task, "t-3");
        let r = realize(&intents(vec![v.clone()]), &p, vec![t, quiet, unrelated]);
        let coverage = IntentCoverage {
            criteria: vec![
                CriterionCoverage {
                    intent: "x".into(),
                    criterion: "a".into(),
                    strength: CoverageStrength::Covered,
                    issues: vec![CoveringIssue {
                        id: "I0001".into(),
                        milestone: "m1".into(),
                        status: "DONE".into(),
                        evidence_need: 1,
                    }],
                    milestones: vec!["m1".into()],
                },
                CriterionCoverage {
                    intent: "x".into(),
                    criterion: "b".into(),
                    strength: CoverageStrength::Uncovered,
                    issues: vec![],
                    milestones: vec![],
                },
                CriterionCoverage {
                    intent: "other".into(),
                    criterion: "c".into(),
                    strength: CoverageStrength::Weak,
                    issues: vec![],
                    milestones: vec![],
                },
            ],
            issues: vec![],
            findings: vec![],
        };
        let e = explain(&v, &coverage, &r);
        assert_eq!(
            e.because[1],
            "`a` is met: its test `suite:a` has current evidence; an issue serving it owes \
             evidence before it is DONE (I0001)"
        );
        assert_eq!(
            e.because[2],
            "`b` is not met: its test `suite:b` has a latest recorded run that did not pass; no \
             live issue serves it; reproduce: test/run.sh b"
        );
        // another intent's coverage of a criterion with the same id is not this one's
        assert_eq!(
            e.because[3],
            "`c` is not met: its test `suite:c` has no recorded run"
        );
        assert_eq!(
            e.because[4],
            "2 unit(s) of work realise it across 1 handover(s), run by codex"
        );
        assert!(e.because[5].starts_with("closed_work_contradicted: "));
        assert!(e.because[6].starts_with("closed_work_unproven: "));
        assert!(e.because[7].starts_with("closed_work_not_satisfied: "));
        assert_eq!(e.coverage.len(), 2);
        let ids: Vec<&str> = e.work.iter().map(|w| w.work.id.as_str()).collect();
        assert_eq!(ids, ["t-1", "t-2"]);

        // work without a provider names none
        let mut v2 = v.clone();
        v2.satisfaction.clear();
        v2.verdict = verdict(&v2.satisfaction);
        let r2 = realize(
            &intents(vec![v2.clone()]),
            &p,
            vec![{
                let mut u = IntentWorkUnit::new(IntentWorkKind::Task, "t-9");
                u.moved_issues = vec!["I0001".into()];
                u
            }],
        );
        let e2 = explain(&v2, &coverage, &r2);
        assert_eq!(
            e2.because[1],
            "1 unit(s) of work realise it across 0 handover(s)"
        );
        // closed work over an intent declaring no criterion: nothing can settle it
        assert_eq!(
            e2.because.last().unwrap(),
            "closed_work_not_satisfied: every milestone is DONE and the verdict is unknown: no \
             criterion is declared, so no evidence can settle it"
        );
    }

    #[test]
    fn a_ledger_attributes_each_task_its_episodes_providers_and_outcome() {
        let mut opened = BTreeMap::new();
        opened.insert("s-b".to_string(), ("gemini".to_string(), "g-1".to_string()));
        let entries = vec![
            entry(
                "1",
                "session.started",
                Some("s-a"),
                json!({ "provider": "codex" }),
            ),
            // a later provider event does not overwrite what the start line named
            entry(
                "2",
                "provider.event.received",
                Some("s-a"),
                json!({ "provider": "claude-code", "provider_session": "x" }),
            ),
            entry(
                "3",
                "session.started",
                None,
                json!({ "provider": "orphan" }),
            ),
            entry("4", "session.started", Some("s-c"), json!({})),
            entry(
                "5",
                "task.started",
                Some("s-a"),
                json!({ "task_id": "t-1", "scope": "lib lib docs" }),
            ),
            // a plan transition before any task ran in the episode belongs to none
            entry("6", "plan_start", Some("s-b"), json!({ "issue": "I0009" })),
            entry("7", "plan_start", None, json!({ "issue": "I0008" })),
            entry(
                "8",
                "task.checkpoint",
                Some("s-b"),
                json!({ "task_id": "t-1" }),
            ),
            entry("9", "plan_done", Some("s-b"), json!({ "issue": "I0001" })),
            entry("10", "plan_done", Some("s-b"), json!({ "issue": "" })),
            entry(
                "11",
                "task.finished",
                Some("s-b"),
                json!({ "task_id": "t-1" }),
            ),
            entry("12", "task.started", None, json!({ "task_id": "t-2" })),
            entry(
                "13",
                "task.finished",
                None,
                json!({ "task_id": "t-2", "outcome": "abandoned" }),
            ),
            entry("14", "task.started", Some("s-c"), json!({ "task_id": "" })),
            entry(
                "15",
                "task.unknown",
                Some("s-c"),
                json!({ "task_id": "t-3" }),
            ),
        ];
        let tasks = tasks_from_ledger(&entries, &opened);
        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["t-1", "t-2", "t-3"]);
        let t1 = &tasks[0];
        assert_eq!(t1.outcome, "finished");
        assert_eq!(t1.scope, ["lib", "docs"]);
        assert_eq!(t1.moved_issues, ["I0001"]);
        assert_eq!(t1.branches, ["feature/I0001-x"]);
        assert_eq!(t1.providers(), ["codex", "gemini"]);
        assert_eq!(t1.episodes[1].provider_session, "g-1");
        assert_eq!(tasks[1].outcome, "abandoned");
        assert!(tasks[1].episodes.is_empty());
        // an event of the task family that moves no outcome still records the episode
        assert_eq!(tasks[2].outcome, "");
        assert_eq!(tasks[2].episodes[0].session, "s-c");
        assert_eq!(tasks[2].episodes[0].provider, "");
        // an episode without a provider names none, and a provider is named once
        assert!(tasks[2].providers().is_empty());
        let mut twice = tasks[0].clone();
        twice.episodes.push(tasks[0].episodes[0].clone());
        twice.episodes.push(tasks[2].episodes[0].clone());
        assert_eq!(twice.providers(), ["codex", "gemini"]);
    }

    #[test]
    fn issue_tokens_are_whole_words_each_once() {
        let ids = ["I0001".to_string(), "I0002".to_string()];
        assert_eq!(
            issue_tokens("I0002 then I0001, again I0002; I0001x _I0001 I0001-y", &ids),
            ["I0002", "I0001"]
        );
        assert!(issue_tokens("", &ids).is_empty());
    }

    #[test]
    fn gather_names_providers_from_open_episodes_and_branches_from_peer_checkouts() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root();
        let state = root.join(STATE_DIR);
        let open = state.join("sessions-open");
        std::fs::create_dir_all(open.join("not-a-document")).unwrap();
        // the one open episode that names itself gives its task a provider
        std::fs::write(
            open.join("s-x.yaml"),
            "session_id: s-x\nprovider: gemini\nprovider_session: g-1\n",
        )
        .unwrap();
        // a document without a session id, and an entry that is no document, name nobody
        std::fs::write(open.join("anonymous.yaml"), "owner: nobody\n").unwrap();
        std::fs::write(
            state.join("ledger.jsonl"),
            concat!(
                r#"{"ts":"1","event":"task.started","head":"abc","branch":"master","#,
                r#""by":"majordomus/0.7.0","session":"s-x","task_id":"t-1","scope":"lib"}"#,
                "\n",
                r#"{"ts":"2","event":"task.started","head":"abc","branch":"master","#,
                r#""by":"majordomus/0.7.0","session":"s-anon","task_id":"t-2"}"#,
                "\n",
            ),
        )
        .unwrap();
        let peer = |id: &str, checkout: serde_json::Value| -> Peer {
            let mut p = json!({
                "id": id,
                "client": { "name": "codex", "version": "1" },
                "transport": "http",
                "connected_at": "2026-10-02T00:00:00Z",
                "last_seen_seconds_ago": 0,
                "attached": id == "p1",
                "claims": [{ "name": "work", "intent": "finish it", "scope": ["lib"],
                             "at": "2026-10-02T00:00:00Z" }],
            });
            if !checkout.is_null() {
                p["checkout"] = checkout;
            }
            serde_json::from_value(p).unwrap()
        };
        let peers = [
            peer(
                "p1",
                json!({ "id": "c1", "worktree": "/r-wt/feature/x", "branch": "feature/x",
                        "this_checkout": false }),
            ),
            // a checkout whose branch could not be read, and a peer read without a checkout,
            // name no branch
            peer(
                "p2",
                json!({ "id": "c2", "worktree": "/r", "this_checkout": true }),
            ),
            peer("p3", serde_json::Value::Null),
        ];

        let (units, skipped) = gather(root, &repo.index().unwrap(), &peers);
        assert_eq!(skipped, 0);
        let unit = |id: &str| {
            units
                .iter()
                .find(|u| u.id == id)
                .unwrap_or_else(|| panic!("no unit {id}: {units:?}"))
        };
        assert_eq!(unit("t-1").providers(), ["gemini"]);
        assert_eq!(unit("t-1").episodes[0].provider_session, "g-1");
        assert!(unit("t-2").providers().is_empty());

        let claim = unit("p1/work");
        assert_eq!(claim.kind, IntentWorkKind::PeerClaim);
        assert_eq!(claim.outcome, "attached");
        assert_eq!(claim.branches, ["feature/x"]);
        assert_eq!(claim.scope, ["lib"]);
        assert_eq!(claim.providers(), ["codex"]);
        assert_eq!(unit("p2/work").outcome, "departed");
        assert!(unit("p2/work").branches.is_empty());
        assert!(unit("p3/work").branches.is_empty());
    }
}
