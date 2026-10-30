//! Accumulation, watched: the worktrees, the merged branches and the free disk of a
//! repository, each measured against the budget its policy declares (ADR 0150).
//!
//! Three quantities grow because nothing decided that they should, and none is visible from
//! the place a person looks: a worktree is registered by whoever starts a piece of work and
//! removed by nobody, a branch that has landed keeps its name, and the disk under both is
//! measurable as a problem only once nothing that could measure it still runs. Each already
//! has a command that reduces it safely. What none had is what a session's context has: a
//! watch that reads the quantity at the moments a worker is already listening, says when a
//! budget is crossed, and acts when the policy has consented to that.
//!
//! This module is the measurement and the verdict, and nothing else. It reads git through
//! [`crate::worktree::WorktreeService`] with [`Detail::Fast`] — no subprocess per worktree,
//! so a provider's start event can afford it — and the volume through `df`, the same reading
//! the build floor in `lib/rust_bin.sh` takes. Three rules hold everywhere in it:
//!
//! - a budget is the policy's and never a constant here: a quantity with no declared budget
//!   is measured and reported as [`BudgetState::Unbudgeted`], which is neither a pass nor a finding;
//! - a reading that could not be taken is [`BudgetState::Unknown`] with its reason, never a zero
//!   and never an extreme value — the sweep that made three active worktrees the oldest
//!   candidates in the set did it by substituting an empty string into arithmetic;
//! - what reduces a quantity is named beside it, and it is always a command that re-measures
//!   every candidate before it removes one.
//!
//! The lifecycle, from a policy and three readings to what a briefing and the mesh are told:
//!
//! ```
//! use majordomus_cli::accumulation::{
//!     judge, AccumulationPolicy, AccumulationReport, BudgetState, Readings,
//! };
//!
//! // the policy declares two budgets and leaves the third quantity unbudgeted
//! let policy: AccumulationPolicy = serde_json::from_str(
//!     r#"{"watch": true, "worktrees_max": 60, "disk_free_min_mb": 51200}"#,
//! )
//! .unwrap();
//! // the readings: what git registers, what the trunk reaches, what the volume has left
//! let readings = Readings {
//!     worktrees: Ok(247),
//!     merged_branches: Ok(118),
//!     disk_free_mb: Ok(48_447),
//! };
//! let report = AccumulationReport::of("/repo", &policy, judge(&readings, &policy));
//! assert_eq!(report.over, 2);
//! assert!(!report.within_budget);
//! assert_eq!(report.quantities[1].state, BudgetState::Unbudgeted);
//! // a briefing gets one line per crossed budget, the mesh gets one line in all
//! assert_eq!(report.brief().len(), 2);
//! assert_eq!(
//!     report.headline().as_deref(),
//!     Some("worktrees 247 (at most 60); disk_free 48447 (at least 51200)")
//! );
//! assert_eq!(
//!     report.summary(),
//!     "accumulation: 2 of 3 quantities over budget; policy says report"
//! );
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::worktree::{Detail, RepositoryTopology, WorktreeError, WorktreeService};

/// The schema of an [`AccumulationReport`].
pub const SCHEMA: &str = "majordomus/accumulation/v1";

/// How often a reduction may start when the policy does not say, in minutes.
pub const DEFAULT_COOLDOWN_MINUTES: u64 = 10;

/// The command that removes what is spare: the worktrees of landed branches, then the
/// names of landed branches nothing has checked out.
pub const REDUCE_COMMAND: &str = "majordomus worktree cleanup --remove --branches";

/// What the watch may do when a budget is crossed.
///
/// ```
/// use majordomus_cli::accumulation::AccumulationAct;
/// // a policy that says nothing has consented to nothing
/// assert_eq!(AccumulationAct::default(), AccumulationAct::Report);
/// let act: AccumulationAct = serde_json::from_str("\"reduce\"").unwrap();
/// assert_eq!(act, AccumulationAct::Reduce);
/// assert_eq!(act.as_str(), "reduce");
/// assert_eq!(AccumulationAct::Report.as_str(), "report");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccumulationAct {
    /// Say it, and name the command. Nothing is removed.
    #[default]
    Report,
    /// Say it, and run the command that removes what is provably spare.
    Reduce,
}

impl AccumulationAct {
    /// The word the policy writes for this consent, and the word a report prints back.
    ///
    /// ```
    /// use majordomus_cli::accumulation::AccumulationAct;
    /// assert_eq!(AccumulationAct::Report.as_str(), "report");
    /// assert_eq!(AccumulationAct::Reduce.as_str(), "reduce");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            AccumulationAct::Report => "report",
            AccumulationAct::Reduce => "reduce",
        }
    }
}

