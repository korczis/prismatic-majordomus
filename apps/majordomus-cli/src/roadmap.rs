//! The roadmap, inferred from what shipped and from what the plan derives (ADR 0125).
//!
//! A roadmap that names versions ahead of time is a second authority: the versions were
//! typed into milestone records, `0.5` was typed twice, and the release that actually
//! shipped was `0.18.0`. Here a version is a fact of the past — a release record that was
//! published — and the future is described by what the graph allows, not by a number.
//!
//! Four horizons, each a function of the derived status alone:
//!
//! | horizon | the milestones in it                                                   |
//! |---------|------------------------------------------------------------------------|
//! | `now`   | `ACTIVE` or `VERIFY`: work has started under it                        |
//! | `next`  | `PLANNED`: nothing it requires is unfinished, so it can begin          |
//! | `later` | `BLOCKED`: a milestone it requires is not `DONE`                       |
//! | `done`  | `DONE`                                                                 |
//!
//! What a release shipped is attributed from two committed facts, never from a date typed
//! for the purpose: the `completed_at` an issue record carries, placed in the window between
//! two published releases, and the issues the commits of each release name, which the
//! changelog already resolves. Neither moves when an unrelated commit lands.
//!
//! What is inferred is said as inference. A pace is the number of a milestone's issues
//! completed over the last [`PACE_WINDOW`] releases, and a forecast is that pace applied to
//! what remains — absent when nothing was completed, because no pace was measured. A signal
//! is a disagreement between two committed facts: commits naming an issue were released
//! while its record still says it has not started. Nothing here writes a status.
//!
//! The lifecycle is one call: the derived plan, the changelog's sections and the intents go
//! in, and the horizons, the shipped releases and the signals come out.
//!
//! ```
//! use majordomus_cli::plan::Plan;
//! use majordomus_cli::roadmap::{infer, Horizon};
//! let plan: Plan = serde_json::from_value(serde_json::json!({
//!   "project": {"name": "p", "repository": "", "default_branch": "main", "active_milestone": ""},
//!   "statuses": {"issue": [], "milestone": []},
//!   "milestones": [{"id": "m", "status": "BLOCKED", "order": 1, "priority": "p1",
//!     "title": "M", "slug": "m", "version": "", "rank": 1, "depends_on": ["n"],
//!     "blocked_by": ["n"], "dependents": [], "claims": [],
//!     "counts": {"total": 0, "required": 0, "by_status": {}}, "issues": [], "outcome": "o"}],
//!   "issues": [], "waves": [], "edges": [], "milestone_edges": [], "findings": []
//! })).unwrap();
//! let roadmap = infer(&plan, &[], &[], "0.1.0");
//! let later = &roadmap.horizons[2];
//! assert_eq!(later.horizon, Horizon::Later);
//! assert_eq!(later.milestones[0].blocked_by, ["n"]);
//! // no release was published, so nothing shipped and nothing is forecast
//! assert!(roadmap.releases.is_empty());
//! assert_eq!(later.milestones[0].pace.releases, None);
//! ```

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::plan::{Plan, PlanCounts, PlanMilestone};
use crate::release::model::{ChangeKind, ReleaseSection};

/// The schema id of the inferred roadmap.
pub const SCHEMA: &str = "majordomus/roadmap/v1";

/// How many published releases a pace is measured over.
pub const PACE_WINDOW: usize = 5;

/// Where a milestone stands, derived from its status alone.
///
/// ```
/// use majordomus_cli::roadmap::Horizon;
/// // serialised as the word a page anchors on
/// assert_eq!(serde_json::to_value(Horizon::Now).unwrap(), "now");
/// assert!(Horizon::Now < Horizon::Done);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Horizon {
    /// Work has started under it.
    Now,
    /// It can begin: nothing it requires is unfinished.
    Next,
    /// A milestone it requires is not done.
    Later,
    /// Its outcome is real.
    Done,
}

impl Horizon {
    /// The horizon of a derived milestone status, or none for one that left the roadmap
    /// (`CANCELLED`, `SUPERSEDED`).
    ///
    /// ```
    /// use majordomus_cli::roadmap::Horizon;
    /// assert_eq!(Horizon::of("VERIFY"), Some(Horizon::Now));
    /// assert_eq!(Horizon::of("PLANNED"), Some(Horizon::Next));
    /// assert_eq!(Horizon::of("BLOCKED"), Some(Horizon::Later));
    /// assert_eq!(Horizon::of("CANCELLED"), None);
    /// ```
    pub fn of(status: &str) -> Option<Horizon> {
        match status {
            "ACTIVE" | "VERIFY" => Some(Horizon::Now),
            "PLANNED" => Some(Horizon::Next),
            "BLOCKED" => Some(Horizon::Later),
            "DONE" => Some(Horizon::Done),
            _ => None,
        }
    }