/// `accumulation:` — the budgets a repository declares, and what crossing one may do.
///
/// ```
/// use majordomus_cli::accumulation::{AccumulationPolicy, AccumulationAct};
/// let declared: AccumulationPolicy = serde_json::from_str(
///     r#"{"watch": true, "act": "reduce", "worktrees_max": 40, "disk_free_min_mb": 51200}"#,
/// )
/// .expect("an accumulation block");
/// assert!(declared.watch);
/// assert_eq!(declared.act, AccumulationAct::Reduce);
/// assert_eq!(declared.worktrees_max, Some(40));
/// assert_eq!(declared.merged_branches_max, None, "absent is absent, not a default");
/// // a policy that predates the block watches nothing and removes nothing
/// let absent = AccumulationPolicy::default();
/// assert!(!absent.watch);
/// assert_eq!(absent.act, AccumulationAct::Report);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct AccumulationPolicy {
    /// Whether a provider's episode events read the quantities and say what is over budget.
    #[serde(default)]
    pub watch: bool,
    /// What crossing a budget may do.
    #[serde(default)]
    pub act: AccumulationAct,
    /// More registered worktrees than this is a finding.
    #[serde(default)]
    pub worktrees_max: Option<u64>,
    /// More local branches already merged into the trunk than this is a finding.
    #[serde(default)]
    pub merged_branches_max: Option<u64>,
    /// Less free space than this on the repository's volume, in MiB, is a finding.
    #[serde(default)]
    pub disk_free_min_mb: Option<u64>,
    /// A reduction starts at most once in this many minutes for the whole repository.
    #[serde(default)]
    pub cooldown_minutes: Option<u64>,
    /// Whether a crossed budget is told to the mesh as an advisory claim, so that the other
    /// machines working on this repository hear of it without anybody writing an alert.
    #[serde(default)]
    pub announce_on_mesh: bool,
}

impl AccumulationPolicy {
    /// The cooldown in force: the declared one, or [`DEFAULT_COOLDOWN_MINUTES`].
    ///
    /// ```
    /// use majordomus_cli::accumulation::{AccumulationPolicy, DEFAULT_COOLDOWN_MINUTES};
    /// assert_eq!(AccumulationPolicy::default().cooldown(), DEFAULT_COOLDOWN_MINUTES);
    /// let p = AccumulationPolicy { cooldown_minutes: Some(3), ..Default::default() };
    /// assert_eq!(p.cooldown(), 3);
    /// ```
    pub fn cooldown(&self) -> u64 {
        self.cooldown_minutes.unwrap_or(DEFAULT_COOLDOWN_MINUTES)
    }
}

/// Which side of its budget a quantity must stay on.
///
/// ```
/// use majordomus_cli::accumulation::{judge, AccumulationPolicy, BudgetBound, Readings};
/// let q = judge(&Readings::unread("not asked"), &AccumulationPolicy::default());
/// // things that pile up are bounded from above, what is left of the disk from below
/// assert_eq!(q[0].bound, BudgetBound::AtMost);
/// assert_eq!(q[2].bound, BudgetBound::AtLeast);
/// assert_eq!(serde_json::to_value(BudgetBound::AtLeast).unwrap(), "at_least");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetBound {
    /// The measurement must not exceed the budget: a count of things that pile up.
    AtMost,
    /// The measurement must not fall below the budget: what is left of something.
    AtLeast,
}

impl BudgetBound {
    /// The words a report puts between a reading and its budget.
    ///
    /// ```
    /// use majordomus_cli::accumulation::BudgetBound;
    /// assert_eq!(BudgetBound::AtMost.as_str(), "at most");
    /// assert_eq!(BudgetBound::AtLeast.as_str(), "at least");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            BudgetBound::AtMost => "at most",
            BudgetBound::AtLeast => "at least",
        }
    }
}

/// Where a quantity stands against its budget.
///
/// ```
/// use majordomus_cli::accumulation::{judge, AccumulationPolicy, BudgetState, Readings};
/// let policy = AccumulationPolicy { worktrees_max: Some(3), ..Default::default() };
/// let state = |worktrees| {
///     let readings = Readings { worktrees, ..Readings::unread("not asked") };
///     judge(&readings, &policy)[0].state
/// };
/// assert_eq!(state(Ok(3)), BudgetState::Ok);
/// assert_eq!(state(Ok(4)), BudgetState::Over);
/// assert_eq!(state(Err("git is absent".into())), BudgetState::Unknown);
/// // with no budget there is a reading and no verdict
/// let free = judge(&Readings { worktrees: Ok(9), ..Readings::unread("x") }, &AccumulationPolicy::default());
/// assert_eq!(free[0].state, BudgetState::Unbudgeted);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    /// Measured, and on the right side of its budget.
    Ok,
    /// Measured, and on the wrong side of its budget.
    Over,
    /// Measured, with no budget declared for it: neither a pass nor a finding.
    Unbudgeted,
    /// Not measured. The reason is in the evidence; it is never read as a pass.
    Unknown,
}

impl BudgetState {
    /// The word a report prints at the start of a quantity's line, and the JSON value.
    ///
    /// ```
    /// use majordomus_cli::accumulation::BudgetState;
    /// assert_eq!(BudgetState::Over.as_str(), "over");
    /// assert_eq!(BudgetState::Unbudgeted.as_str(), "unbudgeted");
    /// assert_eq!(serde_json::to_value(BudgetState::Unknown).unwrap(), BudgetState::Unknown.as_str());
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            BudgetState::Ok => "ok",
            BudgetState::Over => "over",
            BudgetState::Unbudgeted => "unbudgeted",
            BudgetState::Unknown => "unknown",
        }
    }
}

/// One quantity that accumulates: what was measured, what the policy allows, and what
/// reduces it.
///
/// ```
/// use majordomus_cli::accumulation::{judge, AccumulationPolicy, BudgetState, Quantity, Readings};
/// let policy = AccumulationPolicy { merged_branches_max: Some(20), ..Default::default() };
/// let readings = Readings { merged_branches: Ok(118), ..Readings::unread("not asked") };
/// let q: &Quantity = &judge(&readings, &policy)[1];
/// assert_eq!((q.id.as_str(), q.measured, q.budget), ("merged_branches", Some(118), Some(20)));
/// assert_eq!(q.state, BudgetState::Over);
/// assert_eq!(q.policy_key, "accumulation.merged_branches_max");
/// assert_eq!(q.reduce, majordomus_cli::accumulation::REDUCE_COMMAND);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Quantity {
    /// `worktrees`, `merged_branches` or `disk_free`.
    pub id: String,
    /// What is counted, in words.
    pub title: String,
    /// The reading, absent when it could not be taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured: Option<u64>,
    /// The unit of the reading and of the budget.
    pub unit: String,
    /// The budget the policy declares, absent when it declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u64>,
    /// Which side of the budget the reading must stay on.
    pub bound: BudgetBound,
    /// Where the reading stands.
    pub state: BudgetState,
    /// Where the reading was taken, or why it could not be.
    pub evidence: String,
    /// The policy key that declares the budget.
    pub policy_key: String,
    /// The command that reduces the quantity. It re-measures every candidate before it acts.
    pub reduce: String,
}

impl Quantity {
    /// One line for a person: the state, the reading against its budget, and — when the
    /// budget is crossed — the command that reduces it.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationPolicy, Readings};
    /// let policy = AccumulationPolicy { worktrees_max: Some(2), ..Default::default() };
    /// let readings = Readings { worktrees: Ok(5), ..Readings::unread("not asked") };
    /// let q = &judge(&readings, &policy)[0];
    /// assert_eq!(
    ///     q.line(),
    ///     "over        worktrees        5 worktree(s), budget at most 2 — majordomus worktree cleanup --remove --branches"
    /// );
    /// ```
    pub fn line(&self) -> String {
        let reading = match self.measured {
            Some(n) => format!("{n} {}", self.unit),
            None => format!("not measured: {}", self.evidence),
        };
        let budget = match self.budget {
            Some(b) => format!(", budget {} {b}", self.bound.as_str()),
            None => format!(", no budget ({} is not declared)", self.policy_key),
        };
        let remedy = if self.state == BudgetState::Over {
            format!(" — {}", self.reduce)
        } else {
            String::new()
        };
        format!(
            "{:<11} {:<16} {reading}{budget}{remedy}",
            self.state.as_str(),
            self.id
        )
    }
}

/// The readings a verdict is made from. Each is the number or the reason there is none.
///
/// ```
/// use majordomus_cli::accumulation::Readings;
/// let readings = Readings {
///     worktrees: Ok(5),
///     merged_branches: Err("no branch could be compared with a trunk".into()),
///     disk_free_mb: Ok(60_000),
/// };
/// assert!(readings.merged_branches.is_err(), "a reading that was not taken says why");
/// assert_ne!(readings, Readings::unread("git is absent"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readings {
    /// Registered worktrees, the primary checkout among them.
    pub worktrees: Result<u64, String>,
    /// Local branches other than the trunk that the trunk, or the branch it follows, reaches.
    pub merged_branches: Result<u64, String>,
    /// Free space on the repository's volume, in MiB.
    pub disk_free_mb: Result<u64, String>,
}