    /// The heading a surface shows it under, and the sentence that says what puts a
    /// milestone there.
    ///
    /// ```
    /// use majordomus_cli::roadmap::Horizon;
    /// let (heading, meaning) = Horizon::Later.describe();
    /// assert_eq!(heading, "Later");
    /// assert!(meaning.contains("requires"));
    /// ```
    pub fn describe(self) -> (&'static str, &'static str) {
        match self {
            Horizon::Now => (
                "Now",
                "Work has started: an issue is active, in verification or done, and the milestone is not yet real.",
            ),
            Horizon::Next => (
                "Next",
                "Nothing it requires is unfinished, so it can begin; no issue under it has started.",
            ),
            Horizon::Later => (
                "Later",
                "It requires a milestone that is not done, so finishing its own issues would not release it.",
            ),
            Horizon::Done => (
                "Done",
                "Every required issue is done and the milestone's own evidence is recorded.",
            ),
        }
    }
}

/// What kind of disagreement a signal reports.
///
/// ```
/// use majordomus_cli::roadmap::SignalCode;
/// assert_eq!(
///     serde_json::to_value(SignalCode::ShippedUnrecorded).unwrap(),
///     "shipped_unrecorded"
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignalCode {
    /// A feature, fix or performance change released in the window names an issue whose record
    /// says it has not started.
    ShippedUnrecorded,
    /// Every required issue is done, and the milestone's own evidence is not recorded.
    AwaitsEvidence,
    /// The milestone has no issue: its outcome is specified and its work is not.
    NoIssues,
    /// Work has started (`ACTIVE`), and no issue was completed or named by a release in the
    /// window.
    Idle,
}

/// One inferred disagreement between two committed facts about a milestone.
///
/// ```
/// use majordomus_cli::roadmap::{Signal, SignalCode};
/// let s = Signal { code: SignalCode::NoIssues, message: "no issue".into(), subjects: vec!["m".into()] };
/// assert_eq!(serde_json::to_value(&s).unwrap()["code"], "no_issues");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapSignal")]
pub struct Signal {
    /// What kind of disagreement.
    pub code: SignalCode,
    /// One sentence a reader can act on.
    pub message: String,
    /// The records it is about: issue ids, or the milestone itself.
    pub subjects: Vec<String>,
}

/// A milestone's measured pace, and the forecast it supports.
///
/// ```
/// use majordomus_cli::roadmap::Pace;
/// // nothing completed in the window: no pace, so the forecast is absent rather than zero
/// let p = Pace { completed: 0, window: 5, remaining: 3, releases: None };
/// assert!(serde_json::to_value(&p).unwrap().get("releases").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapPace")]
pub struct Pace {
    /// How many of its issues were completed inside the window.
    pub completed: u32,
    /// How many published releases the window spans; fewer than [`PACE_WINDOW`] when the
    /// repository has published fewer.
    pub window: u32,
    /// Required issues not yet done.
    pub remaining: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Releases until nothing remains, at the measured pace. Absent when nothing was
    /// completed in the window: no pace was measured, so none is forecast.
    pub releases: Option<u32>,
}

/// An intent a milestone realises, as the intent derivation states it.
///
/// ```
/// use majordomus_cli::roadmap::IntentRef;
/// let i = IntentRef { id: "x".into(), title: "X".into(), stage: "planned".into(),
///     met: 0, criteria: 2, milestones: vec!["m".into()] };
/// assert_eq!(serde_json::to_value(&i).unwrap()["criteria"], 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapIntent")]
pub struct IntentRef {
    /// The intent's id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its derived stage.
    pub stage: String,
    /// How many of its satisfaction criteria are met by recorded evidence.
    pub met: u32,
    /// How many criteria it states.
    pub criteria: u32,
    /// The milestones that realise it.
    pub milestones: Vec<String>,
}