impl Readings {
    /// Readings none of which was taken, each for the same reason.
    ///
    /// ```
    /// use majordomus_cli::accumulation::Readings;
    /// let blind = Readings::unread("git is absent");
    /// assert_eq!(blind.worktrees, Err("git is absent".to_string()));
    /// assert_eq!(blind.merged_branches, blind.disk_free_mb);
    /// ```
    pub fn unread(reason: &str) -> Readings {
        Readings {
            worktrees: Err(reason.to_string()),
            merged_branches: Err(reason.to_string()),
            disk_free_mb: Err(reason.to_string()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn quantity(
    id: &str,
    title: &str,
    unit: &str,
    bound: BudgetBound,
    reading: &Result<u64, String>,
    budget: Option<u64>,
    evidence: &str,
    reduce: &str,
) -> Quantity {
    let (measured, state, evidence) = match (reading, budget) {
        (Err(why), _) => (None, BudgetState::Unknown, why.clone()),
        (Ok(n), None) => (Some(*n), BudgetState::Unbudgeted, evidence.to_string()),
        (Ok(n), Some(b)) => {
            let within = match bound {
                BudgetBound::AtMost => n <= &b,
                BudgetBound::AtLeast => n >= &b,
            };
            (
                Some(*n),
                if within {
                    BudgetState::Ok
                } else {
                    BudgetState::Over
                },
                evidence.to_string(),
            )
        }
    };
    Quantity {
        id: id.to_string(),
        title: title.to_string(),
        measured,
        unit: unit.to_string(),
        budget,
        bound,
        state,
        evidence,
        policy_key: format!(
            "accumulation.{id}_{}",
            match bound {
                BudgetBound::AtMost => "max",
                BudgetBound::AtLeast => "min_mb",
            }
        ),
        reduce: reduce.to_string(),
    }
}

/// The verdict on each quantity: the readings against the budgets, in the order a report
/// lists them.
///
/// ```
/// use majordomus_cli::accumulation::{judge, AccumulationPolicy, Readings, BudgetState};
/// let policy = AccumulationPolicy {
///     worktrees_max: Some(10),
///     disk_free_min_mb: Some(1000),
///     ..Default::default()
/// };
/// let readings = Readings {
///     worktrees: Ok(10),
///     merged_branches: Ok(7),
///     disk_free_mb: Ok(999),
/// };
/// let q = judge(&readings, &policy);
/// assert_eq!(q[0].state, BudgetState::Ok, "at the budget is within it");
/// assert_eq!(q[1].state, BudgetState::Unbudgeted, "no budget is no verdict");
/// assert_eq!(q[2].state, BudgetState::Over, "one MiB under the floor is under it");
/// // a reading that could not be taken is never a pass, with or without a budget
/// let blind = judge(&Readings::unread("git is absent"), &policy);
/// assert!(blind.iter().all(|q| q.state == BudgetState::Unknown && q.measured.is_none()));
/// assert_eq!(blind[0].evidence, "git is absent");
/// ```
pub fn judge(readings: &Readings, policy: &AccumulationPolicy) -> Vec<Quantity> {
    vec![
        quantity(
            "worktrees",
            "Registered worktrees, the primary checkout among them",
            "worktree(s)",
            BudgetBound::AtMost,
            &readings.worktrees,
            policy.worktrees_max,
            "git's registry of worktrees",
            REDUCE_COMMAND,
        ),
        quantity(
            "merged_branches",
            "Local branches the trunk already reaches",
            "branch(es)",
            BudgetBound::AtMost,
            &readings.merged_branches,
            policy.merged_branches_max,
            "local branches reachable from the trunk or from the branch it follows",
            REDUCE_COMMAND,
        ),
        quantity(
            "disk_free",
            "Free space on the repository's volume",
            "MiB free",
            BudgetBound::AtLeast,
            &readings.disk_free_mb,
            policy.disk_free_min_mb,
            "df on the repository's volume",
            REDUCE_COMMAND,
        ),
    ]
}

/// The last reduction the watch ran in this repository, as it recorded it.
///
/// ```
/// use majordomus_cli::accumulation::LastReduction;
/// let done: LastReduction = serde_json::from_str(
///     r#"{"at": 1791850000, "because": ["worktrees"], "removed": ["feature/a"],
///         "deleted": ["feature/a", "fix/b"], "refused": 4}"#,
/// )
/// .unwrap();
/// assert_eq!((done.removed.len(), done.deleted.len(), done.refused), (1, 2, 4));
/// assert_eq!(done.because, ["worktrees"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LastReduction {
    /// When it started, in Unix seconds.
    pub at: u64,
    /// The quantities that were over budget and asked for it.
    pub because: Vec<String>,
    /// Branches whose worktree was removed.
    pub removed: Vec<String>,
    /// Branches whose name was deleted.
    pub deleted: Vec<String>,
    /// How many candidates were kept, each for a reading that said so.
    pub refused: usize,
}

/// Where the watch keeps what it last did: under the local half of the primary checkout, so
/// that every worktree of a repository reads one record and one cooldown.
///
/// ```
/// use majordomus_cli::accumulation::record_path;
/// use std::path::Path;
/// assert_eq!(
///     record_path(Path::new("/r"), ".ai/local"),
///     Path::new("/r/.ai/local/state/accumulation/last.json")
/// );
/// ```
pub fn record_path(primary: &Path, local: &str) -> std::path::PathBuf {
    primary.join(local).join("state/accumulation/last.json")
}

/// The record at `path`, or `None` when there is none or it cannot be read: a watch that has
/// never acted and one whose record is damaged are both due.
///
/// ```
/// use majordomus_cli::accumulation::{last_reduction, record, LastReduction};
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("state/accumulation/last.json");
/// assert_eq!(last_reduction(&path), None, "nothing was ever done here");
/// let done = LastReduction { at: 7, because: vec![], removed: vec![], deleted: vec![], refused: 0 };
/// record(&path, &done).unwrap();
/// assert_eq!(last_reduction(&path), Some(done));
/// std::fs::write(&path, "not a record").unwrap();
/// assert_eq!(last_reduction(&path), None, "a damaged record is no record");
/// ```
pub fn last_reduction(path: &Path) -> Option<LastReduction> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether a reduction may start at `now`: never sooner than `cooldown_minutes` after the
/// last one. A record from the future is not a reason to wait for it.
///
/// ```
/// use majordomus_cli::accumulation::is_due;
/// assert!(is_due(None, 10, 1_000), "nothing was ever done");
/// assert!(!is_due(Some(1_000), 10, 1_599), "ten minutes have not passed");
/// assert!(is_due(Some(1_000), 10, 1_600));
/// assert!(is_due(Some(5_000), 10, 1_000), "a clock that went back does not stop the watch");
/// assert!(is_due(Some(1_000), 0, 1_000), "no cooldown is no wait");
/// ```
pub fn is_due(last: Option<u64>, cooldown_minutes: u64, now: u64) -> bool {
    match last {
        None => true,
        Some(at) if at > now => true,
        Some(at) => now - at >= cooldown_minutes.saturating_mul(60),
    }
}

/// Write the record of a reduction, creating its directory. The record is written before the
/// reduction runs as well as after it: the first write is what makes a second session's
/// watch find the cooldown already taken.
///
/// ```
/// use majordomus_cli::accumulation::{is_due, last_reduction, record, LastReduction};
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("deep/er/last.json");
/// let started = LastReduction { at: 1_000, because: vec!["disk_free".into()], removed: vec![], deleted: vec![], refused: 0 };
/// record(&path, &started).unwrap();
/// // the directory was made, and a second session now finds the cooldown taken
/// let at = last_reduction(&path).map(|l| l.at);
/// assert!(!is_due(at, 30, 1_060));
/// ```
pub fn record(path: &Path, reduction: &LastReduction) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(reduction).unwrap_or_default()
        ),
    )
}

/// What a repository has accumulated, against what its policy allows.
///
/// ```
/// use majordomus_cli::accumulation::{judge, AccumulationPolicy, AccumulationReport, Readings, SCHEMA};
/// let policy = AccumulationPolicy { watch: true, disk_free_min_mb: Some(100), ..Default::default() };
/// let readings = Readings { worktrees: Ok(2), merged_branches: Ok(0), disk_free_mb: Ok(99) };
/// let report = AccumulationReport::of("/repo", &policy, judge(&readings, &policy));
/// let json = serde_json::to_value(&report).unwrap();
/// assert_eq!(json["schema"], SCHEMA);
/// assert_eq!(json["over"], 1);
/// assert_eq!(json["within_budget"], false);
/// assert_eq!(json["quantities"][2]["state"], "over");
/// assert!(json.get("last_reduction").is_none(), "absent until the watch has acted");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccumulationReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// The primary checkout of the repository that was measured.
    pub repository: String,
    /// Whether the policy has the episode events read this.
    pub watched: bool,
    /// Whether a crossed budget is told to the mesh.
    #[serde(default)]
    pub announce_on_mesh: bool,
    /// What the policy lets a crossed budget do.
    pub act: AccumulationAct,
    /// How often a reduction may start, in minutes.
    pub cooldown_minutes: u64,
    /// Every quantity, with its reading, its budget and its state.
    pub quantities: Vec<Quantity>,
    /// How many quantities are over their budget.
    pub over: usize,
    /// How many could not be measured.
    pub unknown: usize,
    /// Whether nothing is over budget. A quantity that could not be measured does not make
    /// this false, and is not hidden by it: `unknown` says how many there are.
    pub within_budget: bool,
    /// What the watch last removed here, when it has ever acted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reduction: Option<LastReduction>,
}

impl AccumulationReport {
    /// The report over a set of verdicts: the tallies, and what the policy lets a crossed
    /// budget do.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationAct, AccumulationPolicy, AccumulationReport, Readings};
    /// let policy = AccumulationPolicy { act: AccumulationAct::Reduce, cooldown_minutes: Some(30), ..Default::default() };
    /// let report = AccumulationReport::of("/repo", &policy, judge(&Readings::unread("no git"), &policy));
    /// assert_eq!((report.over, report.unknown), (0, 3));
    /// assert!(report.within_budget, "not measured is not over budget, and is counted beside it");
    /// assert_eq!((report.act, report.cooldown_minutes), (AccumulationAct::Reduce, 30));
    /// ```
    pub fn of(repository: &str, policy: &AccumulationPolicy, quantities: Vec<Quantity>) -> Self {
        let over = quantities
            .iter()
            .filter(|q| q.state == BudgetState::Over)
            .count();
        let unknown = quantities
            .iter()
            .filter(|q| q.state == BudgetState::Unknown)
            .count();
        AccumulationReport {
            schema: SCHEMA.to_string(),
            repository: repository.to_string(),
            watched: policy.watch,
            announce_on_mesh: policy.announce_on_mesh,
            act: policy.act,
            cooldown_minutes: policy.cooldown(),
            quantities,
            over,
            unknown,
            within_budget: over == 0,
            last_reduction: None,
        }
    }

    /// The one line a report ends with.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationPolicy, AccumulationReport, Readings};
    /// let policy = AccumulationPolicy { worktrees_max: Some(1), ..Default::default() };
    /// let readings = Readings { worktrees: Ok(3), merged_branches: Ok(0), disk_free_mb: Err("no df".into()) };
    /// let report = AccumulationReport::of("/r", &policy, judge(&readings, &policy));
    /// assert_eq!(
    ///     report.summary(),
    ///     "accumulation: 1 of 3 quantities over budget, 1 not measured; policy says report"
    /// );
    /// assert!(!report.within_budget);
    /// ```
    pub fn summary(&self) -> String {
        let unknown = if self.unknown == 0 {
            String::new()
        } else {
            format!(", {} not measured", self.unknown)
        };
        format!(
            "accumulation: {} of {} quantities over budget{unknown}; policy says {}",
            self.over,
            self.quantities.len(),
            self.act.as_str()
        )
    }

    /// What is over budget, in one line a mesh claim can carry: each quantity with its
    /// reading and its budget, and nothing when every budget holds.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationPolicy, AccumulationReport, Readings};
    /// let policy = AccumulationPolicy {
    ///     worktrees_max: Some(2),
    ///     disk_free_min_mb: Some(900),
    ///     ..Default::default()
    /// };
    /// let readings = Readings { worktrees: Ok(5), merged_branches: Ok(0), disk_free_mb: Ok(100) };
    /// let report = AccumulationReport::of("/r", &policy, judge(&readings, &policy));
    /// assert_eq!(
    ///     report.headline().as_deref(),
    ///     Some("worktrees 5 (at most 2); disk_free 100 (at least 900)")
    /// );
    /// let fine = Readings { worktrees: Ok(1), merged_branches: Ok(0), disk_free_mb: Ok(900) };
    /// assert_eq!(AccumulationReport::of("/r", &policy, judge(&fine, &policy)).headline(), None);
    /// ```
    pub fn headline(&self) -> Option<String> {
        let over: Vec<String> = self
            .quantities
            .iter()
            .filter(|q| q.state == BudgetState::Over)
            .map(|q| {
                format!(
                    "{} {} ({} {})",
                    q.id,
                    q.measured.unwrap_or_default(),
                    q.bound.as_str(),
                    q.budget.unwrap_or_default()
                )
            })
            .collect();
        (!over.is_empty()).then(|| over.join("; "))
    }