/// One milestone on the roadmap, with everything inferred about it.
///
/// ```
/// # use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #   "project": {"name": "p", "repository": "", "default_branch": "main", "active_milestone": ""},
/// #   "statuses": {"issue": [], "milestone": []},
/// #   "milestones": [{"id": "m", "status": "DONE", "order": 1, "priority": "p1",
/// #     "title": "M", "slug": "m", "version": "", "rank": 0, "depends_on": [],
/// #     "blocked_by": [], "dependents": [], "claims": [],
/// #     "counts": {"total": 0, "required": 0, "by_status": {}}, "issues": [], "outcome": "o"}],
/// #   "issues": [], "waves": [], "edges": [], "milestone_edges": [], "findings": []
/// # })).unwrap();
/// use majordomus_cli::roadmap::{infer, Horizon, Milestone};
/// let roadmap = infer(&plan, &[], &[], "0.1.0");
/// let m: &Milestone = &roadmap.horizons[3].milestones[0];
/// assert_eq!((m.horizon, m.id.as_str()), (Horizon::Done, "m"));
/// // a done milestone with no issue is not reported as unplanned
/// assert!(m.signals.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapMilestone")]
pub struct Milestone {
    /// The identity.
    pub id: String,
    /// One line naming the outcome.
    pub title: String,
    /// What is true once it is real.
    pub outcome: String,
    /// The derived status.
    pub status: String,
    /// The horizon that status puts it in.
    pub horizon: Horizon,
    /// `p0` … `p3`.
    pub priority: String,
    /// Its layer in the milestone graph.
    pub rank: u32,
    /// The milestones it requires.
    pub depends_on: Vec<String>,
    /// Those of them that are not done.
    pub blocked_by: Vec<String>,
    /// How many milestones wait on it, directly or through another.
    pub unblocks: u32,
    /// The claims its outcome would make true.
    pub claims: Vec<String>,
    /// Its issues, counted by derived status.
    pub counts: PlanCounts,
    /// The intents it realises.
    pub intents: Vec<String>,
    /// The releases that completed one of its issues or carried a commit naming one, newest
    /// first.
    pub shipped_in: Vec<String>,
    /// Its measured pace.
    pub pace: Pace,
    /// What the committed facts disagree about.
    pub signals: Vec<Signal>,
}

/// One milestone's share of one release.
///
/// ```
/// use majordomus_cli::roadmap::ShippedWork;
/// let w = ShippedWork { milestone: "m".into(), title: "M".into(),
///     completed: vec!["I1".into()], named: vec![] };
/// assert_eq!(serde_json::to_value(&w).unwrap()["completed"][0], "I1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapShippedWork")]
pub struct ShippedWork {
    /// The milestone.
    pub milestone: String,
    /// Its title.
    pub title: String,
    /// Issues whose `completed_at` falls inside the release's window.
    pub completed: Vec<String>,
    /// Issues the release's commits name that were not completed inside its window.
    pub named: Vec<String>,
}

/// One release, as the roadmap reads it.
///
/// ```
/// use majordomus_cli::roadmap::Release;
/// let r = Release { version: "unreleased".into(), date: None, unreleased: true, work: vec![] };
/// // the work since the last release has no date, and says so by leaving it out
/// assert!(serde_json::to_value(&r).unwrap().get("date").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapRelease")]
pub struct Release {
    /// The version, or `unreleased` for the work since the last one.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// When it was published.
    pub date: Option<String>,
    /// Whether it is the work not yet released.
    pub unreleased: bool,
    /// What each milestone got from it, in milestone order.
    pub work: Vec<ShippedWork>,
}

/// One horizon, with its milestones in the order they are taken.
///
/// ```
/// use majordomus_cli::roadmap::{Horizon, HorizonView};
/// let (heading, meaning) = Horizon::Next.describe();
/// let v = HorizonView { horizon: Horizon::Next, heading: heading.into(),
///     meaning: meaning.into(), milestones: vec![] };
/// assert_eq!(serde_json::to_value(&v).unwrap()["horizon"], "next");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RoadmapHorizon")]
pub struct HorizonView {
    /// Which horizon.
    pub horizon: Horizon,
    /// Its heading.
    pub heading: String,
    /// What puts a milestone in it.
    pub meaning: String,
    /// Its milestones: rank first, then priority, then how much each one unblocks.
    pub milestones: Vec<Milestone>,
}

/// The roadmap, inferred: every live milestone in the horizon its status gives it, the
/// releases with what each one shipped, the intents, and the basis of every inference.
///
/// ```
/// # use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #   "project": {"name": "p", "repository": "", "default_branch": "main", "active_milestone": ""},
/// #   "statuses": {"issue": [], "milestone": []}, "milestones": [],
/// #   "issues": [], "waves": [], "edges": [], "milestone_edges": [], "findings": []
/// # })).unwrap();
/// use majordomus_cli::roadmap::{infer, Roadmap, SCHEMA};
/// let roadmap: Roadmap = infer(&plan, &[], &[], "0.1.0");
/// assert_eq!(roadmap.schema, SCHEMA);
/// // an empty horizon is kept, so a reader can say that nothing is in it
/// assert_eq!(roadmap.horizons.len(), 4);
/// assert!(!roadmap.basis.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Roadmap")]
pub struct Roadmap {
    /// [`SCHEMA`].
    pub schema: String,
    /// The version the tree declares.
    pub current: String,
    /// The horizons, `now` first and `done` last; an empty one is kept, so a reader can say
    /// that nothing is in it.
    pub horizons: Vec<HorizonView>,
    /// The releases, newest first, with the work each one shipped.
    pub releases: Vec<Release>,
    /// The intents the milestones realise.
    pub intents: Vec<IntentRef>,
    /// How many signals the milestones carry in all.
    pub signals: u32,
    /// How each inference above was made, one sentence each.
    pub basis: Vec<String>,
}

/// Infer the roadmap from the derived plan, the changelog's release sections (newest
/// first, an `unreleased` section allowed at the head) and the intents.
///
/// Pure: the caller reads the files; this decides.
///
/// ```
/// use majordomus_cli::plan::Plan;
/// use majordomus_cli::roadmap::{infer, Horizon};
/// let plan: Plan = serde_json::from_value(serde_json::json!({
///   "project": {"name": "p", "repository": "", "default_branch": "main", "active_milestone": ""},
///   "statuses": {"issue": [], "milestone": []},
///   "milestones": [{"id": "m", "status": "PLANNED", "order": 1, "priority": "p1",
///     "title": "M", "slug": "m", "version": "", "rank": 0, "depends_on": [],
///     "blocked_by": [], "dependents": [], "claims": [],
///     "counts": {"total": 0, "required": 0, "by_status": {}}, "issues": [], "outcome": "o"}],
///   "issues": [], "waves": [], "edges": [], "milestone_edges": [], "findings": []
/// })).unwrap();
/// let roadmap = infer(&plan, &[], &[], "0.1.0");
/// let next = roadmap.horizons.iter().find(|h| h.horizon == Horizon::Next).unwrap();
/// assert_eq!(next.milestones[0].id, "m");
/// // a milestone with no issue says so rather than reading as ready work
/// assert_eq!(roadmap.signals, 1);
/// ```
pub fn infer(
    plan: &Plan,
    sections: &[ReleaseSection],
    intents: &[IntentRef],
    current: &str,
) -> Roadmap {
    let windows = windows(sections);
    let releases = releases(plan, &windows);
    let named_released = shipped_recently(&windows);
    let published: Vec<&Window> = windows.iter().filter(|w| !w.unreleased).collect();
    let pace_floor = published
        .get(PACE_WINDOW)
        .and_then(|w| w.date.clone())
        .unwrap_or_default();
    let pace_ceiling = published
        .first()
        .and_then(|w| w.date.clone())
        .unwrap_or_default();
    let window_len = published.len().min(PACE_WINDOW) as u32;

    let mut by_horizon: BTreeMap<Horizon, Vec<Milestone>> = BTreeMap::new();
    for m in &plan.milestones {
        let Some(horizon) = Horizon::of(&m.status) else {
            continue;
        };
        let milestone = view(
            plan,
            m,
            horizon,
            &releases,
            &named_released,
            intents,
            (&pace_floor, &pace_ceiling, window_len),
        );
        by_horizon.entry(horizon).or_default().push(milestone);
    }
    let horizons: Vec<HorizonView> = [Horizon::Now, Horizon::Next, Horizon::Later, Horizon::Done]
        .into_iter()
        .map(|h| {
            let mut milestones = by_horizon.remove(&h).unwrap_or_default();
            milestones.sort_by(|a, b| {
                let key = |m: &Milestone| {
                    (
                        m.rank,
                        m.priority.clone(),
                        std::cmp::Reverse(m.unblocks),
                        order_of(plan, &m.id),
                        m.id.clone(),
                    )
                };
                key(a).cmp(&key(b))
            });
            let (heading, meaning) = h.describe();
            HorizonView {
                horizon: h,
                heading: heading.to_string(),
                meaning: meaning.to_string(),
                milestones,
            }
        })
        .collect();
    let signals = horizons
        .iter()
        .flat_map(|h| &h.milestones)
        .map(|m| m.signals.len() as u32)
        .sum();
    Roadmap {
        schema: SCHEMA.to_string(),
        current: current.to_string(),
        horizons,
        releases,
        intents: intents.to_vec(),
        signals,
        basis: basis(),
    }
}