    /// The quantities that are over budget, by id: what a reduction records as its reason.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationPolicy, AccumulationReport, Readings};
    /// let policy = AccumulationPolicy { worktrees_max: Some(1), merged_branches_max: Some(1), ..Default::default() };
    /// let readings = Readings { worktrees: Ok(2), merged_branches: Ok(1), disk_free_mb: Ok(0) };
    /// let report = AccumulationReport::of("/repo", &policy, judge(&readings, &policy));
    /// assert_eq!(report.over_ids(), ["worktrees"]);
    /// ```
    pub fn over_ids(&self) -> Vec<String> {
        self.quantities
            .iter()
            .filter(|q| q.state == BudgetState::Over)
            .map(|q| q.id.clone())
            .collect()
    }

    /// What a briefing carries: one line per quantity that is over budget or could not be
    /// measured, and nothing at all when every budget holds — text that arrives in every
    /// episode is paid for in every episode.
    ///
    /// ```
    /// use majordomus_cli::accumulation::{judge, AccumulationPolicy, AccumulationReport, Readings};
    /// let policy = AccumulationPolicy { worktrees_max: Some(9), ..Default::default() };
    /// let fine = Readings { worktrees: Ok(3), merged_branches: Ok(0), disk_free_mb: Ok(1) };
    /// assert!(AccumulationReport::of("/r", &policy, judge(&fine, &policy)).brief().is_empty());
    /// let over = Readings { worktrees: Ok(30), ..fine };
    /// let lines = AccumulationReport::of("/r", &policy, judge(&over, &policy)).brief();
    /// assert_eq!(lines.len(), 1);
    /// assert!(lines[0].starts_with("over        worktrees"), "{}", lines[0]);
    /// ```
    pub fn brief(&self) -> Vec<String> {
        self.quantities
            .iter()
            .filter(|q| {
                q.state == BudgetState::Over
                    || (q.state == BudgetState::Unknown && q.budget.is_some())
            })
            .map(Quantity::line)
            .collect()
    }
}