fn basis() -> Vec<String> {
    vec![
        "A horizon is a function of the milestone's derived status alone: now is ACTIVE or VERIFY, next is PLANNED, later is BLOCKED, done is DONE.".to_string(),
        "Inside a horizon the order is the graph's rank, then priority, then how many milestones each one unblocks; the record's order field only breaks what is left.".to_string(),
        "A version is a published release record. No milestone is given a version before it ships.".to_string(),
        "An issue was completed in a release when its completed_at falls after the previous release and no later than that one; it was named by a release when a commit in that release's range names it.".to_string(),
        format!("A pace is the issues of a milestone completed over the last {PACE_WINDOW} published releases; a forecast divides what remains by it, and none is given when nothing was completed."),
        "A signal reports two committed facts that disagree. It never changes a status: a record is transitioned by the worker or accepted by a person.".to_string(),
    ]
}

/// One release's window: the work completed after the previous release and no later than
/// this one, and the issues its commits name.
struct Window {
    version: String,
    date: Option<String>,
    after: Option<String>,
    unreleased: bool,
    named: BTreeSet<String>,
    /// Issues named by a change that alters behaviour (`feat`, `fix`, `perf`): what the
    /// release shipped, as opposed to what it planned, documented or tidied.
    shipped: BTreeSet<String>,
}

fn windows(sections: &[ReleaseSection]) -> Vec<Window> {
    let dated: Vec<Option<String>> = sections.iter().map(|s| s.date.clone()).collect();
    sections
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // the previous release is the next older section that carries a date
            let after = dated[i + 1..].iter().flatten().next().cloned();
            let names = |shipping: bool| -> BTreeSet<String> {
                s.groups
                    .iter()
                    .flat_map(|g| &g.changes)
                    .filter(|c| {
                        !shipping
                            || matches!(
                                c.kind,
                                ChangeKind::Feat | ChangeKind::Fix | ChangeKind::Perf
                            )
                    })
                    .flat_map(|c| &c.references)
                    .filter(|r| r.kind == "issue")
                    .map(|r| r.id.clone())
                    .collect()
            };
            Window {
                version: s.version.clone(),
                date: s.date.clone(),
                after,
                unreleased: s.unreleased,
                named: names(false),
                shipped: names(true),
            }
        })
        .collect()
}

fn inside(window: &Window, stamp: &str) -> bool {
    if stamp.is_empty() {
        return false;
    }
    let after = window.after.as_deref().is_none_or(|a| stamp > a);
    // the unreleased window has no end: everything after the last release is in it
    let until = window.unreleased || window.date.as_deref().is_some_and(|d| stamp <= d);
    after && until
}

fn releases(plan: &Plan, windows: &[Window]) -> Vec<Release> {
    windows
        .iter()
        .map(|w| {
            let mut work: BTreeMap<&str, ShippedWork> = BTreeMap::new();
            for issue in &plan.issues {
                let completed = inside(w, &issue.completed_at);
                let named = w.named.contains(&issue.id);
                if !completed && !named {
                    continue;
                }
                let entry = work
                    .entry(issue.milestone.as_str())
                    .or_insert_with(|| ShippedWork {
                        milestone: issue.milestone.clone(),
                        title: plan
                            .milestone(&issue.milestone)
                            .map(|m| m.title.clone())
                            .unwrap_or_default(),
                        completed: Vec::new(),
                        named: Vec::new(),
                    });
                if completed {
                    entry.completed.push(issue.id.clone());
                }
                if named && !completed {
                    entry.named.push(issue.id.clone());
                }
            }
            Release {
                version: w.version.clone(),
                date: w.date.clone(),
                unreleased: w.unreleased,
                work: work.into_values().collect(),
            }
        })
        .collect()
}

/// Every issue a change of behaviour in one of the last [`PACE_WINDOW`] published releases
/// names, with the newest release that named it. Older releases and other kinds of change are
/// left out: a commit that wrote the plan names every issue it planned, and that is not
/// shipping any of them.
fn shipped_recently(windows: &[Window]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for w in windows.iter().filter(|w| !w.unreleased).take(PACE_WINDOW) {
        for id in &w.shipped {
            out.entry(id.clone()).or_insert_with(|| w.version.clone());
        }
    }
    out
}

fn order_of(plan: &Plan, id: &str) -> i64 {
    plan.milestone(id).map(|m| m.order).unwrap_or_default()
}

fn unblocks(plan: &Plan, id: &str) -> u32 {
    let mut seen = BTreeSet::new();
    let mut stack = vec![id.to_string()];
    while let Some(at) = stack.pop() {
        for d in plan
            .milestone(&at)
            .map(|m| m.dependents.clone())
            .unwrap_or_default()
        {
            if seen.insert(d.clone()) {
                stack.push(d);
            }
        }
    }
    seen.len() as u32
}