/// The readings a topology carries: how many worktrees git registers, and how many local
/// branches other than the trunk it already reaches. The second is unknown when no branch
/// could be compared with a trunk — a repository whose trunk could not be decided has not
/// merged nothing, it has not been asked.
///
/// ```
/// use majordomus_cli::accumulation::readings_of;
/// use majordomus_cli::worktree::{Detail, WorktreeService};
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     assert!(std::process::Command::new("git")
///         .current_dir(dir.path())
///         .args(["-c", "user.email=t@example.com", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
///         .args(args)
///         .status()
///         .unwrap()
///         .success());
/// };
/// git(&["init", "-q", "-b", "main"]);
/// std::fs::write(dir.path().join("a"), "a").unwrap();
/// git(&["add", "-A"]);
/// git(&["commit", "-q", "-m", "one"]);
/// git(&["branch", "landed"]);
/// let topology = WorktreeService::open(dir.path()).unwrap().topology(Detail::Fast).unwrap();
/// assert_eq!(readings_of(&topology), (Ok(1), Ok(1)), "one checkout, one branch the trunk reaches");
/// ```
pub fn readings_of(topology: &RepositoryTopology) -> (Result<u64, String>, Result<u64, String>) {
    let worktrees = Ok(topology.tallies.worktrees as u64);
    let others: Vec<_> = topology.branches.iter().filter(|b| !b.trunk).collect();
    let merged = if !others.is_empty() && others.iter().all(|b| b.merged_into_trunk.is_none()) {
        Err("no branch could be compared with a trunk".to_string())
    } else {
        Ok(others
            .iter()
            .filter(|b| b.merged_into_trunk == Some(true))
            .count() as u64)
    };
    (worktrees, merged)
}

/// The free space `df -Pk` reports, in MiB: the fourth column of its second line. `None`
/// when the output has no such number — a reading that is absent, never a zero.
///
/// ```
/// use majordomus_cli::accumulation::parse_df;
/// let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s1 971350180 15000000 61440000 20% /\n";
/// assert_eq!(parse_df(out), Some(60000));
/// assert_eq!(parse_df("Filesystem 1024-blocks Used Available\n"), None);
/// assert_eq!(parse_df("h\n/dev/x 1 2 - 3% /\n"), None, "a dash is not a number");
/// assert_eq!(parse_df(""), None);
/// ```
pub fn parse_df(output: &str) -> Option<u64> {
    let row = output.lines().nth(1)?;
    let available: u64 = row.split_whitespace().nth(3)?.parse().ok()?;
    Some(available / 1024)
}

/// The free space on the volume holding `path`, in MiB, or why it could not be read.
///
/// ```
/// use majordomus_cli::accumulation::disk_free_mb;
/// use std::path::Path;
/// let here: u64 = disk_free_mb(Path::new(".")).expect("the volume this runs on is readable");
/// let _ = here; // whatever the machine has; the point is that it is a number
/// // a path df cannot answer for is a reason, never a zero
/// assert!(disk_free_mb(Path::new("/nonexistent/majordomus/volume")).is_err());
/// ```
pub fn disk_free_mb(path: &Path) -> Result<u64, String> {
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .map_err(|e| format!("df could not be run: {e}"))?;
    parse_df(&String::from_utf8_lossy(&out.stdout))
        .ok_or_else(|| format!("df reported no free space for {}", path.display()))
}

/// Measure the repository `root` belongs to against `policy`.
///
/// The topology is read with [`Detail::Fast`]: the registrations and the branches, no
/// subprocess per worktree. That is what lets a provider's start event ask. `local` is the
/// repository-relative path of the layer's local half, where the watch keeps its record.
///
/// ```
/// use majordomus_cli::accumulation::{measure, AccumulationPolicy, BudgetState};
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     assert!(std::process::Command::new("git")
///         .current_dir(dir.path())
///         .args(["-c", "user.email=t@example.com", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
///         .args(args)
///         .status()
///         .unwrap()
///         .success());
/// };
/// git(&["init", "-q", "-b", "main"]);
/// std::fs::write(dir.path().join("a"), "a").unwrap();
/// git(&["add", "-A"]);
/// git(&["commit", "-q", "-m", "one"]);
/// let policy = AccumulationPolicy { worktrees_max: Some(0), ..Default::default() };
/// let report = measure(dir.path(), &policy, ".ai/local").unwrap();
/// assert_eq!(report.over_ids(), ["worktrees"], "one checkout is more than none");
/// assert_eq!(report.quantities[1].state, BudgetState::Unbudgeted);
/// // a directory that is no repository is refused, not measured as empty
/// assert!(measure(tempfile::tempdir().unwrap().path(), &policy, ".ai/local").is_err());
/// ```
pub fn measure(
    root: &Path,
    policy: &AccumulationPolicy,
    local: &str,
) -> Result<AccumulationReport, WorktreeError> {
    let service = WorktreeService::open(root)?;
    let topology = service.topology(Detail::Fast)?;
    let (worktrees, merged_branches) = readings_of(&topology);
    let primary = service.identity().primary_worktree().path.clone();
    let readings = Readings {
        worktrees,
        merged_branches,
        disk_free_mb: disk_free_mb(&primary),
    };
    let mut report =
        AccumulationReport::of(&primary.to_string_lossy(), policy, judge(&readings, policy));
    report.last_reduction = last_reduction(&record_path(&primary, local));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?}");
    }

    /// The policy key of each quantity is the key a person writes: the name follows from the
    /// quantity and the side of the bound, so a report can never name a key the policy does
    /// not read.
    #[test]
    fn every_quantity_names_the_policy_key_that_budgets_it() {
        let q = judge(&Readings::unread("x"), &AccumulationPolicy::default());
        let keys: Vec<&str> = q.iter().map(|q| q.policy_key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "accumulation.worktrees_max",
                "accumulation.merged_branches_max",
                "accumulation.disk_free_min_mb"
            ]
        );
        let declared: AccumulationPolicy = serde_json::from_str(
            r#"{"worktrees_max": 1, "merged_branches_max": 2, "disk_free_min_mb": 3}"#,
        )
        .unwrap();
        assert_eq!(
            (
                declared.worktrees_max,
                declared.merged_branches_max,
                declared.disk_free_min_mb
            ),
            (Some(1), Some(2), Some(3)),
            "each key is one the policy block deserialises"
        );
        assert_eq!(BudgetBound::AtMost.as_str(), "at most");
        assert_eq!(BudgetBound::AtLeast.as_str(), "at least");
        for (state, word) in [
            (BudgetState::Ok, "ok"),
            (BudgetState::Over, "over"),
            (BudgetState::Unbudgeted, "unbudgeted"),
            (BudgetState::Unknown, "unknown"),
        ] {
            assert_eq!(state.as_str(), word);
        }
    }

    /// A line says the reading against its budget, the absence of either in words, and the
    /// remedy only where a budget is crossed.
    #[test]
    fn a_line_says_what_was_read_what_is_allowed_and_what_reduces_it() {
        let policy = AccumulationPolicy {
            disk_free_min_mb: Some(100),
            ..Default::default()
        };
        let readings = Readings {
            worktrees: Ok(4),
            merged_branches: Err("no trunk".into()),
            disk_free_mb: Ok(500),
        };
        let q = judge(&readings, &policy);
        assert_eq!(
            q[0].line(),
            "unbudgeted  worktrees        4 worktree(s), no budget (accumulation.worktrees_max is not declared)"
        );
        assert_eq!(
            q[1].line(),
            "unknown     merged_branches  not measured: no trunk, no budget (accumulation.merged_branches_max is not declared)"
        );
        assert_eq!(
            q[2].line(),
            "ok          disk_free        500 MiB free, budget at least 100"
        );
        let report = AccumulationReport::of("/r", &policy, q);
        assert!(
            report.within_budget,
            "an unmeasured quantity is not over budget"
        );
        assert_eq!(report.unknown, 1, "and it is not hidden either");
        assert!(
            report.brief().is_empty(),
            "an unbudgeted quantity that could not be read asks nothing of a briefing"
        );
        assert_eq!(
            report.summary(),
            "accumulation: 0 of 3 quantities over budget, 1 not measured; policy says report"
        );
    }

    /// A budgeted quantity that could not be measured reaches the briefing: a watch that
    /// went blind is the thing a worker must hear about.
    #[test]
    fn a_budgeted_quantity_that_went_blind_is_said() {
        let policy = AccumulationPolicy {
            disk_free_min_mb: Some(100),
            act: AccumulationAct::Reduce,
            ..Default::default()
        };
        let readings = Readings {
            worktrees: Ok(1),
            merged_branches: Ok(0),
            disk_free_mb: Err("df could not be run".into()),
        };
        let report = AccumulationReport::of("/r", &policy, judge(&readings, &policy));
        assert_eq!(report.brief().len(), 1);
        assert!(report.brief()[0].contains("not measured: df could not be run"));
        assert_eq!(
            report.summary(),
            "accumulation: 0 of 3 quantities over budget, 1 not measured; policy says reduce"
        );
        let clean = Readings {
            disk_free_mb: Ok(100),
            ..readings
        };
        assert_eq!(
            AccumulationReport::of("/r", &policy, judge(&clean, &policy)).summary(),
            "accumulation: 0 of 3 quantities over budget; policy says reduce"
        );
    }

    /// Over a real repository: the worktrees git registers and the branches the trunk
    /// reaches are counted, the trunk itself is not, and the volume is read.
    #[test]
    fn a_repository_is_measured_from_git_and_the_volume() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        std::fs::write(root.join("a"), "a\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "base"]);
        git(&root, &["branch", "landed"]);
        git(&root, &["branch", "open"]);
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                dir.path().join("wt").to_str().unwrap(),
                "open",
            ],
        );
        std::fs::write(dir.path().join("wt/b"), "b\n").unwrap();
        git(&dir.path().join("wt"), &["add", "-A"]);
        git(&dir.path().join("wt"), &["commit", "-q", "-m", "ahead"]);

        let policy = AccumulationPolicy {
            watch: true,
            worktrees_max: Some(1),
            merged_branches_max: Some(1),
            disk_free_min_mb: Some(0),
            ..Default::default()
        };
        let report = measure(&root, &policy, ".ai/local").unwrap();
        assert_eq!(
            report.last_reduction, None,
            "the watch has never acted here"
        );
        assert_eq!(report.schema, SCHEMA);
        assert!(report.watched);
        let by = |id: &str| report.quantities.iter().find(|q| q.id == id).unwrap();
        assert_eq!(
            by("worktrees").measured,
            Some(2),
            "the primary and one more"
        );
        assert_eq!(by("worktrees").state, BudgetState::Over);
        assert_eq!(
            by("merged_branches").measured,
            Some(1),
            "`landed` is reached by the trunk, `open` is ahead of it, the trunk is not counted"
        );
        assert_eq!(by("merged_branches").state, BudgetState::Ok);
        assert_eq!(by("disk_free").state, BudgetState::Ok);
        assert!(by("disk_free").measured.is_some());
        assert_eq!(report.over, 1);
        assert!(!report.within_budget);

        assert!(
            measure(dir.path(), &policy, ".ai/local").is_err(),
            "a directory that is no repository is refused, not measured as empty"
        );
    }

    /// A repository whose branches could not be compared with a trunk has an unknown count
    /// of merged ones, and one with no branch but the trunk has none.
    #[test]
    fn merged_branches_are_unknown_when_no_trunk_was_decided() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a"), "a\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "base"]);
        let service = WorktreeService::open(dir.path()).unwrap();
        let mut topology = service.topology(Detail::Fast).unwrap();
        assert_eq!(readings_of(&topology).1, Ok(0), "only the trunk exists");

        git(dir.path(), &["branch", "other"]);
        topology = service.topology(Detail::Fast).unwrap();
        assert_eq!(readings_of(&topology).1, Ok(1));
        for b in &mut topology.branches {
            b.merged_into_trunk = None;
        }
        assert_eq!(
            readings_of(&topology).1,
            Err("no branch could be compared with a trunk".to_string())
        );
    }

    /// What the watch did is written where every worktree reads it, read back as written,
    /// and reported with the next measurement; a damaged record is no record.
    #[test]
    fn a_reduction_is_recorded_and_the_next_measurement_carries_it() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a"), "a\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "base"]);
        let primary = WorktreeService::open(dir.path())
            .unwrap()
            .identity()
            .primary_worktree()
            .path
            .clone();
        let path = record_path(&primary, ".ai/local");
        assert_eq!(last_reduction(&path), None);
        let done = LastReduction {
            at: 42,
            because: vec!["worktrees".into()],
            removed: vec!["feature/x".into()],
            deleted: vec![],
            refused: 2,
        };
        record(&path, &done).unwrap();
        assert_eq!(last_reduction(&path), Some(done.clone()));
        let report = measure(dir.path(), &AccumulationPolicy::default(), ".ai/local").unwrap();
        assert_eq!(report.last_reduction, Some(done));
        assert!(report.over_ids().is_empty());

        std::fs::write(&path, "{ half a record").unwrap();
        assert_eq!(last_reduction(&path), None);
    }

    /// A path `df` cannot answer for is a reason, not a number.
    #[test]
    fn a_volume_that_cannot_be_read_is_a_reason() {
        let missing = Path::new("/nonexistent/majordomus/accumulation");
        let e = disk_free_mb(missing).unwrap_err();
        assert!(
            e.contains("no free space") || e.contains("could not be run"),
            "{e}"
        );
    }
}