fn view(
    plan: &Plan,
    m: &PlanMilestone,
    horizon: Horizon,
    releases: &[Release],
    named_released: &BTreeMap<String, String>,
    intents: &[IntentRef],
    (floor, ceiling, window): (&str, &str, u32),
) -> Milestone {
    let done = m.counts.by_status.get("DONE").copied().unwrap_or_default();
    let remaining = m.counts.required.saturating_sub(done);
    let issues: Vec<_> = plan.issues.iter().filter(|i| i.milestone == m.id).collect();
    // no published release, no window: nothing is counted, so nothing is forecast
    let completed = issues
        .iter()
        .filter(|i| {
            let at = i.completed_at.as_str();
            window > 0 && !at.is_empty() && at > floor && at <= ceiling
        })
        .count() as u32;
    let releases_to_go =
        (completed > 0 && remaining > 0).then(|| (remaining * window).div_ceil(completed));
    let shipped_in: Vec<String> = releases
        .iter()
        .filter(|r| r.work.iter().any(|w| w.milestone == m.id))
        .map(|r| r.version.clone())
        .collect();

    let mut signals = Vec::new();
    let unrecorded: Vec<&str> = issues
        .iter()
        .filter(|i| matches!(i.status.as_str(), "READY" | "BLOCKED"))
        .filter(|i| named_released.contains_key(&i.id))
        .map(|i| i.id.as_str())
        .collect();
    if !unrecorded.is_empty() {
        let first = &unrecorded[0];
        signals.push(Signal {
            code: SignalCode::ShippedUnrecorded,
            message: format!(
                "{} issue(s) named by a feature, fix or performance change released recently (first {first}, in {}) still read as not started in their records",
                unrecorded.len(),
                named_released[*first]
            ),
            subjects: unrecorded.iter().map(|s| s.to_string()).collect(),
        });
    }
    if m.status == "VERIFY" {
        signals.push(Signal {
            code: SignalCode::AwaitsEvidence,
            message: "every required issue is done; the milestone's own evidence is not recorded, so it is not done".to_string(),
            subjects: vec![m.id.clone()],
        });
    }
    if m.counts.total == 0 && horizon != Horizon::Done {
        signals.push(Signal {
            code: SignalCode::NoIssues,
            message: "no issue names this milestone: the outcome is specified and the work is not"
                .to_string(),
            subjects: vec![m.id.clone()],
        });
    }
    let named_recently = releases
        .iter()
        .filter(|r| !r.unreleased)
        .take(window as usize)
        .any(|r| {
            r.work
                .iter()
                .any(|w| w.milestone == m.id && !w.named.is_empty())
        });
    // VERIFY is waiting on evidence, which the signal above says; idle is about work
    if m.status == "ACTIVE" && window > 0 && completed == 0 && !named_recently {
        signals.push(Signal {
            code: SignalCode::Idle,
            message: format!(
                "work has started, and no issue was completed or named by a release in the last {window}"
            ),
            subjects: vec![m.id.clone()],
        });
    }

    Milestone {
        id: m.id.clone(),
        title: m.title.clone(),
        outcome: m.outcome.clone(),
        status: m.status.clone(),
        horizon,
        priority: m.priority.clone(),
        rank: m.rank,
        depends_on: m.depends_on.clone(),
        blocked_by: m.blocked_by.clone(),
        unblocks: unblocks(plan, &m.id),
        claims: m.claims.clone(),
        counts: m.counts.clone(),
        intents: intents
            .iter()
            .filter(|i| i.milestones.contains(&m.id))
            .map(|i| i.id.clone())
            .collect(),
        shipped_in,
        pace: Pace {
            completed,
            window,
            remaining,
            releases: releases_to_go,
        },
        signals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::model::{Change, ChangeGroup, Reference};
    use serde_json::json;

    fn plan() -> Plan {
        let milestone = |id: &str,
                         status: &str,
                         rank: u32,
                         deps: &[&str],
                         dependents: &[&str],
                         done: u32,
                         total: u32| {
            json!({"id": id, "status": status, "order": 1, "priority": "p1", "title": id.to_uppercase(),
              "slug": id, "version": "", "rank": rank, "depends_on": deps,
              "blocked_by": if status == "BLOCKED" { deps.to_vec() } else { vec![] },
              "dependents": dependents, "claims": [],
              "counts": {"total": total, "required": total, "by_status": {"DONE": done}},
              "issues": [], "outcome": "o"})
        };
        let issue = |id: &str, m: &str, status: &str, completed: &str| {
            json!({"id": id, "milestone": m, "status": status, "wave": 0, "priority": "p1",
              "profile": "implementation", "parallel_safe": true, "title": id, "slug": id,
              "depends_on": [], "blocked_by": [], "dependents": [], "scope": [], "serves": [],
              "objective": "", "evidence_have": 0, "evidence_need": 0, "started_at": "",
              "verified_at": "", "completed_at": completed})
        };
        serde_json::from_value(json!({
          "project": {"name": "p", "repository": "", "default_branch": "main", "active_milestone": "a"},
          "statuses": {"issue": [], "milestone": []},
          "milestones": [
            milestone("a", "ACTIVE", 0, &[], &["b"], 2, 4),
            milestone("b", "BLOCKED", 1, &["a"], &["c"], 0, 2),
            milestone("c", "BLOCKED", 2, &["b"], &[], 0, 1),
            milestone("d", "PLANNED", 0, &[], &[], 0, 0),
            milestone("e", "DONE", 0, &[], &[], 1, 1),
            milestone("f", "VERIFY", 0, &[], &[], 1, 1),
            milestone("g", "ACTIVE", 0, &[], &[], 0, 1),
            milestone("x", "CANCELLED", 0, &[], &[], 0, 0)
          ],
          "issues": [
            issue("I1", "a", "DONE", "2026-01-05T00:00:00Z"),
            issue("I2", "a", "DONE", "2026-01-15T00:00:00Z"),
            issue("I3", "a", "READY", ""),
            issue("I4", "a", "ACTIVE", ""),
            issue("I5", "b", "BLOCKED", ""),
            issue("I6", "b", "BLOCKED", ""),
            issue("I7", "c", "BLOCKED", ""),
            issue("I8", "e", "DONE", "2025-12-01T00:00:00Z"),
            issue("I9", "f", "DONE", "2026-01-21T00:00:00Z"),
            issue("I10", "g", "ACTIVE", "")
          ],
          "waves": [], "edges": [], "milestone_edges": [], "findings": []
        }))
        .unwrap()
    }

    fn section(version: &str, date: Option<&str>, names: &[&str]) -> ReleaseSection {
        section_of(ChangeKind::Feat, version, date, names)
    }

    fn section_of(
        kind: ChangeKind,
        version: &str,
        date: Option<&str>,
        names: &[&str],
    ) -> ReleaseSection {
        ReleaseSection {
            version: version.to_string(),
            tag: date.map(|_| format!("v{version}")),
            date: date.map(str::to_string),
            commit: None,
            unreleased: date.is_none(),
            notes_url: None,
            compare_url: None,
            tree_url: None,
            decisions: Vec::new(),
            groups: vec![ChangeGroup {
                kind,
                heading: "Features".to_string(),
                rank: 0,
                changes: vec![Change {
                    kind,
                    scope: None,
                    subject: "s".to_string(),
                    breaking: false,
                    commit: "abc".to_string(),
                    url: None,
                    references: names
                        .iter()
                        .map(|id| Reference {
                            kind: "issue".to_string(),
                            id: id.to_string(),
                            title: id.to_string(),
                            route: None,
                        })
                        .collect(),
                }],
            }],
            artifacts: Vec::new(),
        }
    }

    fn sections() -> Vec<ReleaseSection> {
        vec![
            section("unreleased", None, &[]),
            section("0.3.0", Some("2026-01-20T00:00:00Z"), &["I5"]),
            section("0.2.0", Some("2026-01-10T00:00:00Z"), &["I1"]),
            // a commit that wrote the plan names an issue without shipping it
            section_of(
                ChangeKind::Docs,
                "0.1.0",
                Some("2026-01-01T00:00:00Z"),
                &["I7"],
            ),
        ]
    }

    fn milestone<'a>(r: &'a Roadmap, id: &str) -> &'a Milestone {
        r.horizons
            .iter()
            .flat_map(|h| &h.milestones)
            .find(|m| m.id == id)
            .unwrap()
    }

    #[test]
    fn each_milestone_stands_in_the_horizon_its_status_derives_and_in_graph_order() {
        let r = infer(&plan(), &sections(), &[], "0.3.0");
        let ids = |h: Horizon| -> Vec<String> {
            r.horizons
                .iter()
                .find(|v| v.horizon == h)
                .unwrap()
                .milestones
                .iter()
                .map(|m| m.id.clone())
                .collect()
        };
        // a unblocks two milestones through b, so it leads its rank
        assert_eq!(ids(Horizon::Now), ["a", "f", "g"]);
        assert_eq!(ids(Horizon::Next), ["d"]);
        assert_eq!(ids(Horizon::Later), ["b", "c"]);
        assert_eq!(ids(Horizon::Done), ["e"]);
        // a cancelled milestone leaves the roadmap
        assert!(r
            .horizons
            .iter()
            .all(|h| h.milestones.iter().all(|m| m.id != "x")));
        assert_eq!(milestone(&r, "a").unblocks, 2);
        assert_eq!(r.schema, SCHEMA);
        assert_eq!(r.basis.len(), 6);
    }

    #[test]
    fn a_release_ships_what_was_completed_in_its_window_and_what_its_commits_name() {
        let r = infer(&plan(), &sections(), &[], "0.3.0");
        let release = |v: &str| r.releases.iter().find(|x| x.version == v).unwrap();
        let work = |v: &str, m: &str| release(v).work.iter().find(|w| w.milestone == m).cloned();
        // I1 was completed on 01-05, inside (0.1.0, 0.2.0], and the 0.2.0 commits name it
        let a2 = work("0.2.0", "a").unwrap();
        assert_eq!(a2.completed, ["I1"]);
        // named and completed in one release is said once, as completed
        assert!(a2.named.is_empty());
        assert_eq!(work("0.3.0", "a").unwrap().completed, ["I2"]);
        assert_eq!(work("0.3.0", "b").unwrap().named, ["I5"]);
        // completed before the oldest release: in the oldest window, which has no floor
        assert_eq!(work("0.1.0", "e").unwrap().completed, ["I8"]);
        // after the newest release: the unreleased window
        assert_eq!(work("unreleased", "f").unwrap().completed, ["I9"]);
        assert_eq!(milestone(&r, "a").shipped_in, ["0.3.0", "0.2.0"]);
    }

    #[test]
    fn a_pace_is_measured_and_a_forecast_is_given_only_where_one_was() {
        let r = infer(&plan(), &sections(), &[], "0.3.0");
        let a = &milestone(&r, "a").pace;
        assert_eq!((a.completed, a.window, a.remaining), (2, 3, 2));
        // two remaining at two per three releases: three releases
        assert_eq!(a.releases, Some(3));
        let g = &milestone(&r, "g").pace;
        assert_eq!(g.completed, 0);
        assert_eq!(g.releases, None);
    }

    #[test]
    fn a_signal_reports_two_committed_facts_that_disagree() {
        let r = infer(&plan(), &sections(), &[], "0.3.0");
        let codes = |id: &str| -> Vec<SignalCode> {
            milestone(&r, id).signals.iter().map(|s| s.code).collect()
        };
        // I5 is BLOCKED in its record, and 0.3.0 shipped a commit naming it
        assert_eq!(codes("b"), [SignalCode::ShippedUnrecorded]);
        assert!(milestone(&r, "b").signals[0].message.contains("0.3.0"));
        assert_eq!(codes("f"), [SignalCode::AwaitsEvidence]);
        assert_eq!(codes("d"), [SignalCode::NoIssues]);
        assert_eq!(codes("g"), [SignalCode::Idle]);
        assert!(codes("a").is_empty());
        // c's I7 was named by a docs commit, which is not shipping it
        assert!(codes("c").is_empty());
        assert!(codes("e").is_empty());
        assert_eq!(r.signals, 4);
    }

    #[test]
    fn a_milestone_names_the_intents_it_realises() {
        let intent = IntentRef {
            id: "i".to_string(),
            title: "I".to_string(),
            stage: "realizing".to_string(),
            met: 1,
            criteria: 3,
            milestones: vec!["a".to_string()],
        };
        let r = infer(&plan(), &sections(), &[intent], "0.3.0");
        assert_eq!(milestone(&r, "a").intents, ["i"]);
        assert!(milestone(&r, "b").intents.is_empty());
        assert_eq!(r.intents.len(), 1);
    }

    #[test]
    fn with_no_release_nothing_is_attributed_and_nothing_is_forecast() {
        let r = infer(&plan(), &[], &[], "0.0.0");
        assert!(r.releases.is_empty());
        let a = &milestone(&r, "a").pace;
        assert_eq!((a.window, a.releases), (0, None));
        // no window: idle cannot be judged, so it is not said
        assert!(milestone(&r, "g").signals.is_empty());
        for h in [Horizon::Now, Horizon::Next, Horizon::Later, Horizon::Done] {
            assert!(!h.describe().1.is_empty());
        }
    }
}
