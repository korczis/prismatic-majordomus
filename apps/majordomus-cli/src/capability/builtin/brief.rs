//! The `brief` module: what surrounds one working session, in one line and in a few, so that
//! a session can report on itself without being asked (ADR 0136). Nothing here writes, and
//! nothing here builds.
//!
//! # Why one capability composes what nine owners already answer
//!
//! Every fact in the brief has an owner already: `lifecycle.episodes` and the task record the
//! session and its task, `peers.list` the board of every checkout, the mesh journal this
//! session's claims and the machines, `majordomus prs brief` the integration queue, the
//! monitor's cache of the forge the pull requests, runs and the CI pool, the operating system
//! the machine, `release.currency` the release, and the machine's derive lock
//! (`lib/machine_lock.sh`) the derive queue. A session reads none of them unless it
//! remembers to, and on 2026-10-10 the sessions of this repository relayed board state to
//! each other by hand. The brief asks each owner once and says, per section, what it
//! observed, from where, when, the age past which that is stale, a verdict word, the
//! command that refreshes it, and what asking cost; it judges nothing of its own and stores
//! nothing.
//!
//! # Who answers
//!
//! The board and the mesh live in the memory of the server that holds this checkout's lease.
//! A command-line process has an empty board of its own and no mesh, so a brief composed
//! there would report "nobody is here" when the truth is "I did not ask". The command line
//! therefore asks the leaseholder for the brief (`local: true`, so the server never asks
//! back), and composes it itself only when no server answers — saying so. The forge is read
//! only by the leaseholder, through the monitor's shared bounded cache: one call per
//! question per TTL for every session of the repository, never one per status-line tick.
//!
//! A server that lost the lease (superseded by a newer one) still holds its own MCP
//! sessions, and the repository's gather cannot see them: `peers.list` reads a checkout's
//! board from the server its lease names. Such a server composes the brief from its own
//! board, adds the leaseholder's, and says that its own is unreachable by everyone else.
//!
//! # Bounded
//!
//! Every section runs in its own thread under its own budget; a section past it is reported
//! unread, with what it was allowed, and never waited on. The only children are `git`, `gh`
//! and `ps`; the brief never runs this executable or a compiler (ADR 0224).
//!
//! # The report
//!
//! What a session writes about itself on the interval is declared once, in
//! `share/brief/report.yaml`; [`report::render`] composes `keep_awake`, `report.prompt` and
//! `report.reminder` from it and from the attention entries [`report::derive_attention`]
//! measured, and every carrier relays those texts verbatim.

mod ci;
mod machine;
pub(crate) mod refs;
pub(crate) mod report;
mod work;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::lease;
use crate::peers::{Announcement, Peer};
use crate::{capability, module};

use super::lifecycle::Episodes;
use super::peers::PeerList;
use super::server::{self, Checkouts, ServerStanding};
use super::{get, mcp};

/// The interval a policy that declares none gets, in minutes.
pub(crate) const DEFAULT_INTERVAL_MINUTES: u64 = 20;
/// How many peers the multi-line brief names before it says how many it left out.
const PEERS_NAMED: usize = 8;
/// How long the command line waits for the leaseholder's brief. A leaseholder of this
/// generation answers inside [`OVERALL_DEADLINE`] (its gather of every checkout's board,
/// measured at about 3 s over 118 checkouts, is the peers section, inside its budget), so
/// four seconds past the deadline is slack for a loaded server. Waiting longer waits only on
/// a leaseholder older than the budgets: on 2026-10-29 one took the old 20 s to fail, and
/// the composition that followed it brought the command to 24 s, past half the mod's 30 s.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest a composition waits for any section: past the longest budget, under
/// [`FORWARD_TIMEOUT`] and under the mod's own 30 s.
pub(crate) const OVERALL_DEADLINE: Duration = Duration::from_secs(6);
/// Past this, a clone's `origin/master` is called unfetched.
pub(crate) const FETCH_STALE_AFTER: Duration = Duration::from_secs(15 * 60);
/// Swap use above this share is degraded.
pub(crate) const SWAP_DEGRADED_PERCENT: u64 = 80;
/// The widest the one line may be.
pub(crate) const LINE_WIDTH: usize = 160;
/// The width the multi-line brief wraps at, and the column its text starts in.
pub(crate) const WRAP_WIDTH: usize = 100;
const LABEL_WIDTH: usize = 13;
/// The workflow whose run on master is master's verdict: the CI model's adapter
/// (`.ai/repo/ci/gates.yaml`: `.github/workflows/validate.yml` is a thin adapter over it).
pub(crate) const REQUIRED_WORKFLOW: &str = "validate";

/// Each section's budget, in the order the brief reports them.
pub(crate) const BUDGETS: &[(&str, Duration)] = &[
    ("session", Duration::from_millis(200)),
    ("work", Duration::from_secs(5)),
    ("peers", Duration::from_secs(4)),
    ("mesh", Duration::from_millis(200)),
    // reads two files and asks git one count; measured at 336-396 ms on 2026-10-29 at a load
    // average of 29, past the 300 ms first declared, so it is given a second
    ("integration", Duration::from_secs(1)),
    ("ci", Duration::from_secs(5)),
    ("machine", Duration::from_secs(1)),
    // release.currency reads tags and the trunk's history: measured at about 1 s in a server
    // under load, and past 2 s at a load average of 29 (2026-10-29), so its budget is 3 s
    ("release", Duration::from_secs(3)),
    ("derive", Duration::from_millis(100)),
];

fn budget_of(section: &str) -> Duration {
    BUDGETS
        .iter()
        .find(|(n, _)| *n == section)
        .map(|(_, d)| *d)
        .unwrap_or(OVERALL_DEADLINE)
}

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One task the provider runs in the background of the session, as its engine reports it.
pub struct BriefBackground {
    /// The provider's id for it.
    pub id: String,
    /// What it is, in the provider's words.
    pub description: String,
    /// Its state in the provider's words (`running`, `completed`, ...).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `brief.session`.
pub struct BriefInput {
    /// The provider's own session identity (Claude Code's session id): the brief names the open episode that session owns and the work it holds. Absent, the calling board peer's, then the episode the pointer resolves to, which the answer says is not necessarily this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Which checkouts the peers section covers: `repository` (every board, the default) or `this`.
    #[serde(default)]
    pub checkouts: Checkouts,
    /// Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again.
    #[serde(default)]
    pub local: bool,
    /// The previous report's instant, RFC 3339: the work section then says what was delivered since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// The provider's background tasks, when its engine exposes them; absent, the brief says the provider's background is not exposed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub background: Vec<BriefBackground>,
}

impl BenchmarkCases for BriefInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // every field filled, so that the GET's query parameters each carry an example, and
        // narrow: a benchmark asks no other checkout's server and forwards nothing
        vec![NamedCase::new(
            "default",
            BriefInput {
                session: Some("00000000-0000-0000-0000-000000000000".into()),
                checkouts: Checkouts::This,
                local: true,
                since: Some("2026-10-29T09:20:00Z".into()),
                background: vec![BriefBackground {
                    id: "b1".into(),
                    description: "cargo test --lib brief".into(),
                    status: "running".into(),
                }],
            },
        )]
    }
}

// ---------------------------------------------------------------- output

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Whether a section of the brief could be read.
pub enum BriefSectionState {
    /// Its owner answered in full.
    Read,
    /// Its owner answered, and part of what it covers could not be asked; the summary says which.
    Partial,
    /// Its owner did not answer; the summary says why.
    Unread,
    /// The repository declares nothing for it (no derive lock, a mesh switched off).
    Undeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// What a reader may take a section's facts for: a status word of the design declaration.
pub enum BriefVerdict {
    /// Its owner answered, within the bound its owner declares.
    Current,
    /// Its owner answered with a fact older than its bound, or contradicted by a newer one.
    Stale,
    /// Its owner answered part of what it covers.
    Partial,
    /// Its owner did not answer, or not within the section's budget.
    #[default]
    Unread,
    /// The repository declares nothing for it.
    Undeclared,
}

impl BriefVerdict {
    /// The word, as it serialises.
    pub fn as_str(self) -> &'static str {
        match self {
            BriefVerdict::Current => "current",
            BriefVerdict::Stale => "stale",
            BriefVerdict::Partial => "partial",
            BriefVerdict::Unread => "unread",
            BriefVerdict::Undeclared => "undeclared",
        }
    }

    fn of_state(state: BriefSectionState) -> Self {
        match state {
            BriefSectionState::Read => BriefVerdict::Current,
            BriefSectionState::Partial => BriefVerdict::Partial,
            BriefSectionState::Unread => BriefVerdict::Unread,
            BriefSectionState::Undeclared => BriefVerdict::Undeclared,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What one section observed: from where, when, how old it is against the bound its owner
/// declares, the word a reader may take it for, the command that refreshes it, and what
/// asking cost.
pub struct BriefObservation {
    /// The owner asked: a capability, a file, a command.
    pub source: String,
    /// When the fact was observed, RFC 3339; absent when it was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// How old the fact is, in seconds, when it was not read now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// The age past which the owner calls the fact stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_seconds: Option<u64>,
    /// What a reader may take it for.
    pub verdict: BriefVerdict,
    /// The exact command that refreshes or explains it; present whenever the verdict is not `current`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    /// What composing the section took, in milliseconds.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// What it was allowed, in milliseconds.
    #[serde(default)]
    pub budget_ms: u64,
}

impl Default for BriefObservation {
    /// What a section composed by a server older than this envelope carries: nothing observed.
    fn default() -> Self {
        BriefObservation {
            source: "not observed by the server that composed this brief".into(),
            observed_at: None,
            age_seconds: None,
            bound_seconds: None,
            verdict: BriefVerdict::Unread,
            remedy: Some("majordomus brief --format json".into()),
            elapsed_ms: 0,
            budget_ms: 0,
        }
    }
}

impl BriefObservation {
    /// Whether this is the default: nothing observed.
    pub fn is_default(&self) -> bool {
        *self == BriefObservation::default()
    }

    /// An observation made now and current.
    fn now() -> Self {
        BriefObservation {
            source: String::new(),
            observed_at: Some(crate::ledger::now()),
            age_seconds: None,
            bound_seconds: None,
            verdict: BriefVerdict::Current,
            remedy: None,
            elapsed_ms: 0,
            budget_ms: 0,
        }
    }

    /// An observation read now from `source`, its verdict that of `state`.
    fn read(source: &str, state: BriefSectionState, remedy: &str) -> Self {
        let verdict = BriefVerdict::of_state(state);
        BriefObservation {
            source: source.into(),
            remedy: (verdict != BriefVerdict::Current).then(|| remedy.to_string()),
            verdict,
            ..BriefObservation::now()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Which process composed the brief, and why that one.
pub struct BriefOrigin {
    /// `leaseholder` (the server this checkout's lease names), `superseded` (a server that lost the lease), or `this_process` (no server answered, or the call asked for `local`).
    pub process: String,
    /// The address of the server that composed it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Why this process and not the leaseholder, when it is not the leaseholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// What composing the whole brief took, in milliseconds.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// The section that took longest, and how long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slowest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The session the brief is about.
pub struct BriefSessionSection {
    /// Whether it could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// The open episode, when one is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    /// When that episode opened, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_started_at: Option<String>,
    /// The provider session that owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session: Option<String>,
    /// How the episode was found: `provider_session` (the caller's own) or `pointer` (the checkout's pointer, not necessarily this session's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// The checkout's branch.
    pub branch: String,
    /// The checkout's head, abbreviated.
    pub head: String,
    /// The active task's id, when one is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// The active task's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The id of the last task the checkout recorded, whatever its outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_task: Option<String>,
    /// That task's outcome: `active`, `partial`, `completed`, `handed_over`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_outcome: Option<String>,
    /// When that task started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_started_at: Option<String>,
    /// When it was last checkpointed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_checkpoint_at: Option<String>,
    /// The provider session that started it, when its record names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_owner_session: Option<String>,
    /// The caller's place on this checkout's board: the MCP session the call came through, else the board peer whose address carries the provider session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One worker of the repository, as the brief names it.
pub struct BriefPeer {
    /// Its place on its board (`p1`); never its identity across a reconnect.
    pub id: String,
    /// Its client's name.
    pub client: String,
    /// The worker's process, when its address names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The machine it runs on, when its address names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The socket other sessions message it through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// The branch it works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The checkout it is attached to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// Where the branch was read: `announce` (a `refs/heads/` scope it announced), `claim` (one of its mesh claims), `session` (its mesh session) or `server` (the checkout of the server it is attached to, the last resort).
    pub branch_source: String,
    /// What it announced, whole: the intent of the claim whose `refs/heads/` scope named the
    /// branch, so that the branch and the intent describe one piece of work; else its latest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// How many other claims it holds beside the one `intent` is, listed by `majordomus peers`.
    #[serde(default)]
    pub other_claims: usize,
    /// Whether this is the session the brief is about.
    pub you: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The workers of this repository, from every board the brief could read.
pub struct BriefPeersSection {
    /// Whether every board could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// Attached peers across the boards read, the caller included.
    pub attached: usize,
    /// How many boards the answer covers.
    pub boards: usize,
    /// Every board that could not be read, as `<worktree>: <reason>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unread: Vec<String>,
    /// Pairs of peers whose claimed scopes meet.
    pub overlaps: usize,
    /// Set when the composing server is superseded: its own board is unreachable by the repository's gather.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded: Option<String>,
    /// The leaseholder a superseded server lost the lease to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaseholder: Option<String>,
    /// The attached peers, one unclipped line each, at most a bounded number.
    pub named: Vec<String>,
    /// The attached peers, at most a bounded number, each with its branch and whole intent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<BriefPeer>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The mesh: the links of this runtime and what the journal addresses to it.
pub struct BriefMeshSection {
    /// Whether cooperation could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// Whether cooperation runs in the composing process.
    pub active: bool,
    /// Why it does not, when it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Links to other runtimes.
    pub links: usize,
    /// Of them, connected.
    pub connected: usize,
    /// Of them, degraded, unreachable or expired.
    pub failing: usize,
    /// Claims held across the mesh.
    pub claims_held: usize,
    /// Of them, held by this runtime.
    pub own_claims: usize,
    /// Open review requests that name this runtime as the reviewer.
    pub reviews_for_you: usize,
    /// Handovers another runtime published that nobody has consumed.
    pub handovers_waiting: usize,
    /// Overlaps between a claim of this runtime and another's.
    pub overlaps_for_you: usize,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One workflow run, as the forge reported it.
pub struct BriefRun {
    /// Its id.
    pub id: u64,
    /// Its workflow's name.
    pub workflow: String,
    /// The branch it ran on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it ran on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head_sha: String,
    /// `queued`, `in_progress` or `completed`.
    pub status: String,
    /// How it finished, once it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// When it was created, RFC 3339.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    /// Jobs finished, when its jobs were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs_done: Option<u32>,
    /// Jobs in all, when its jobs were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs_total: Option<u32>,
    /// Jobs still waiting for a runner, when its jobs were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs_queued: Option<u32>,
    /// Every job that failed or timed out, by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_jobs: Vec<String>,
}

impl BriefRun {
    /// A run from one entry of `gh run list --json`.
    pub(crate) fn from_gh(v: &serde_json::Value) -> BriefRun {
        let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        BriefRun {
            id: v["databaseId"].as_u64().unwrap_or_default(),
            workflow: s("workflowName"),
            branch: s("headBranch"),
            head_sha: s("headSha"),
            status: s("status"),
            conclusion: v["conclusion"]
                .as_str()
                .filter(|c| !c.is_empty())
                .map(str::to_string),
            created_at: s("createdAt"),
            ..BriefRun::default()
        }
    }

    /// Whether it finished failing, or a job of it already failed.
    pub(crate) fn is_failing(&self) -> bool {
        matches!(
            self.conclusion.as_deref(),
            Some("failure" | "timed_out" | "startup_failure")
        ) || !self.failed_jobs.is_empty()
    }

    /// Whether its jobs say more than its status: it runs, waits, or failed.
    pub(crate) fn wants_jobs(&self) -> bool {
        self.status != "completed" || self.is_failing()
    }

    /// `run 40813274272 [failing] 24/36, failed structure (timed_out), 12 [queued]`.
    pub(crate) fn words(&self) -> String {
        let word = if self.is_failing() {
            "failing"
        } else {
            match (self.status.as_str(), self.conclusion.as_deref()) {
                ("completed", Some("success")) => "passed",
                ("completed", Some("cancelled")) => "cancelled",
                ("completed", _) => "unknown",
                ("in_progress", _) => "running",
                _ => "queued",
            }
        };
        let mut s = format!("{} {} [{word}]", self.workflow, self.id);
        if let (Some(d), Some(t)) = (self.jobs_done, self.jobs_total) {
            s.push_str(&format!(" {d}/{t}"));
        }
        if !self.failed_jobs.is_empty() {
            s.push_str(&format!(", failed {}", self.failed_jobs.join(", ")));
        }
        if let Some(q) = self.jobs_queued.filter(|q| *q > 0) {
            s.push_str(&format!(", {q} [queued]"));
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A merge on master's first-parent line.
pub struct BriefMerge {
    /// The pull request it merged, when its subject names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// The merge commit, abbreviated.
    pub sha: String,
    /// When it was committed, RFC 3339.
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The integration queue, with how old its observation is and what master did since.
pub struct BriefIntegrationSection {
    /// Whether this checkout ever observed the forge.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// When the forge was observed, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// How old that observation is, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// The master commit the forge reported then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_master: Option<String>,
    /// This clone's `origin/master` now, from its ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_now: Option<String>,
    /// When this clone last fetched, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_fetched_at: Option<String>,
    /// How long ago that was, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_fetched_age_seconds: Option<u64>,
    /// How many commits master moved past the observation, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_by: Option<u64>,
    /// The latest run of the required workflow on master now, from the forge cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_ci: Option<BriefRun>,
    /// The last merge on master's first-parent line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_merge: Option<BriefMerge>,
    /// Whether the observation is past its bound or contradicted by master.
    #[serde(default)]
    pub stale: bool,
    /// The command that refreshes it, when it is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Whether the trunk's landed work is released, as `release.currency` says it (I3070).
pub struct BriefReleaseSection {
    /// `read` when it was measured, `unread` when it could not be, with the summary saying why.
    pub state: BriefSectionState,
    /// What it observed. Left out when it is the default, so that this section's own default
    /// (what an older server's answer reads as) is the value it always was.
    #[serde(default, skip_serializing_if = "BriefObservation::is_default")]
    pub observed: BriefObservation,
    /// Current, owed, overdue or unknown, when the capability answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<crate::release::currency::Stance>,
    /// Landed commits past the last release.
    #[serde(default)]
    pub landed: usize,
    /// The section in words.
    pub summary: String,
}

impl Default for BriefReleaseSection {
    /// What a brief composed by a server older than this section carries: nothing measured.
    fn default() -> Self {
        BriefReleaseSection {
            state: BriefSectionState::Unread,
            observed: BriefObservation::default(),
            stance: None,
            landed: 0,
            summary: "the server that composed this brief does not measure the release".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The machine's derive queue: its lock and its priority marker (`lib/machine_lock.sh`).
pub struct BriefDeriveSection {
    /// `undeclared` when the repository ships no derive lock.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// The lock's owner line, when the lock is held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    /// The holder's process id, when its line names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Whether that process is running on this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    /// Seconds since the lock was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_seconds: Option<u64>,
    /// The branches the priority marker names, when it is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One mesh claim this session holds.
pub struct BriefClaim {
    /// Its key on the mesh.
    pub key: String,
    /// The issue it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// What it says it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The paths and refs it claims.
    pub scope: Vec<String>,
    /// `held`, `conflicted` or `suspended`.
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One item of this session's work: a branch, its pull request, its train and its run.
pub struct BriefWorkItem {
    /// The branch.
    pub branch: String,
    /// The issue it executes, from its claim or its task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Whether origin carries the branch.
    #[serde(default)]
    pub pushed: bool,
    /// Its head: the pull request's, else origin's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Its canonical worktree, when it exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The active task in that worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The outcome of the last task recorded there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_outcome: Option<String>,
    /// Its open pull request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// The pull request's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The forge's mergeStateStatus (`CLEAN`, `BLOCKED`, `DIRTY`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_state: Option<String>,
    /// The train carrying it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train: Option<String>,
    /// Its latest run: the train's when a train carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<BriefRun>,
    /// Its status word.
    #[serde(default)]
    pub word: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A train carrying one of this session's pull requests.
pub struct BriefTrain {
    /// The pull request.
    pub pr: u64,
    /// The train's branch.
    pub branch: String,
    /// Where the membership was read.
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A local process: a build or a forge call under this session, or a builder on the machine.
pub struct BriefProcess {
    /// Its pid.
    pub pid: u32,
    /// Its command's name.
    pub command: String,
    /// How long it has run, in seconds.
    pub age_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Something this session delivered since the previous report.
pub struct BriefDelivered {
    /// `merged` (an own pull request merged into master) or `pushed` (an own branch head committed after it).
    pub kind: String,
    /// The pull request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// The branch.
    pub branch: String,
    /// The commit, abbreviated.
    pub sha: String,
    /// When, RFC 3339.
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// This session's own work: its claims, branches, worktrees, pull requests, trains, runs and
/// background processes, and what it delivered since the previous report.
pub struct BriefWorkSection {
    /// Whether it could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// Claims held.
    #[serde(default)]
    pub held: usize,
    /// Claims in conflict.
    #[serde(default)]
    pub conflicted: usize,
    /// Claims suspended.
    #[serde(default)]
    pub suspended: usize,
    /// The claims.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<BriefClaim>,
    /// One item per branch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<BriefWorkItem>,
    /// The trains carrying its pull requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trains: Vec<BriefTrain>,
    /// Builds and forge calls running under its client.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub background: Vec<BriefProcess>,
    /// The provider's own background tasks, as the caller passed them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_background: Vec<BriefBackground>,
    /// Whether the caller passed them at all.
    #[serde(default)]
    pub provider_background_exposed: bool,
    /// What it delivered since the previous report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delivered: Vec<BriefDelivered>,
    /// The section in words.
    pub summary: String,
}

impl Default for BriefWorkSection {
    /// What a brief composed by a server older than this section carries: nothing measured.
    fn default() -> Self {
        BriefWorkSection {
            state: BriefSectionState::Unread,
            observed: BriefObservation::default(),
            held: 0,
            conflicted: 0,
            suspended: 0,
            claims: Vec::new(),
            items: Vec::new(),
            trains: Vec::new(),
            background: Vec::new(),
            provider_background: Vec::new(),
            provider_background_exposed: false,
            delivered: Vec::new(),
            summary:
                "unread: the server that composed this brief does not measure this session's work"
                    .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One self-hosted runner.
pub struct BriefRunner {
    /// Its name.
    pub name: String,
    /// `online` or `offline`.
    pub status: String,
    /// Whether it runs a job.
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The CI pool: hosted runs in progress and queued, and the self-hosted runners.
pub struct BriefCiSection {
    /// Whether it could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// `owner/name` of the repository.
    #[serde(default)]
    pub repository: String,
    /// Runs in progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_progress: Option<u64>,
    /// Runs queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued: Option<u64>,
    /// Self-hosted runners online.
    #[serde(default)]
    pub online: usize,
    /// Of them, busy.
    #[serde(default)]
    pub busy: usize,
    /// Self-hosted runners offline.
    #[serde(default)]
    pub offline: usize,
    /// The runners, by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runners: Vec<BriefRunner>,
    /// Why the runners could not be read, when they could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runners_unread: Option<String>,
    /// The section in words.
    pub summary: String,
}

impl Default for BriefCiSection {
    /// What a brief composed anywhere but the leaseholder carries: the forge was not asked.
    fn default() -> Self {
        BriefCiSection {
            state: BriefSectionState::Unread,
            observed: BriefObservation {
                source: "the forge, read by the leaseholder only".into(),
                remedy: Some("majordomus serve ensure".into()),
                ..BriefObservation::default()
            },
            repository: String::new(),
            in_progress: None,
            queued: None,
            online: 0,
            busy: 0,
            offline: 0,
            runners: Vec::new(),
            runners_unread: None,
            summary: "unread: not the leaseholder (the CI pool is read by the server holding this checkout's lease)".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One volume the machine section measured.
pub struct BriefVolume {
    /// A path on it.
    pub path: String,
    /// Bytes free.
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The machine: free disk against the builder floor, swap, and builders against their limit.
pub struct BriefMachineSection {
    /// Whether it could be read.
    pub state: BriefSectionState,
    /// What it observed.
    #[serde(default)]
    pub observed: BriefObservation,
    /// Every volume measured.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<BriefVolume>,
    /// The least free of them, in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_bytes: Option<u64>,
    /// The floor no builder may start below, GiB (`machine.heavy_work.min_free_gib`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_gib: Option<u64>,
    /// Swap used, bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_used_bytes: Option<u64>,
    /// Swap in all, bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_total_bytes: Option<u64>,
    /// Builds running (cargo, or rustc with no cargo over it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builders: Option<u64>,
    /// How many the machine admits at once (`machine.heavy_work.slots`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_builders: Option<u64>,
    /// The builder processes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builder_processes: Vec<BriefProcess>,
    /// The worst word of the three: `current`, `degraded` or `blocked`.
    #[serde(default)]
    pub word: String,
    /// The section in words.
    pub summary: String,
}

impl Default for BriefMachineSection {
    /// What a brief composed by a server older than this section carries: nothing measured.
    fn default() -> Self {
        BriefMachineSection {
            state: BriefSectionState::Unread,
            observed: BriefObservation::default(),
            volumes: Vec::new(),
            free_bytes: None,
            floor_gib: None,
            swap_used_bytes: None,
            swap_total_bytes: None,
            builders: None,
            max_builders: None,
            builder_processes: Vec::new(),
            word: String::new(),
            summary: "unread: the server that composed this brief does not measure the machine"
                .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One condition the sections measured, in the words of its row of the report contract.
pub struct BriefAttention {
    /// The row's id (`integration.stale`).
    pub id: String,
    /// The section it is about.
    pub section: String,
    /// A status word of the design declaration.
    pub word: String,
    /// The fact, with what was measured.
    pub fact: String,
    /// The command that refreshes or acts on it.
    pub remedy: String,
    /// Who must act: `session`, `person` or `seat:<name>`.
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The report this brief owes, composed from the report contract (share/brief/report.yaml).
pub struct BriefReport {
    /// The contract's name and version.
    pub contract: String,
    /// The contract's fingerprint: 12 hex digits of its sha256.
    pub fingerprint: String,
    /// The first words of the prompt.
    pub marker: String,
    /// The prompt a session is woken with on the interval, verbatim.
    pub prompt: String,
    /// The text that rides on a prompt the person sends, verbatim.
    pub reminder: String,
    /// The previous report's instant, when the caller gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The answer of `brief.session`: what surrounds one session, in one line and in a few, the
/// conditions that need attention, and the report it owes.
pub struct SessionBrief {
    /// When it was composed, RFC 3339.
    pub at: String,
    /// The brief in one line: the status line's text.
    pub line: String,
    /// The brief in a few lines: the attention block, then one or more per section.
    pub lines: Vec<String>,
    /// How often a session reports on itself, in minutes (`session.brief_interval_minutes`).
    pub interval_minutes: u64,
    /// What a session is told at its start about reporting on itself, composed from the report contract (share/brief/report.yaml).
    pub keep_awake: String,
    /// Which process composed it.
    pub answered_by: BriefOrigin,
    /// Every condition the sections measured, the person's first, then worst first.
    #[serde(default)]
    pub attention: Vec<BriefAttention>,
    /// The report this brief owes, composed from the report contract (share/brief/report.yaml).
    #[serde(default)]
    pub report: BriefReport,
    /// Per section, a fingerprint of its facts with every instant and age taken out: two briefs differ in a section exactly when its fingerprint does.
    #[serde(default)]
    pub fingerprints: BTreeMap<String, String>,
    /// This session: its episode and task.
    pub session: BriefSessionSection,
    /// This session's own work.
    #[serde(default)]
    pub work: BriefWorkSection,
    /// The workers of every board.
    pub peers: BriefPeersSection,
    /// The mesh.
    pub mesh: BriefMeshSection,
    /// The integration queue.
    pub integration: BriefIntegrationSection,
    /// The CI pool.
    #[serde(default)]
    pub ci: BriefCiSection,
    /// The machine.
    #[serde(default)]
    pub machine: BriefMachineSection,
    /// Whether the trunk's landed work is released. Absent from the answer of a server older
    /// than this section, which leaves it unread rather than failing the whole brief.
    #[serde(default)]
    pub release: BriefReleaseSection,
    /// The derive queue.
    pub derive: BriefDeriveSection,
}

// ---------------------------------------------------------------- the handler

fn brief(ctx: &Context, input: BriefInput) -> Result<SessionBrief, CapabilityError> {
    let started = Instant::now();
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let (standing, _, read) = server::standing_at(&root, &server::local_half(&root));
    let doc = read.document();
    let own = doc.is_some_and(lease::is_own);
    let url = doc.and_then(|d| d.url.clone());
    let answering = matches!(standing, ServerStanding::Ready | ServerStanding::Outdated);

    // The command line (and a plain request to a server that is not the leaseholder) asks
    // the leaseholder, which holds the board and the mesh. An MCP session of a superseded
    // server is that server's own, and is answered there.
    let mut note = None;
    // the leaseholder that did not answer the brief: it still holds this checkout's board,
    // which this process's own is not
    let mut unanswered = None;
    if !own && !input.local && ctx.caller.is_none() {
        if let (true, Some(url)) = (answering, url.as_deref()) {
            match forward(ctx, url, &input) {
                Ok(b) => return Ok(finish_forwarded(b, &input)),
                Err(why) => {
                    note = Some(why);
                    unanswered = Some(url.to_string());
                }
            }
        } else {
            note = Some(format!(
                "no server answers for this checkout (its server is {}); composed by this process, whose board is empty",
                standing.as_str()
            ));
        }
    }
    let superseded = !own && ctx.caller.is_some() && doc.is_some();
    let origin = if own {
        BriefOrigin {
            process: "leaseholder".into(),
            url: url.clone(),
            note: None,
            elapsed_ms: 0,
            slowest: None,
        }
    } else if superseded {
        BriefOrigin {
            process: "superseded".into(),
            url: None,
            note: Some(format!(
                "this server does not hold the lease; the leaseholder is {}",
                url.clone().unwrap_or_else(|| "still binding".into())
            )),
            elapsed_ms: 0,
            slowest: None,
        }
    } else {
        BriefOrigin {
            process: "this_process".into(),
            url: None,
            note,
            elapsed_ms: 0,
            slowest: None,
        }
    };

    let mut sections = run_sections(
        ctx,
        &root,
        &input,
        own,
        superseded.then_some(url).flatten(),
        unanswered,
    );
    let interval_minutes = interval_minutes(&root);
    // master's run is read with the CI pool, inside its budget, and said with integration
    if let Some(run) = sections.master_run.take() {
        sections.integration.master_ci = Some(run);
        sections.integration.summary = integration_summary(&sections.integration);
    }
    let mut origin = origin;
    origin.elapsed_ms = started.elapsed().as_millis() as u64;
    origin.slowest = sections.slowest();
    Ok(compose(
        crate::ledger::now(),
        interval_minutes,
        origin,
        sections,
        input.since,
    ))
}

/// A leaseholder's answer, finished here when it is older than the report contract: its
/// attention and report are composed from its sections, so that a session is never handed
/// an empty prompt.
fn finish_forwarded(mut b: SessionBrief, input: &BriefInput) -> SessionBrief {
    if b.report.prompt.is_empty() {
        let s = Sections::of(&b);
        b = compose(
            b.at.clone(),
            b.interval_minutes,
            b.answered_by.clone(),
            s,
            input.since.clone(),
        );
    }
    b
}

/// Ask the leaseholder over the route the registry declares for this capability.
fn forward(ctx: &Context, url: &str, input: &BriefInput) -> Result<SessionBrief, String> {
    let path = ctx
        .registry
        .get("brief.session")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or_else(|| "brief.session declares no HTTP route to ask over".to_string())?;
    let mut query = format!(
        "{path}?local=true&checkouts={}",
        checkouts_word(input.checkouts)
    );
    if let Some(s) = &input.session {
        query.push_str("&session=");
        query.push_str(&percent_encode(s));
    }
    if let Some(s) = &input.since {
        query.push_str("&since=");
        query.push_str(&percent_encode(s));
    }
    if !input.background.is_empty() {
        let text = serde_json::to_string(&input.background).unwrap_or_else(|_| "[]".into());
        query.push_str("&background=");
        query.push_str(&percent_encode(&text));
    }
    let reply = crate::mcp::bridge::request(url, "GET", &query, &[], None, FORWARD_TIMEOUT)
        .map_err(|e| {
            format!("the leaseholder at {url} did not answer ({e}); composed by this process")
        })?;
    if reply.status != 200 {
        return Err(format!(
            "the leaseholder at {url} answered {} (a server older than this executable has no brief); composed by this process",
            reply.status
        ));
    }
    serde_json::from_str(&reply.body).map_err(|e| {
        format!("the leaseholder at {url} answered a brief this executable cannot read ({e}); composed by this process")
    })
}

/// The word a [`Checkouts`] is spelled with on every transport.
fn checkouts_word(c: Checkouts) -> &'static str {
    match c {
        Checkouts::Repository => "repository",
        Checkouts::This => "this",
    }
}

/// Percent-encode a query value: everything but the unreserved characters.
fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ---------------------------------------------------------------- the sections, bounded

/// Every section of one composition.
pub(crate) struct Sections {
    pub(crate) session: BriefSessionSection,
    pub(crate) work: BriefWorkSection,
    pub(crate) peers: BriefPeersSection,
    pub(crate) mesh: BriefMeshSection,
    pub(crate) integration: BriefIntegrationSection,
    pub(crate) ci: BriefCiSection,
    pub(crate) machine: BriefMachineSection,
    pub(crate) release: BriefReleaseSection,
    pub(crate) derive: BriefDeriveSection,
    /// master's run, read with the CI pool and handed to integration.
    pub(crate) master_run: Option<BriefRun>,
}

impl Sections {
    /// The sections a brief already carries.
    fn of(b: &SessionBrief) -> Sections {
        Sections {
            session: b.session.clone(),
            work: b.work.clone(),
            peers: b.peers.clone(),
            mesh: b.mesh.clone(),
            integration: b.integration.clone(),
            ci: b.ci.clone(),
            machine: b.machine.clone(),
            release: b.release.clone(),
            derive: b.derive.clone(),
            master_run: None,
        }
    }

    /// The section that took longest, as `work 1.9 s`.
    fn slowest(&self) -> Option<String> {
        [
            ("session", self.session.observed.elapsed_ms),
            ("work", self.work.observed.elapsed_ms),
            ("peers", self.peers.observed.elapsed_ms),
            ("mesh", self.mesh.observed.elapsed_ms),
            ("integration", self.integration.observed.elapsed_ms),
            ("ci", self.ci.observed.elapsed_ms),
            ("machine", self.machine.observed.elapsed_ms),
            ("release", self.release.observed.elapsed_ms),
            ("derive", self.derive.observed.elapsed_ms),
        ]
        .into_iter()
        .max_by_key(|(_, ms)| *ms)
        .map(|(name, ms)| format!("{name} {}", millis(ms)))
    }
}

/// `850 ms`, `1.9 s`.
fn millis(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}

/// A section's type, as the bounded runner needs it.
trait Section: Send + 'static {
    fn observed_mut(&mut self) -> &mut BriefObservation;
    /// The section a composition reports when its owner did not answer within `budget`.
    fn over_budget(name: &str, budget: Duration, waited: Duration) -> Self;
}

/// The unread section of an owner that ran past its budget.
fn late(name: &str, budget: Duration, waited: Duration) -> (BriefObservation, String) {
    let budget_ms = budget.as_millis() as u64;
    (
        BriefObservation {
            source: format!("{name}: its owner did not answer within its budget"),
            observed_at: None,
            age_seconds: None,
            bound_seconds: None,
            verdict: BriefVerdict::Unread,
            remedy: Some("majordomus brief --format json".into()),
            // strictly past the budget, whatever the clock's grain
            elapsed_ms: (waited.as_millis() as u64).max(budget_ms + 1),
            budget_ms,
        },
        format!("unread (took > {budget_ms} ms)"),
    )
}

macro_rules! section_impl {
    ($t:ty, $build:expr) => {
        impl Section for $t {
            fn observed_mut(&mut self) -> &mut BriefObservation {
                &mut self.observed
            }
            fn over_budget(name: &str, budget: Duration, waited: Duration) -> Self {
                let (observed, summary) = late(name, budget, waited);
                #[allow(clippy::redundant_closure_call)]
                ($build)(observed, summary)
            }
        }
    };
}

section_impl!(BriefSessionSection, |observed, summary| {
    BriefSessionSection {
        state: BriefSectionState::Unread,
        observed,
        episode: None,
        episode_started_at: None,
        provider_session: None,
        resolved_by: None,
        branch: String::new(),
        head: String::new(),
        task_id: None,
        task: None,
        last_task: None,
        task_outcome: None,
        task_started_at: None,
        task_checkpoint_at: None,
        task_owner_session: None,
        caller: None,
        summary,
    }
});
section_impl!(BriefWorkSection, |observed, summary| BriefWorkSection {
    observed,
    summary,
    ..BriefWorkSection::default()
});
section_impl!(BriefPeersSection, |observed, summary| BriefPeersSection {
    state: BriefSectionState::Unread,
    observed,
    attached: 0,
    boards: 0,
    unread: Vec::new(),
    overlaps: 0,
    superseded: None,
    leaseholder: None,
    named: Vec::new(),
    people: Vec::new(),
    summary,
});
section_impl!(BriefMeshSection, |observed, summary| BriefMeshSection {
    state: BriefSectionState::Unread,
    observed,
    active: false,
    reason: None,
    links: 0,
    connected: 0,
    failing: 0,
    claims_held: 0,
    own_claims: 0,
    reviews_for_you: 0,
    handovers_waiting: 0,
    overlaps_for_you: 0,
    summary,
});
section_impl!(BriefIntegrationSection, |observed, summary| {
    BriefIntegrationSection {
        state: BriefSectionState::Unread,
        observed,
        observed_at: None,
        age_seconds: None,
        observed_master: None,
        master_now: None,
        master_fetched_at: None,
        master_fetched_age_seconds: None,
        moved_by: None,
        master_ci: None,
        last_merge: None,
        stale: false,
        remedy: None,
        summary,
    }
});
section_impl!(BriefCiSection, |observed, summary| BriefCiSection {
    observed,
    summary,
    ..BriefCiSection::default()
});
section_impl!(BriefMachineSection, |observed, summary| {
    BriefMachineSection {
        observed,
        summary,
        ..BriefMachineSection::default()
    }
});
section_impl!(BriefReleaseSection, |observed, summary| {
    BriefReleaseSection {
        observed,
        summary,
        ..BriefReleaseSection::default()
    }
});
section_impl!(BriefDeriveSection, |observed, summary| BriefDeriveSection {
    state: BriefSectionState::Unread,
    observed,
    holder: None,
    pid: None,
    alive: None,
    held_seconds: None,
    priority: None,
    summary,
});

/// One section running in its own thread, with the instant it was started at.
struct Pending<T> {
    name: &'static str,
    /// The section and what composing it took, measured in its own thread: the sections are
    /// collected one after another, so the time a section is collected at says nothing of it.
    rx: mpsc::Receiver<(T, Duration)>,
    started: Instant,
}

/// Start `work` in a thread of its own. The thread is never joined: a section past its
/// budget is reported unread and left to finish (every child it runs is bounded on its own).
fn spawn<T: Send + 'static>(
    name: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Pending<T> {
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let spawned = std::thread::Builder::new()
        .name(format!("brief-{name}"))
        .spawn(move || {
            let section = work();
            let _ = tx.send((section, started.elapsed()));
        });
    if spawned.is_err() {
        // no thread to run it in: the receiver reads a closed channel, which is unread
    }
    Pending { name, rx, started }
}

impl<T: Section> Pending<T> {
    /// The section, or its unread form once its budget has passed.
    fn take(self) -> T {
        let budget = budget_of(self.name);
        let deadline = self.started + budget.min(OVERALL_DEADLINE);
        let left = deadline.saturating_duration_since(Instant::now());
        match self.rx.recv_timeout(left) {
            Ok((mut s, took)) => {
                let o = s.observed_mut();
                o.elapsed_ms = took.as_millis() as u64;
                o.budget_ms = budget.as_millis() as u64;
                s
            }
            Err(_) => T::over_budget(self.name, budget, self.started.elapsed()),
        }
    }
}

/// Every section, each in its own thread under its budget.
fn run_sections(
    ctx: &Context,
    root: &Path,
    input: &BriefInput,
    leaseholder: bool,
    superseded_by: Option<String>,
    unanswered: Option<String>,
) -> Sections {
    // the caller's provider session: asked for, else the calling board peer's
    let caller_peer = ctx
        .caller
        .as_ref()
        .and_then(|id| ctx.peers.list().into_iter().find(|p| &p.id == id));
    let psid: Option<String> = input.session.clone().or_else(|| {
        caller_peer
            .as_ref()
            .and_then(|p| p.address.as_ref()?.session.clone())
    });
    let board_ids: Vec<String> = ctx
        .peers
        .list()
        .into_iter()
        .filter(|p| {
            p.attached
                && (ctx.caller.as_ref() == Some(&p.id)
                    || psid.is_some()
                        && p.address.as_ref().and_then(|a| a.session.as_deref()) == psid.as_deref())
        })
        .map(|p| p.id.as_str().to_string())
        .collect();
    // the one process table, shared by the work and machine sections
    let procs = std::sync::Arc::new(machine::process_table());

    let session = {
        let (ctx, root, psid, ids) = (
            ctx.clone(),
            root.to_path_buf(),
            psid.clone(),
            board_ids.clone(),
        );
        spawn("session", move || {
            session_section(&ctx, &root, psid.as_deref(), &ids)
        })
    };
    let work = {
        let (ctx, root, psid, ids) = (
            ctx.clone(),
            root.to_path_buf(),
            psid.clone(),
            board_ids.clone(),
        );
        let (since, background, procs) =
            (input.since.clone(), input.background.clone(), procs.clone());
        spawn("work", move || {
            work::section(&work::Inputs {
                ctx: &ctx,
                root: &root,
                psid: psid.as_deref(),
                board_ids: &ids,
                leaseholder,
                since: since.as_deref(),
                background: &background,
                procs: &procs,
            })
        })
    };
    let peers = {
        let (ctx, checkouts, psid, unanswered) = (
            ctx.clone(),
            input.checkouts,
            psid.clone(),
            unanswered.clone(),
        );
        spawn("peers", move || {
            peers_section(&ctx, checkouts, superseded_by, unanswered, psid.as_deref())
        })
    };
    let mesh = {
        let (ctx, unanswered) = (ctx.clone(), unanswered.clone());
        spawn("mesh", move || mesh_section(&ctx, unanswered.as_deref()))
    };
    let integration = {
        let root = root.to_path_buf();
        spawn("integration", move || integration_section(&root))
    };
    let ci = {
        let root = root.to_path_buf();
        spawn("ci", move || {
            let pool = ci::section(&root, leaseholder);
            let master = leaseholder.then(|| master_run(&root)).flatten();
            (pool, master)
        })
    };
    let machine = {
        let (root, procs) = (root.to_path_buf(), procs.clone());
        spawn("machine", move || machine::measure(&root, &procs))
    };
    let release = {
        let ctx = ctx.clone();
        spawn("release", move || release_section(&ctx))
    };
    let derive = {
        let root = root.to_path_buf();
        spawn("derive", move || derive_section(&root))
    };

    // the CI pool and master's run travel together; unwrap them under the pool's budget
    let ci_started = ci.started;
    let ci_budget = budget_of("ci");
    let ci_left = (ci_started + ci_budget).saturating_duration_since(Instant::now());
    let (ci, master_run) = match ci.rx.recv_timeout(ci_left) {
        Ok(((mut pool, master), took)) => {
            pool.observed.elapsed_ms = took.as_millis() as u64;
            pool.observed.budget_ms = ci_budget.as_millis() as u64;
            (pool, master)
        }
        Err(_) => (
            BriefCiSection::over_budget("ci", ci_budget, ci_started.elapsed()),
            None,
        ),
    };
    let mut session: BriefSessionSection = session.take();
    let peers: BriefPeersSection = peers.take();
    name_the_caller(&mut session, &peers, psid.as_deref());
    Sections {
        session,
        work: work.take(),
        peers,
        mesh: mesh.take(),
        integration: integration.take(),
        ci,
        machine: machine.take(),
        release: release.take(),
        derive: derive.take(),
        master_run,
    }
}

/// The latest run of the required workflow on `origin/master` now.
fn master_run(root: &Path) -> Option<BriefRun> {
    let common = refs::common_dir(&refs::git_dir(root)?);
    let sha = refs::resolve(&common, "refs/remotes/origin/master")?;
    let answer = crate::monitor::github::runs_of_commit(
        root,
        REQUIRED_WORKFLOW,
        &sha,
        crate::monitor::github::BUDGET,
    )
    .ok()?;
    let first = answer.value.as_array()?.first()?.clone();
    let mut run = BriefRun::from_gh(&first);
    if run.wants_jobs() {
        if let Ok(j) =
            crate::monitor::github::run_jobs(root, run.id, crate::monitor::github::BUDGET)
        {
            work::Jobs::of(&j.value).apply(&mut run);
        }
    }
    Some(run)
}

// ---------------------------------------------------------------- session

/// The provider session the environment names, in `mj_provider_session_env`'s order: what
/// the command line asks with when the caller gives none.
pub(crate) fn provider_session_from_env(env: impl Fn(&str) -> Option<String>) -> Option<String> {
    [
        "MAJORDOMUS_PROVIDER_SESSION",
        "CLAUDE_CODE_SESSION_ID",
        "CODEX_SESSION_ID",
        "GEMINI_SESSION_ID",
    ]
    .iter()
    .find_map(|k| {
        env(k)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

fn session_section(
    ctx: &Context,
    root: &Path,
    provider_session: Option<&str>,
    board_ids: &[String],
) -> BriefSessionSection {
    let episodes: Result<Episodes, String> = ctx
        .execute("lifecycle.episodes", json!({}))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    let caller = ctx
        .caller
        .as_ref()
        .map(|p| p.as_str().to_string())
        .or_else(|| board_ids.first().cloned());
    let (episode, owner, opened, resolved_by) = match &episodes {
        Ok(e) => {
            let by_session = provider_session
                .and_then(|s| e.episodes.iter().find(|ep| ep.provider_session == s));
            let mine =
                by_session.or_else(|| e.episodes.iter().find(|ep| ep.session_id == e.current));
            (
                mine.map(|ep| ep.session_id.clone()),
                mine.map(|ep| ep.provider_session.clone())
                    .filter(|s| !s.is_empty()),
                mine.map(|ep| ep.started_at.clone())
                    .filter(|s| !s.is_empty()),
                mine.map(|_| {
                    if by_session.is_some() {
                        "provider_session"
                    } else {
                        "pointer"
                    }
                    .to_string()
                }),
            )
        }
        Err(_) => (None, None, None, None),
    };
    // the branch and head from the checkout's own refs, read now
    let (branch, head) = match refs::head(root) {
        Some((b, h)) => (
            b.unwrap_or_else(|| "DETACHED".into()),
            h.map(|h| h.chars().take(10).collect()).unwrap_or_default(),
        ),
        None => match &ctx.index.repository.git {
            crate::git::GitState::Available(i) => (
                i.branch.clone().unwrap_or_else(|| "DETACHED".into()),
                i.head
                    .clone()
                    .unwrap_or_default()
                    .chars()
                    .take(10)
                    .collect(),
            ),
            crate::git::GitState::Unavailable { .. } => (String::new(), String::new()),
        },
    };
    // the task: one file, read now. Only a running task is this session's task:
    // `current.yaml` keeps the last task after it finished or was handed over (I4293).
    let local = server::local_half(root);
    let record = root.join(&local).join("state/current.yaml");
    let task = super::continuity::read_task(&record);
    let running = task.as_ref().filter(|t| task_is_running(&t.outcome));
    let state = if episodes.is_ok() {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let mut s = caller_words(caller.as_deref(), provider_session, true);
    match (&episode, resolved_by.as_deref()) {
        (Some(ep), Some("provider_session")) => s.push_str(&format!(
            "; episode {ep}{}",
            opened
                .as_deref()
                .map(|o| format!(", opened {}", ago_words(o)))
                .unwrap_or_default()
        )),
        (Some(ep), _) => s.push_str(&format!(
            "; episode {ep}, the checkout's pointer (owner {}), not necessarily this session",
            owner
                .as_deref()
                .map(short_session)
                .unwrap_or_else(|| "unknown".into())
        )),
        (None, _) => match (&episodes, provider_session) {
            (Err(e), _) => s.push_str(&format!("; episodes unread ({e})")),
            (Ok(_), Some(p)) => s.push_str(&format!("; no open episode for provider session {p}")),
            (Ok(_), None) => s.push_str("; no open episode"),
        },
    }
    s.push_str(&format!(" on {branch} at {head}"));
    match (&task, running) {
        (Some(t), Some(_)) => s.push_str(&format!("; task {} [active]: {}", t.id, t.task)),
        (Some(t), None) => s.push_str(&format!(
            "; no active task (last {} [{}]{}, owner {}) — source current.yaml, read now",
            t.id,
            if t.outcome.is_empty() {
                "unknown"
            } else {
                t.outcome.as_str()
            },
            if t.checkpoint_at.is_empty() {
                String::new()
            } else {
                format!(", checkpointed {}", day_words(&t.checkpoint_at))
            },
            if t.provider_session.is_empty() {
                "unknown".to_string()
            } else {
                short_session(&t.provider_session)
            }
        )),
        (None, _) => s.push_str("; no task recorded — source current.yaml, read now"),
    }
    let summary = format!("[{}] {s}", BriefVerdict::of_state(state).as_str());
    BriefSessionSection {
        state,
        observed: BriefObservation::read(
            "lifecycle.episodes, git refs and current.yaml (files)",
            state,
            "majordomus lifecycle episodes",
        ),
        episode,
        episode_started_at: opened,
        provider_session: owner,
        resolved_by,
        branch,
        head,
        task_id: running.map(|t| t.id.clone()),
        task: running.map(|t| t.task.clone()),
        last_task: task.as_ref().map(|t| t.id.clone()),
        task_outcome: task
            .as_ref()
            .map(|t| t.outcome.clone())
            .filter(|o| !o.is_empty()),
        task_started_at: task
            .as_ref()
            .map(|t| t.started_at.clone())
            .filter(|o| !o.is_empty()),
        task_checkpoint_at: task
            .as_ref()
            .map(|t| t.checkpoint_at.clone())
            .filter(|o| !o.is_empty()),
        task_owner_session: task
            .as_ref()
            .map(|t| t.provider_session.clone())
            .filter(|o| !o.is_empty()),
        caller,
        summary,
    }
}

/// How the session section names its caller: `p2 = 07a5ffde (provider session)`. A caller
/// that is not on the board is said so only when the board was read (`board_read`).
fn caller_words(caller: Option<&str>, provider_session: Option<&str>, board_read: bool) -> String {
    match (caller, provider_session) {
        (Some(c), Some(p)) => format!("{c} = {} (provider session)", short_session(p)),
        (Some(c), None) => format!("{c} (no provider session named)"),
        (None, Some(p)) if board_read => format!("provider session {p} (not on this board)"),
        (None, Some(p)) => format!("provider session {p} (this checkout's board unread)"),
        (None, None) => "no provider session named and no board peer".into(),
    }
}

/// The caller, named from the peers section when the session section could not: a command
/// line composes the brief with an empty board of its own, and this checkout's board is the
/// leaseholder's, which the peers section asks for (I4390). Says when that board was unread.
fn name_the_caller(
    session: &mut BriefSessionSection,
    peers: &BriefPeersSection,
    psid: Option<&str>,
) {
    if session.caller.is_some() || psid.is_none() {
        return;
    }
    let before = caller_words(None, psid, true);
    let after = match peers.people.iter().find(|p| p.you) {
        Some(you) => {
            session.caller = Some(you.id.clone());
            caller_words(Some(&you.id), psid, true)
        }
        None if peers
            .unread
            .iter()
            .any(|u| u.starts_with(THIS_BOARD_UNREAD)) =>
        {
            caller_words(None, psid, false)
        }
        None => return,
    };
    session.summary = session.summary.replacen(&before, &after, 1);
}

/// How an unread board of this checkout is named among the peers section's unread boards.
const THIS_BOARD_UNREAD: &str = "this checkout's board";

/// The first eight characters of a provider session id.
fn short_session(s: &str) -> String {
    s.chars().take(8).collect()
}

/// `20 h ago` for an RFC 3339 instant.
fn ago_words(at: &str) -> String {
    match crate::peers::epoch_seconds(at) {
        Some(t) => format!("{} ago", report::span((now_epoch() - t).max(0) as u64)),
        None => at.to_string(),
    }
}

/// `10-27` for an RFC 3339 instant.
fn day_words(at: &str) -> String {
    at.get(5..10).unwrap_or(at).to_string()
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------- peers

fn peers_section(
    ctx: &Context,
    checkouts: Checkouts,
    leaseholder: Option<String>,
    unanswered: Option<String>,
    psid: Option<&str>,
) -> BriefPeersSection {
    // A command line whose leaseholder did not answer the brief composes it here, and this
    // process's own board is empty: this checkout's board is the leaseholder's. It is asked
    // for that board alone, under the lease's probe timeout, beside the repository's gather
    // rather than after it, so the section's budget pays for the slower of the two; when it
    // fails too, the board is unread, never "nobody is here" (I4390).
    let leaseholders_board = unanswered.map(|url| {
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("brief-board".into())
            .spawn(move || super::peers::board_of(&ctx, &url))
    });
    let list: PeerList = match ctx
        .execute(
            "peers.list",
            json!({ "checkouts": checkouts_word(checkouts) }),
        )
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    {
        Ok(l) => l,
        Err(e) => {
            let summary = format!("unread: peers.list did not answer ({e})");
            return BriefPeersSection {
                state: BriefSectionState::Unread,
                observed: BriefObservation::read(
                    "peers.list",
                    BriefSectionState::Unread,
                    "majordomus peers",
                ),
                attached: 0,
                boards: 0,
                unread: Vec::new(),
                overlaps: 0,
                superseded: None,
                leaseholder: None,
                named: Vec::new(),
                people: Vec::new(),
                summary,
            };
        }
    };
    let mut peers = list.peers;
    let mut unread: Vec<String> = list
        .boards
        .iter()
        .filter_map(|b| {
            b.reason
                .as_ref()
                .map(|r| format!("{}: {r}", b.checkout.worktree.display()))
        })
        .collect();
    let mut superseded = None;
    if let Some(url) = &leaseholder {
        let mine = peers.iter().filter(|p| p.attached).count();
        superseded = Some(format!(
            "this server is superseded by {url}: the {mine} peer(s) attached here are on a board the repository's gather cannot reach"
        ));
        match super::peers::board_of(ctx, url) {
            Ok(theirs) => {
                let stamp = peers.iter().find_map(|p| p.checkout.clone());
                peers.extend(theirs.into_iter().map(|mut p| {
                    p.checkout = stamp.clone();
                    p
                }))
            }
            Err(e) => unread.push(format!("the leaseholder's board: {e}")),
        }
    }
    if let Some(asked) = leaseholders_board {
        let stamp = list
            .boards
            .iter()
            .find(|b| b.checkout.this_checkout)
            .map(|b| b.checkout.clone());
        let answer = match asked {
            Ok(reader) => reader
                .join()
                .unwrap_or_else(|_| Err("its reader stopped before it answered".into())),
            Err(e) => Err(format!("no thread to ask it in ({e})")),
        };
        match answer {
            Ok(theirs) => peers.extend(theirs.into_iter().map(|mut p| {
                p.checkout = stamp.clone();
                p
            })),
            Err(e) => unread.push(format!("{THIS_BOARD_UNREAD}, held by the leaseholder: {e}")),
        }
    }
    let attached: Vec<&Peer> = peers.iter().filter(|p| p.attached).collect();
    let sessions = mesh_branches(ctx);
    let people: Vec<BriefPeer> = attached
        .iter()
        .take(PEERS_NAMED)
        .map(|p| brief_peer(p, psid, &sessions))
        .collect();
    let named: Vec<String> = people.iter().map(peer_text).collect();
    let state = if unread.is_empty() && superseded.is_none() {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let mut summary = format!(
        "[{}] {} attached on {} board(s), {} unread, {} overlap(s)",
        BriefVerdict::of_state(state).as_str(),
        attached.len(),
        list.covered,
        unread.len(),
        list.overlaps.len()
    );
    if let Some(s) = &superseded {
        summary.push_str(&format!("; {s}"));
    }
    if attached.len() > PEERS_NAMED {
        summary.push_str(&format!(
            "; +{} more: majordomus peers",
            attached.len() - PEERS_NAMED
        ));
    }
    BriefPeersSection {
        state,
        observed: BriefObservation::read(
            "peers.list (every board's server)",
            state,
            "majordomus peers",
        ),
        attached: attached.len(),
        // every checkout covered, listed or counted: the summary lists only the ones that
        // hold a board (peers::PeerView), and a checkout with none is still covered
        boards: list.covered,
        unread,
        overlaps: list.overlaps.len(),
        superseded,
        leaseholder,
        named,
        people,
        summary,
    }
}

/// The branch each mesh session of this runtime says it is on, by session name
/// (`board-p3`, `claude-code-<psid>`).
fn mesh_branches(ctx: &Context) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(coop) = ctx.mesh.cooperation() else {
        return out;
    };
    let runtime = ctx.mesh.cooperation_status().runtime.unwrap_or_default();
    for s in &coop.state().sessions {
        if s.runtime != runtime && !s.info.session.starts_with("claude-code-") {
            continue;
        }
        if let Some(b) = &s.info.branch {
            out.insert(s.info.session.clone(), b.clone());
        }
    }
    out
}

/// The claim a peer is named by, and the branch it names: the most recent claim whose scope
/// holds a `refs/heads/` entry, so that the branch and the intent the brief shows describe the
/// same piece of work; a peer that names no branch is named by its latest claim.
///
/// On 2026-10-29 a session holding nine claims was shown on the branch of one and with the
/// intent of another, announced a minute later by a sibling workflow.
fn naming_claim(claims: &[Announcement]) -> (Option<&Announcement>, Option<String>) {
    let branch_of = |c: &Announcement| {
        c.scope
            .iter()
            .find_map(|s| s.strip_prefix("refs/heads/").map(str::to_string))
    };
    let named = claims
        .iter()
        .filter_map(|c| branch_of(c).map(|b| (c, b)))
        .max_by(|(a, _), (b, _)| a.at.cmp(&b.at));
    match named {
        Some((c, b)) => (Some(c), Some(b)),
        None => (claims.iter().max_by(|a, b| a.at.cmp(&b.at)), None),
    }
}

/// One peer as the brief names it: its branch from what it announced, then its mesh
/// session, then, as the last resort, the checkout of the server it is attached to.
fn brief_peer(p: &Peer, psid: Option<&str>, sessions: &BTreeMap<String, String>) -> BriefPeer {
    let (claim, announced) = naming_claim(&p.claims);
    let intent = claim.or(p.announcement.as_ref()).map(|a| a.intent.clone());
    let other_claims = p.claims.len().saturating_sub(usize::from(claim.is_some()));
    let address = p.address.as_ref();
    let session_branch = address
        .and_then(|a| a.mesh_session())
        .and_then(|k| sessions.get(&k).cloned())
        .or_else(|| sessions.get(&format!("board-{}", p.id.as_str())).cloned());
    let (branch, branch_source) = match (announced, session_branch) {
        (Some(b), _) => (Some(b), "announce"),
        (None, Some(b)) => (Some(b), "session"),
        (None, None) => (p.checkout.as_ref().and_then(|c| c.branch.clone()), "server"),
    };
    BriefPeer {
        id: p.id.as_str().to_string(),
        client: p.client.name.clone(),
        pid: address.and_then(|a| a.pid),
        host: address.map(|a| a.host.clone()),
        socket: address.and_then(|a| a.socket.clone()),
        branch,
        worktree: p
            .checkout
            .as_ref()
            .map(|c| c.worktree.display().to_string()),
        branch_source: branch_source.into(),
        intent,
        other_claims,
        you: psid.is_some() && address.and_then(|a| a.session.as_deref()) == psid,
    }
}

/// `p3  claude-code pid 2210 on lundra @fix/x (announce): what it announced`, whole.
fn peer_text(p: &BriefPeer) -> String {
    let mut s = format!("{}  ", p.id);
    if p.you {
        s.push_str("you, ");
    }
    s.push_str(&p.client);
    if let Some(pid) = p.pid {
        s.push_str(&format!(" pid {pid}"));
    }
    if let Some(h) = &p.host {
        s.push_str(&format!(" on {h}"));
    }
    match (&p.branch, p.branch_source.as_str()) {
        (Some(b), "server") => s.push_str(&format!(" @{b} (server's)")),
        (Some(b), src) => s.push_str(&format!(" @{b} ({src})")),
        (None, _) => s.push_str(" @?"),
    }
    s.push_str(": ");
    s.push_str(p.intent.as_deref().unwrap_or("announced nothing"));
    if p.other_claims > 0 {
        s.push_str(&format!(
            " (+{} other claim(s): majordomus peers)",
            p.other_claims
        ));
    }
    s
}

/// Whether an overlap between two claims is something this runtime is told: one of the two
/// is its own and the other is not. An overlap among this runtime's own claims (one
/// session's subagents in several worktrees) is not news to it, and one between two other
/// runtimes is theirs (I4293).
fn overlap_is_news(own: &[&str], first: &str, second: &str) -> bool {
    own.contains(&first) != own.contains(&second)
}

/// Whether a task with this outcome is the work in hand: `active` alone. A finished task
/// (`completed`, `partial`, `blocked`, `failed`, `no_match`) and one handed over are not.
fn task_is_running(outcome: &str) -> bool {
    outcome == "active"
}

// ---------------------------------------------------------------- mesh

fn mesh_section(ctx: &Context, unanswered: Option<&str>) -> BriefMeshSection {
    use crate::mesh::cooperation::LinkState;
    use crate::mesh::state::{ClaimState, ReviewState};
    let status = ctx.mesh.cooperation_status();
    let mut section = BriefMeshSection {
        state: BriefSectionState::Undeclared,
        observed: BriefObservation::read(
            "the mesh journal (memory)",
            BriefSectionState::Undeclared,
            "majordomus mesh doctor",
        ),
        active: status.active,
        reason: status.reason.clone(),
        links: status.peers.len(),
        connected: 0,
        failing: 0,
        claims_held: 0,
        own_claims: 0,
        reviews_for_you: 0,
        handovers_waiting: 0,
        overlaps_for_you: 0,
        summary: String::new(),
    };
    let Some(cooperation) = ctx.mesh.cooperation() else {
        // the mesh runs in the leaseholder: a command line whose leaseholder did not answer
        // did not read it, which is not the same as a mesh that is off (I4390)
        if let Some(url) = unanswered {
            section.state = BriefSectionState::Unread;
            section.observed = BriefObservation::read(
                "the mesh journal (the leaseholder's memory)",
                BriefSectionState::Unread,
                "majordomus mesh state",
            );
            section.summary =
                format!("unread: the mesh runs in the leaseholder at {url}, which did not answer");
            return section;
        }
        let reason = status
            .reason
            .as_deref()
            .unwrap_or("cooperation is not running in this process");
        // the cooperation status words its own reason "not active: ..."; said once
        let reason = reason.strip_prefix("not active: ").unwrap_or(reason);
        section.summary = format!("not active here: {reason}");
        return section;
    };
    section.state = BriefSectionState::Read;
    section.observed = BriefObservation::read(
        "the mesh journal (memory)",
        BriefSectionState::Read,
        "majordomus mesh state",
    );
    section.connected = status
        .peers
        .iter()
        .filter(|p| matches!(p.state, LinkState::Connected))
        .count();
    section.failing = status
        .peers
        .iter()
        .filter(|p| {
            matches!(
                p.state,
                LinkState::Degraded | LinkState::Unreachable | LinkState::Expired
            )
        })
        .count();
    let runtime = status.runtime.clone().unwrap_or_default();
    let state = cooperation.state();
    let held: Vec<_> = state
        .claims
        .iter()
        .filter(|c| matches!(c.state, ClaimState::Held))
        .collect();
    section.claims_held = held.len();
    section.own_claims = held.iter().filter(|c| c.runtime == runtime).count();
    section.reviews_for_you = state
        .reviews
        .iter()
        .filter(|r| {
            matches!(r.state, ReviewState::Open) && r.reviewer.as_deref() == Some(runtime.as_str())
        })
        .count();
    section.handovers_waiting = state
        .handovers
        .iter()
        .filter(|h| h.runtime != runtime && h.consumed_by.is_empty())
        .count();
    let own_keys: Vec<&str> = held
        .iter()
        .filter(|c| c.runtime == runtime)
        .map(|c| c.key.as_str())
        .collect();
    section.overlaps_for_you = state
        .overlaps
        .iter()
        .filter(|o| overlap_is_news(&own_keys, &o.first, &o.second))
        .count();
    let mut s = format!(
        "{} of {} link(s) connected",
        section.connected, section.links
    );
    if section.failing > 0 {
        s.push_str(&format!(", {} failing", section.failing));
    }
    s.push_str(&format!(
        "; {} claim(s) held, {} of them this runtime's",
        section.claims_held, section.own_claims
    ));
    let addressed = section.reviews_for_you + section.handovers_waiting + section.overlaps_for_you;
    if addressed == 0 {
        s.push_str("; nothing addressed to this runtime");
    } else {
        s.push_str(&format!(
            "; for this runtime: {} review(s), {} handover(s) waiting, {} overlap(s)",
            section.reviews_for_you, section.handovers_waiting, section.overlaps_for_you
        ));
    }
    section.summary = s;
    section
}

// ---------------------------------------------------------------- integration

fn integration_section(root: &Path) -> BriefIntegrationSection {
    let Some(obs) = crate::commands::prs::brief_typed(root) else {
        return BriefIntegrationSection {
            state: BriefSectionState::Undeclared,
            observed: BriefObservation::read(
                ".ai/local/state/integration/observation.json (never written here)",
                BriefSectionState::Undeclared,
                "majordomus prs refresh",
            ),
            observed_at: None,
            age_seconds: None,
            observed_master: None,
            master_now: None,
            master_fetched_at: None,
            master_fetched_age_seconds: None,
            moved_by: None,
            master_ci: None,
            last_merge: last_merge(root),
            stale: false,
            remedy: Some("majordomus prs refresh".into()),
            summary: "this checkout never observed the forge: majordomus prs refresh".into(),
        };
    };
    let common = refs::git_dir(root).map(|g| refs::common_dir(&g));
    let master_now = common
        .as_deref()
        .and_then(|c| refs::resolve(c, &format!("refs/remotes/origin/{}", obs.base)));
    let age =
        crate::peers::epoch_seconds(&obs.observed_at).map(|t| (now_epoch() - t).max(0) as u64);
    let moved = match (&obs.observed_master, &master_now) {
        (Some(o), Some(n)) if o != n => {
            work::git_bounded(root, &["rev-list", "--count", &format!("{o}..{n}")])
                .and_then(|t| t.trim().parse::<u64>().ok())
        }
        _ => None,
    };
    let differs = matches!((&obs.observed_master, &master_now), (Some(o), Some(n)) if o != n);
    let bound = crate::integration::OBSERVATION_STALE_AFTER.as_secs();
    let stale = age.is_some_and(|a| a > bound) || differs;
    let fetched_age = refs::fetched_seconds_ago(root);
    let mut remedy = stale.then(|| "majordomus prs refresh".to_string());
    if fetched_age.is_some_and(|a| a > FETCH_STALE_AFTER.as_secs()) {
        remedy = Some(match remedy {
            Some(r) => format!("git fetch origin master && {r}"),
            None => "git fetch origin master".into(),
        });
    }
    let mut section = BriefIntegrationSection {
        state: BriefSectionState::Read,
        observed: BriefObservation {
            source:
                ".ai/local/state/integration/observation.json and the queue's summary.json (files)"
                    .into(),
            observed_at: Some(obs.observed_at.clone()),
            age_seconds: age,
            bound_seconds: Some(bound),
            verdict: if stale {
                BriefVerdict::Stale
            } else {
                BriefVerdict::Current
            },
            remedy: remedy.clone(),
            elapsed_ms: 0,
            budget_ms: 0,
        },
        observed_at: Some(obs.observed_at.clone()),
        age_seconds: age,
        observed_master: obs.observed_master.clone(),
        master_now,
        master_fetched_at: refs::fetched_at(root),
        master_fetched_age_seconds: fetched_age,
        moved_by: moved,
        master_ci: None,
        last_merge: last_merge(root),
        stale,
        remedy,
        summary: String::new(),
    };
    section.summary = integration_words(&section, &obs);
    section
}

/// The integration section in words, once master's run is known.
fn integration_summary(s: &BriefIntegrationSection) -> String {
    let mut words = s.summary.clone();
    if let Some(run) = &s.master_ci {
        let marker = " | master run ";
        if let Some(at) = words.find(marker) {
            words.truncate(at);
        }
        words.push_str(&format!("{marker}{}", run.words()));
    }
    words
}

fn integration_words(
    s: &BriefIntegrationSection,
    obs: &crate::commands::prs::IntegrationBrief,
) -> String {
    let short = |x: &Option<String>| {
        x.as_deref()
            .map(|v| v.chars().take(10).collect::<String>())
            .unwrap_or_else(|| "?".into())
    };
    let mut w = if s.stale {
        format!(
            "[stale] queue observed {} ago ({}) at {}; origin/master {}{}",
            s.age_seconds
                .map(report::span)
                .unwrap_or_else(|| "?".into()),
            obs.observed_at,
            short(&s.observed_master),
            short(&s.master_now),
            match s.moved_by {
                Some(n) => format!(" (+{n})"),
                None => String::new(),
            }
        )
    } else {
        format!(
            "[current] queue observed {} ago at {}",
            s.age_seconds
                .map(report::span)
                .unwrap_or_else(|| "?".into()),
            short(&s.observed_master)
        )
    };
    if let Some(a) = s.master_fetched_age_seconds {
        w.push_str(&format!(", fetched {} ago", report::span(a)));
    }
    if let Some(r) = &s.remedy {
        w.push_str(&format!(" — {r}"));
    }
    // the counts are the forge's at that observation, and are said as such
    w.push_str(&format!("; {} (as observed)", obs.counts));
    if let Some(m) = &s.last_merge {
        w.push_str(&format!(
            "; last merge on master {}{} at {}",
            m.pr.map(|n| format!("#{n} ")).unwrap_or_default(),
            m.sha,
            m.at
        ));
    }
    if let Some(e) = &obs.last_executor_merge {
        w.push_str(&format!("; last executor merge {e}"));
    }
    if let Some(l) = &obs.lease {
        w.push_str(&format!("; {l}"));
    }
    w
}

/// The last merge on origin/master's first-parent line.
fn last_merge(root: &Path) -> Option<BriefMerge> {
    let text = work::git_bounded(
        root,
        &[
            "log",
            "-1",
            "--first-parent",
            "--format=%H%x09%cI%x09%s",
            "refs/remotes/origin/master",
        ],
    )?;
    let mut f = text.trim().splitn(3, '\t');
    let sha: String = f.next()?.chars().take(10).collect();
    let at = f.next()?.to_string();
    let subject = f.next().unwrap_or_default();
    Some(BriefMerge {
        pr: work::merged_pr(subject).map(|(n, _)| n),
        sha,
        at,
    })
}

// ---------------------------------------------------------------- release and derive

/// Whether the trunk is released (`release.currency`, I3070), without the version: a brief is
/// composed at every session's start, and the version reads the last release's registry.
fn release_section(ctx: &Context) -> BriefReleaseSection {
    let answer: Result<crate::release::currency::Currency, String> = ctx
        .execute(super::release::CURRENCY_ID, json!({ "next": false }))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    match answer {
        Ok(a) => {
            let state = match a.stance {
                crate::release::currency::Stance::Unknown => BriefSectionState::Unread,
                _ => BriefSectionState::Read,
            };
            BriefReleaseSection {
                state,
                observed: BriefObservation::read(
                    "release.currency",
                    state,
                    "majordomus release next",
                ),
                stance: Some(a.stance),
                landed: a.landed,
                summary: a.summary,
            }
        }
        Err(why) => BriefReleaseSection {
            summary: format!("release.currency did not answer: {why}"),
            observed: BriefObservation::read(
                "release.currency",
                BriefSectionState::Unread,
                "majordomus release next",
            ),
            ..BriefReleaseSection::default()
        },
    }
}

/// The derive lock and its priority marker, where `lib/machine_lock.sh` puts them: beside the
/// primary checkout, or where `MAJORDOMUS_DERIVE_LOCK` and `MAJORDOMUS_DERIVE_PRIORITY` say.
fn derive_section(root: &Path) -> BriefDeriveSection {
    let declared = root.join("lib/machine_lock.sh").is_file();
    let lock = std::env::var_os("MAJORDOMUS_DERIVE_LOCK")
        .map(PathBuf::from)
        .or_else(|| primary_home(root).map(|h| h.join(".derive.lock")));
    let priority = std::env::var_os("MAJORDOMUS_DERIVE_PRIORITY")
        .map(PathBuf::from)
        .or_else(|| {
            lock.as_ref()
                .and_then(|l| l.parent().map(|p| p.join(".derive.priority")))
        });
    derive_from(declared, lock.as_deref(), priority.as_deref())
}

/// The directory holding the primary checkout, found from the checkout's git files.
fn primary_home(root: &Path) -> Option<PathBuf> {
    work::primary_checkout(root).and_then(|p| p.parent().map(Path::to_path_buf))
}

/// The derive section from the files alone: kept apart from where they are found so that a
/// test can hand it a lock of its own.
fn derive_from(declared: bool, lock: Option<&Path>, priority: Option<&Path>) -> BriefDeriveSection {
    let mut section = BriefDeriveSection {
        state: BriefSectionState::Undeclared,
        observed: BriefObservation::read(
            "the derive lock (files)",
            BriefSectionState::Undeclared,
            "majordomus doctor",
        ),
        holder: None,
        pid: None,
        alive: None,
        held_seconds: None,
        priority: None,
        summary: "the repository declares no derive lock".into(),
    };
    if !declared {
        return section;
    }
    section.state = BriefSectionState::Read;
    section.observed = BriefObservation::read(
        "the derive lock (files)",
        BriefSectionState::Read,
        "majordomus doctor",
    );
    section.priority = priority.filter(|p| p.exists()).map(|p| {
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    });
    let owner = lock.map(|l| l.join("owner"));
    match owner.as_deref().map(std::fs::read_to_string) {
        Some(Ok(line)) => {
            let line = line.trim().to_string();
            section.pid = owner_pid(&line);
            section.alive = section.pid.map(lease::alive);
            section.held_seconds = lock
                .and_then(|l| std::fs::metadata(l).ok())
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs());
            let who = line.split_whitespace().next().unwrap_or("?").to_string();
            let mut s = format!("lock held by {who}");
            match (section.pid, section.alive) {
                (Some(pid), Some(true)) => s.push_str(&format!(" (pid {pid}, running")),
                (Some(pid), _) => s.push_str(&format!(
                    " (pid {pid}, NOT running: the next derive reclaims it"
                )),
                (None, _) => s.push_str(" (no pid in its line: nobody can check it"),
            }
            if let Some(secs) = section.held_seconds {
                s.push_str(&format!(", {} min", secs / 60));
            }
            s.push(')');
            section.holder = Some(line);
            section.summary = s;
        }
        _ => section.summary = "lock free".into(),
    }
    if let Some(p) = &section.priority {
        section.summary.push_str(&format!(
            "; priority to {}",
            if p.is_empty() { "?" } else { p }
        ));
    }
    section
}

/// The holder's pid: the line's `pid=<n>`, else its last word, only when it is a number —
/// the reading `lib/machine_lock.sh` gives the same line.
fn owner_pid(line: &str) -> Option<u32> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix("pid=").and_then(|n| n.parse().ok()))
        .or_else(|| line.split_whitespace().last().and_then(|w| w.parse().ok()))
}

/// `session.brief_interval_minutes`, or the default when the policy declares none or
/// cannot be read.
fn interval_minutes(root: &Path) -> u64 {
    crate::repository::Repository::open(root)
        .ok()
        .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
        .and_then(|p| p.policy.session.brief_interval_minutes)
        .filter(|m| *m > 0)
        .unwrap_or(DEFAULT_INTERVAL_MINUTES)
}

// ---------------------------------------------------------------- composing

/// The brief from its sections: the attention, the line, the lines, the fingerprints and the
/// three texts of the report contract.
fn compose(
    at: String,
    interval_minutes: u64,
    answered_by: BriefOrigin,
    s: Sections,
    since: Option<String>,
) -> SessionBrief {
    let attention = report::derive_attention(&report::Measured {
        session: &s.session,
        work: &s.work,
        peers: &s.peers,
        mesh: &s.mesh,
        integration: &s.integration,
        ci: &s.ci,
        machine: &s.machine,
        release: &s.release,
        derive: &s.derive,
    });
    let line = one_line(&s, &attention);
    let lines = brief_lines(&s, &attention, &answered_by);
    let mut b = SessionBrief {
        at,
        line,
        lines,
        interval_minutes,
        keep_awake: String::new(),
        answered_by,
        attention,
        report: BriefReport {
            contract: report::contract_name(),
            fingerprint: report::fingerprint().into(),
            marker: report::contract().marker.clone(),
            prompt: String::new(),
            reminder: String::new(),
            since,
        },
        fingerprints: BTreeMap::new(),
        session: s.session,
        work: s.work,
        peers: s.peers,
        mesh: s.mesh,
        integration: s.integration,
        ci: s.ci,
        machine: s.machine,
        release: s.release,
        derive: s.derive,
    };
    b.fingerprints = fingerprints(&b);
    b.keep_awake = report::render(report::Use::Start, &b);
    b.report.prompt = report::render(report::Use::Interval, &b);
    b.report.reminder = report::render(report::Use::Reminder, &b);
    b
}

/// One fingerprint per section, of its facts with every instant, age and cost taken out,
/// and the attention entries about it: what the mod compares to decide that a fact changed.
fn fingerprints(b: &SessionBrief) -> BTreeMap<String, String> {
    let by = report::by_section(&b.attention);
    let section = |name: &str, v: serde_json::Value| {
        let mut v = v;
        clock_free(&mut v);
        let attention: Vec<&str> = by
            .get(name)
            .map(|es| es.iter().map(|e| e.id.as_str()).collect())
            .unwrap_or_default();
        let text = format!("{v}{attention:?}");
        (name.to_string(), report::fingerprint_of(&text))
    };
    let j = |v: Result<serde_json::Value, serde_json::Error>| v.unwrap_or_default();
    [
        section("session", j(serde_json::to_value(&b.session))),
        section("work", j(serde_json::to_value(&b.work))),
        section("peers", j(serde_json::to_value(&b.peers))),
        section("mesh", j(serde_json::to_value(&b.mesh))),
        section("integration", j(serde_json::to_value(&b.integration))),
        section("ci", j(serde_json::to_value(&b.ci))),
        section("machine", j(serde_json::to_value(&b.machine))),
        section("release", j(serde_json::to_value(&b.release))),
        section("derive", j(serde_json::to_value(&b.derive))),
    ]
    .into_iter()
    .collect()
}

/// The keys that carry the clock rather than a fact: instants, ages, costs, and the words
/// those are spelled in.
const CLOCK_KEYS: &[&str] = &[
    "observed",
    "summary",
    "observed_at",
    "age_seconds",
    "held_seconds",
    "master_fetched_at",
    "master_fetched_age_seconds",
    "episode_started_at",
    "task_checkpoint_at",
    "elapsed_ms",
    "free_bytes",
    "swap_used_bytes",
    "named",
    "volumes",
];

fn clock_free(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            for k in CLOCK_KEYS {
                m.remove(*k);
            }
            for (_, x) in m.iter_mut() {
                clock_free(x);
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(clock_free),
        _ => {}
    }
}

/// Text wrapped at [`WRAP_WIDTH`] after a label column, every word kept.
fn wrapped(label: &str, text: &str) -> Vec<String> {
    let indent = " ".repeat(LABEL_WIDTH);
    let mut out = Vec::new();
    let mut current = format!("{label:<width$}", width = LABEL_WIDTH);
    let mut empty = true;
    // split on single spaces so that a deliberate double space (`p2  claude-code`) survives
    for word in text.split(' ').filter(|w| !w.contains('\n')) {
        let fits = current.chars().count() + 1 + word.chars().count() <= WRAP_WIDTH;
        if !empty && !fits {
            out.push(std::mem::replace(&mut current, indent.clone()));
            empty = true;
        }
        if !empty {
            current.push(' ');
        }
        current.push_str(word);
        empty = false;
    }
    out.push(current.trim_end().to_string());
    out
}

/// A section's summary led by its status word, as every section's line is
/// (`[owed] 2 landed commit(s) ...`): an owner whose own sentence carries no word (the release
/// currency, the derive lock, the mesh) gets the section's. A summary that already leads with
/// a bracketed word, or with `unread`, is left as it is.
fn worded(word: &str, summary: &str) -> String {
    if summary.starts_with('[') || summary.starts_with("unread") {
        summary.to_string()
    } else {
        format!("[{word}] {summary}")
    }
}

/// The release's status word: its stance (`current`, `owed`, `overdue`), `unread` otherwise.
fn release_word(r: &BriefReleaseSection) -> &'static str {
    use crate::release::currency::Stance;
    match (r.state, r.stance) {
        (BriefSectionState::Read, Some(Stance::Owed)) => "owed",
        (BriefSectionState::Read, Some(Stance::Overdue)) => "overdue",
        (BriefSectionState::Read, _) => "current",
        _ => "unread",
    }
}

/// The queue's status word: its observation's, except that a checkout that never observed
/// the forge has the queue `unread` rather than undeclared, since `prs refresh` reads it.
fn integration_word(i: &BriefIntegrationSection) -> &'static str {
    match i.state {
        BriefSectionState::Undeclared => "unread",
        _ => i.observed.verdict.as_str(),
    }
}

/// The multi-line brief: the attention block, then every section in the contract's order,
/// wrapped and never clipped.
fn brief_lines(s: &Sections, attention: &[BriefAttention], origin: &BriefOrigin) -> Vec<String> {
    let mut lines = Vec::new();
    if attention.is_empty() {
        lines.extend(wrapped("attention", "none measured"));
    } else {
        for (i, a) in attention.iter().enumerate() {
            let text = format!(
                "[{}] {}: {} — {} ({})",
                a.word, a.section, a.fact, a.remedy, a.actor
            );
            lines.extend(wrapped(if i == 0 { "attention" } else { "" }, &text));
        }
    }
    lines.extend(wrapped(
        "session",
        &worded(s.session.observed.verdict.as_str(), &s.session.summary),
    ));
    lines.extend(wrapped(
        "work",
        &worded(s.work.observed.verdict.as_str(), &s.work.summary),
    ));
    for item in &s.work.items {
        lines.extend(wrapped("", &work_item_words(item)));
    }
    if !s.work.background.is_empty() || s.work.state != BriefSectionState::Unread {
        let mut bg: Vec<String> = s
            .work
            .background
            .iter()
            .map(|p| {
                format!(
                    "{} (pid {}, {})",
                    p.command,
                    p.pid,
                    report::span(p.age_seconds)
                )
            })
            .collect();
        if bg.is_empty() {
            bg.push("none".into());
        }
        let provider = if s.work.provider_background_exposed {
            s.work
                .provider_background
                .iter()
                .map(|t| {
                    format!(
                        "{} {} [{}]",
                        t.id,
                        t.description,
                        if t.status.is_empty() {
                            "unknown"
                        } else {
                            &t.status
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "not exposed".into()
        };
        lines.extend(wrapped(
            "",
            &format!(
                "background  {}; provider background: {provider}",
                bg.join(", ")
            ),
        ));
    }
    for d in &s.work.delivered {
        lines.extend(wrapped(
            "",
            &format!(
                "delivered [{}] {}{} {} at {}",
                d.kind,
                d.pr.map(|n| format!("#{n} ")).unwrap_or_default(),
                d.branch,
                d.sha,
                d.at
            ),
        ));
    }
    lines.extend(wrapped(
        "peers",
        &worded(s.peers.observed.verdict.as_str(), &s.peers.summary),
    ));
    for n in &s.peers.named {
        lines.extend(wrapped("", n));
    }
    lines.extend(wrapped(
        "mesh",
        &worded(s.mesh.observed.verdict.as_str(), &s.mesh.summary),
    ));
    lines.extend(wrapped(
        "integration",
        &worded(integration_word(&s.integration), &s.integration.summary),
    ));
    for t in &s.work.trains {
        lines.extend(wrapped(
            "",
            &format!("train {} carries #{} (from {})", t.branch, t.pr, t.source),
        ));
    }
    lines.extend(wrapped(
        "ci",
        &worded(s.ci.observed.verdict.as_str(), &s.ci.summary),
    ));
    lines.extend(wrapped(
        "machine",
        &worded(s.machine.observed.verdict.as_str(), &s.machine.summary),
    ));
    lines.extend(wrapped(
        "release",
        &worded(release_word(&s.release), &s.release.summary),
    ));
    lines.extend(wrapped(
        "derive",
        &worded(s.derive.observed.verdict.as_str(), &s.derive.summary),
    ));
    let mut answered = format!("{} ", origin.process);
    if let Some(u) = &origin.url {
        answered.push_str(&format!("{u} "));
    }
    if let Some(n) = &origin.note {
        answered.push_str(&format!("({n}) "));
    }
    let late: Vec<&str> = [
        ("session", &s.session.observed),
        ("work", &s.work.observed),
        ("peers", &s.peers.observed),
        ("mesh", &s.mesh.observed),
        ("integration", &s.integration.observed),
        ("ci", &s.ci.observed),
        ("machine", &s.machine.observed),
        ("release", &s.release.observed),
        ("derive", &s.derive.observed),
    ]
    .into_iter()
    .filter(|(_, o)| o.budget_ms > 0 && o.elapsed_ms > o.budget_ms)
    .map(|(n, _)| n)
    .collect();
    if late.is_empty() {
        answered.push_str("(every section inside its budget");
    } else {
        answered.push_str(&format!("(past its budget: {}", late.join(", ")));
    }
    if let Some(slow) = &origin.slowest {
        answered.push_str(&format!("; slowest {slow}"));
    }
    answered.push(')');
    lines.extend(wrapped("answered by", &answered));
    lines
}

/// One item of work in words: `I4334  #2656 docs/x @ 8afdc0c934 [dirty] ...`.
fn work_item_words(i: &BriefWorkItem) -> String {
    let mut s = String::new();
    if let Some(issue) = &i.issue {
        s.push_str(&format!("{issue}  "));
    }
    if let Some(pr) = i.pr {
        s.push_str(&format!("#{pr} "));
    }
    s.push_str(&i.branch);
    if let Some(h) = &i.head {
        s.push_str(&format!(" @ {}", h.chars().take(10).collect::<String>()));
    }
    s.push_str(&format!(
        " [{}]",
        if i.word.is_empty() {
            "unknown"
        } else {
            &i.word
        }
    ));
    match (&i.pr, &i.merge_state) {
        (Some(_), Some(m)) => s.push_str(&format!(" mergeStateStatus {m}")),
        (None, _) if i.pushed => s.push_str(" no PR"),
        (None, _) => s.push_str(" not pushed"),
        _ => {}
    }
    if let Some(t) = &i.train {
        s.push_str(&format!("; in {t}"));
    }
    if let Some(r) = &i.run {
        s.push_str(&format!("; run {}", r.words()));
    }
    if let Some(w) = &i.worktree {
        s.push_str(&format!(
            "; worktree {w}, task [{}]",
            match (&i.task, &i.task_outcome) {
                (Some(_), _) => "active",
                (None, Some(o)) => o.as_str(),
                (None, None) => "none",
            }
        ));
    }
    s
}

/// The one line: the caller, the worst attention entry, then one token per section, at most
/// [`LINE_WIDTH`] characters. Tokens past the width are dropped from the right and counted.
fn one_line(s: &Sections, attention: &[BriefAttention]) -> String {
    let session = &s.session;
    let who = match (&session.task_id, &session.episode) {
        (Some(t), _) => task_label(t, session.task.as_deref().unwrap_or_default()),
        (None, Some(e)) => e.clone(),
        (None, None) => "no task".into(),
    };
    let caller = session
        .caller
        .clone()
        .map(|c| format!("{c} "))
        .unwrap_or_default();
    let first = if session.state == BriefSectionState::Unread {
        format!("{caller}session ?")
    } else {
        let prs = s.work.items.iter().filter(|i| i.pr.is_some()).count();
        format!(
            "{caller}{who} @{}{}",
            session.branch,
            if prs > 0 {
                format!(" +{prs}PR")
            } else {
                String::new()
            }
        )
    };
    let mut tokens = vec![first];
    // the worst entry names its section, as `release owed 1 landed commit(s) past ...`: the
    // section's own token keeps its place further right, and is the first a full line drops,
    // so the lead must say on its own what it is about
    if let Some(worst) = attention.iter().find(|a| a.id != "session.no_task") {
        // a fact that already starts with its section ("peers took > 4000 ms") says it once
        let fact = worst
            .fact
            .strip_prefix(&format!("{} ", worst.section))
            .unwrap_or(&worst.fact);
        tokens.push(format!(
            "{} {} {}",
            worst.section,
            worst.word,
            clip(fact, 48)
        ));
    }
    // the work's facts are in the attention entries and the first token; the line says it
    // only when it could not be read
    if s.work.state == BriefSectionState::Unread {
        tokens.push("work unread".into());
    }
    tokens.push(ci_token(&s.ci));
    tokens.push(prs_token(&s.integration));
    tokens.push(machine_token(&s.machine));
    tokens.push(match s.mesh.state {
        BriefSectionState::Read => {
            let mut m = format!("mesh {}/{}", s.mesh.connected, s.mesh.links);
            let addressed =
                s.mesh.reviews_for_you + s.mesh.handovers_waiting + s.mesh.overlaps_for_you;
            if addressed > 0 {
                m.push_str(&format!(" +{addressed} for you"));
            }
            m
        }
        BriefSectionState::Unread => "mesh unread".into(),
        _ => "mesh off".into(),
    });
    tokens.push(match s.peers.state {
        BriefSectionState::Unread => "peers unread".to_string(),
        _ => {
            let mut p = format!("peers {}/{}b", s.peers.attached, s.peers.boards);
            if !s.peers.unread.is_empty() {
                p.push_str(&format!(" ({} unread)", s.peers.unread.len()));
            }
            if s.peers.superseded.is_some() {
                p.push_str(" superseded");
            }
            p
        }
    });
    tokens.push(release_token(&s.release));
    match (&s.derive.state, &s.derive.holder) {
        (BriefSectionState::Undeclared, _) => {}
        (BriefSectionState::Unread, _) => tokens.push("derive unread".into()),
        (_, None) => tokens.push("derive free".into()),
        (_, Some(h)) => tokens.push(format!(
            "derive {}{}",
            clip(h.split_whitespace().next().unwrap_or("?"), 28),
            s.derive
                .held_seconds
                .map(|x| format!(" {}m", x / 60))
                .unwrap_or_default()
        )),
    }
    fit(tokens, LINE_WIDTH)
}

/// Tokens joined by ` | `, at most `width` characters; what does not fit is dropped from the
/// right and counted as `+N`.
fn fit(tokens: Vec<String>, width: usize) -> String {
    let all = tokens.len();
    for keep in (1..=all).rev() {
        let mut line = tokens[..keep].join(" | ");
        if keep < all {
            line.push_str(&format!(" | +{}", all - keep));
        }
        if line.chars().count() <= width {
            return line;
        }
    }
    clip(&tokens[0], width)
}

fn release_token(r: &BriefReleaseSection) -> String {
    use crate::release::currency::Stance;
    match (r.state, r.stance) {
        (BriefSectionState::Read, Some(Stance::Owed)) => format!("release owed {}", r.landed),
        (BriefSectionState::Read, Some(Stance::Overdue)) => format!("release overdue {}", r.landed),
        (BriefSectionState::Read, _) => "release current".into(),
        _ => "release unread".into(),
    }
}

fn ci_token(c: &BriefCiSection) -> String {
    match (c.in_progress, c.queued) {
        (None, None) => "ci unread".into(),
        (r, q) => {
            let n = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or_else(|| "?".into());
            let mut t = format!("ci {} run {} queued", n(r), n(q));
            if c.runners_unread.is_none() && !c.runners.is_empty() {
                t.push_str(&format!(" sh {}/{}", c.busy, c.online));
            }
            t
        }
    }
}

fn prs_token(i: &BriefIntegrationSection) -> String {
    match (i.state, i.stale) {
        (BriefSectionState::Undeclared, _) => "prs never observed".into(),
        (BriefSectionState::Unread, _) => "prs unread".into(),
        (_, true) => format!(
            "prs stale {}",
            i.age_seconds.map(age_token).unwrap_or_else(|| "?".into())
        ),
        (_, false) => format!(
            "prs current {}",
            i.age_seconds.map(age_token).unwrap_or_else(|| "?".into())
        ),
    }
}

/// `17h`, `45m`, `3d`, `20s`.
fn age_token(s: u64) -> String {
    match s {
        0..=119 => format!("{s}s"),
        120..=7199 => format!("{}m", s / 60),
        7200..=172_799 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

fn machine_token(m: &BriefMachineSection) -> String {
    if m.state == BriefSectionState::Unread {
        return "machine unread".into();
    }
    let gib = |b: u64| b / (1 << 30);
    let mut t = match (m.free_bytes, m.floor_gib) {
        (Some(f), Some(fl)) => format!("disk {}/{fl}G", gib(f)),
        (Some(f), None) => format!("disk {}G", gib(f)),
        _ => "disk ?".into(),
    };
    if let (Some(u), Some(tot)) = (m.swap_used_bytes, m.swap_total_bytes) {
        if let Some(pct) = u.saturating_mul(100).checked_div(tot) {
            t.push_str(&format!(" swap {pct}%"));
        }
    }
    if let (Some(n), Some(max)) = (m.builders, m.max_builders) {
        t.push_str(&format!(" b {n}/{max}"));
    }
    t
}

/// A task as the line names it: the issue its words open with (`I2399: ...`), else its id.
fn task_label(id: &str, words: &str) -> String {
    let first = words.split([':', ' ']).next().unwrap_or_default();
    let is_issue =
        first.len() > 1 && first.starts_with('I') && first[1..].chars().all(|c| c.is_ascii_digit());
    if is_issue {
        first.to_string()
    } else {
        id.to_string()
    }
}

/// At most `n` characters, cut at a word boundary, with an ellipsis when something was cut:
/// for the one line alone, whose width is bounded.
fn clip(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        return s;
    }
    let cut: String = s.chars().take(n.saturating_sub(1)).collect();
    // the cut is at a word boundary when the next character is a space; else back to the last
    let next_is_space = s.chars().nth(n.saturating_sub(1)) == Some(' ');
    let at_word = match cut.rfind(' ') {
        _ if next_is_space => cut,
        Some(i) if i > n / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", at_word.trim_end())
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "brief",
        title: "The session brief",
        description: "What surrounds one working session, in one line and in a few: its episode and task, its own work (claims, branches, worktrees, pull requests, trains, runs, background processes), the peers of every board, the mesh, the integration queue, the CI pool, the machine, the release and the derive queue, each section asked of its owner once under its own budget and saying what it observed, from where, how old it is against its bound, a verdict word and the command that refreshes it; the conditions that need attention, derived from those sections; and the report the session owes, composed from the report contract share/brief/report.yaml (ADR 0136). Writes nothing and builds nothing.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "brief.session",
                title: "The session brief",
                description: "One line and a few, in the order session, work, peers, mesh, integration, ci, machine, release, derive, led by the attention entries the sections measured. The session is the caller's (the provider session given, else the calling board peer's, else the checkout's pointer, said as such); work is this session's own claims, branches and their canonical worktrees, pull requests, trains, latest runs with their jobs and background processes; integration says how old the queue observation is and whether origin/master moved past it; ci and the forge parts of work are read by the leaseholder alone through the monitor's shared cache; machine is free disk, swap and builders against machine.heavy_work. Every section carries its observation (source, age, bound, verdict, remedy, cost) and runs under its own budget, a section past it reading unread. Composed by the server holding this checkout's lease: the command line asks it and composes the brief itself only when none answers, saying so. Carries the reporting interval (`session.brief_interval_minutes`), per-section fingerprints of its clock-free facts, and the three texts composed from the report contract share/brief/report.yaml: keep_awake, report.prompt and report.reminder. Writes nothing.",
                input: BriefInput,
                output: SessionBrief,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_brief"),
                    http: get("/api/v1/brief"),
                    cli: Some(CliExposure { path: vec!["brief".into()] }),
                },
                tags: ["session", "peers", "coordination", "brief"],
                // every source carries its own cache policy, and the board is live memory
                cache: CachePolicy::Disabled,
                handler: brief,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::currency::Stance;

    // ------------------------------------------------ sections a test writes

    fn session(task: Option<&str>) -> BriefSessionSection {
        BriefSessionSection {
            state: BriefSectionState::Read,
            observed: BriefObservation::now(),
            episode: Some("s-1".into()),
            episode_started_at: None,
            provider_session: None,
            resolved_by: Some("provider_session".into()),
            branch: "feature/x".into(),
            head: "abc1234567".into(),
            task_id: task.map(str::to_string),
            task: task.map(|_| "do the thing".to_string()),
            last_task: task.map(str::to_string),
            task_outcome: task.map(|_| "active".to_string()),
            task_started_at: None,
            task_checkpoint_at: None,
            task_owner_session: None,
            caller: Some("p2".into()),
            summary:
                "[current] p2 = 07a5ffde (provider session); episode s-1 on feature/x at abc1234567"
                    .into(),
        }
    }

    fn peers(unread: usize, superseded: bool) -> BriefPeersSection {
        BriefPeersSection {
            state: if unread > 0 || superseded {
                BriefSectionState::Partial
            } else {
                BriefSectionState::Read
            },
            observed: BriefObservation::now(),
            attached: 3,
            boards: 5,
            unread: (0..unread)
                .map(|i| format!("/wt/{i}: did not answer"))
                .collect(),
            overlaps: 0,
            superseded: superseded.then(|| "this server is superseded by x".to_string()),
            leaseholder: superseded.then(|| "http://127.0.0.1:9".to_string()),
            named: vec!["p1  claude-code @master (server's): integrating".into()],
            people: Vec::new(),
            summary: "[current] 3 attached on 5 board(s), 0 unread, 0 overlap(s)".into(),
        }
    }

    fn mesh_off() -> BriefMeshSection {
        BriefMeshSection {
            state: BriefSectionState::Undeclared,
            observed: BriefObservation::now(),
            active: false,
            reason: Some("disabled".into()),
            links: 0,
            connected: 0,
            failing: 0,
            claims_held: 0,
            own_claims: 0,
            reviews_for_you: 0,
            handovers_waiting: 0,
            overlaps_for_you: 0,
            summary: "not active here: disabled".into(),
        }
    }

    fn integration_never() -> BriefIntegrationSection {
        BriefIntegrationSection::over_budget("integration", Duration::ZERO, Duration::ZERO)
    }

    fn origin() -> BriefOrigin {
        BriefOrigin {
            process: "leaseholder".into(),
            url: Some("http://127.0.0.1:1".into()),
            note: None,
            elapsed_ms: 0,
            slowest: None,
        }
    }

    /// A release section as `release.currency` would make it.
    fn release(stance: Stance, landed: usize) -> BriefReleaseSection {
        BriefReleaseSection {
            state: BriefSectionState::Read,
            observed: BriefObservation::now(),
            stance: Some(stance),
            landed,
            summary: format!("{landed} landed commit(s) wait past v0.1.0"),
        }
    }

    fn quiet_machine() -> BriefMachineSection {
        machine::section(
            vec![BriefVolume {
                path: "/".into(),
                free_bytes: 41 << 30,
            }],
            Some((1 << 30, 10 << 30)),
            &Ok(Vec::new()),
            Some(35),
            Some(5),
        )
    }

    /// Sections in which nothing needs attention but the absent task.
    fn calm() -> Sections {
        Sections {
            session: session(Some("I2399")),
            work: BriefWorkSection {
                state: BriefSectionState::Read,
                observed: BriefObservation::now(),
                summary: "[current] 0 claim(s) held".into(),
                ..BriefWorkSection::default()
            },
            peers: peers(0, false),
            mesh: mesh_off(),
            integration: integration_never(),
            ci: ci::compose(
                Some(0),
                Some(0),
                Ok(vec![]),
                vec![],
                "o/r".into(),
                Duration::ZERO,
                Duration::ZERO,
            ),
            machine: quiet_machine(),
            release: release(Stance::Current, 0),
            derive: derive_from(false, None, None),
            master_run: None,
        }
    }

    fn brief_of(s: Sections) -> SessionBrief {
        compose("2026-10-29T09:40:12Z".into(), 20, origin(), s, None)
    }

    /// Sections in which every condition the contract declares holds.
    fn everything_wrong() -> Sections {
        let mut s = calm();
        s.session = session(None);
        s.session.observed = BriefObservation::now();
        s.session.last_task = Some("t-20261023195110-d51b".into());
        s.session.task_outcome = Some("partial".into());
        s.work.items = vec![
            BriefWorkItem {
                branch: "docs/a".into(),
                pr: Some(2656),
                merge_state: Some("DIRTY".into()),
                worktree: Some("/w/docs/a".into()),
                pushed: true,
                ..BriefWorkItem::default()
            },
            BriefWorkItem {
                branch: "fix/b".into(),
                pr: Some(2685),
                merge_state: Some("BLOCKED".into()),
                pushed: true,
                ..BriefWorkItem::default()
            },
            BriefWorkItem {
                branch: "feature/c".into(),
                pushed: true,
                head: Some("8afdc0c934aaaa".into()),
                ..BriefWorkItem::default()
            },
            BriefWorkItem {
                branch: "feature/d".into(),
                pr: Some(2678),
                train: Some("integration/train-10a-20261028".into()),
                run: Some(BriefRun {
                    id: 40813274272,
                    workflow: "validate".into(),
                    status: "in_progress".into(),
                    jobs_done: Some(24),
                    jobs_total: Some(36),
                    jobs_queued: Some(12),
                    failed_jobs: vec!["structure (timed_out)".into()],
                    ..BriefRun::default()
                }),
                pushed: true,
                ..BriefWorkItem::default()
            },
        ];
        s.peers = peers(1, true);
        s.mesh = BriefMeshSection {
            state: BriefSectionState::Read,
            links: 2,
            connected: 1,
            failing: 1,
            reviews_for_you: 1,
            handovers_waiting: 1,
            overlaps_for_you: 1,
            ..mesh_off()
        };
        s.integration = BriefIntegrationSection {
            state: BriefSectionState::Read,
            stale: true,
            age_seconds: Some(17 * 3600),
            observed_master: Some("81ef931a3e0000".into()),
            master_now: Some("5370ee63d10000".into()),
            moved_by: Some(3),
            master_fetched_age_seconds: Some(3600),
            master_ci: Some(BriefRun {
                id: 40796031054,
                workflow: "validate".into(),
                status: "completed".into(),
                conclusion: Some("failure".into()),
                ..BriefRun::default()
            }),
            ..integration_never()
        };
        s.ci = ci::compose(
            Some(1),
            Some(10),
            Err(crate::monitor::github::Unanswered::Forbidden(
                "403: Resource not accessible by integration".into(),
            )),
            vec![],
            "o/r".into(),
            Duration::ZERO,
            Duration::ZERO,
        );
        s.machine = machine::section(
            vec![BriefVolume {
                path: "/".into(),
                free_bytes: 20 << 30,
            }],
            Some((9 << 30, 10 << 30)),
            &Ok((0..5)
                .map(|i| machine::Proc {
                    pid: i,
                    ppid: 1,
                    age_seconds: 1,
                    command: "cargo".into(),
                })
                .collect()),
            Some(35),
            Some(5),
        );
        s.release = release(Stance::Owed, 12);
        s.derive = BriefDeriveSection {
            pid: Some(99_999_999),
            alive: Some(false),
            held_seconds: Some(600),
            ..derive_from(true, None, None)
        };
        // a section past its budget
        s.work.observed = late("work", Duration::from_secs(5), Duration::from_secs(6)).0;
        s
    }

    // ------------------------------------------------ the contract and its texts

    #[test]
    fn every_attention_row_has_a_condition_and_every_condition_a_row() {
        let declared: std::collections::BTreeSet<&str> = report::contract()
            .attention
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        let conditions: std::collections::BTreeSet<&str> =
            report::CONDITIONS.iter().copied().collect();
        assert_eq!(
            declared, conditions,
            "the contract's rows and the Rust conditions are one set"
        );
        // and every condition fires on what it is about: the set measured is the set declared,
        // the runner and the CI-pool rows each on a fixture of their own
        let mut emitted: std::collections::BTreeSet<String> = brief_of(everything_wrong())
            .attention
            .iter()
            .map(|a| a.id.clone())
            .collect();
        let mut idle = everything_wrong();
        idle.ci = ci::compose(
            Some(1),
            Some(10),
            Ok(vec![BriefRunner {
                name: "l1".into(),
                status: "online".into(),
                busy: false,
            }]),
            vec![],
            "o/r".into(),
            Duration::ZERO,
            Duration::ZERO,
        );
        let mut none = everything_wrong();
        none.ci = ci::compose(
            Some(1),
            Some(10),
            Ok(vec![BriefRunner {
                name: "l1".into(),
                status: "offline".into(),
                busy: false,
            }]),
            vec![],
            "o/r".into(),
            Duration::ZERO,
            Duration::ZERO,
        );
        let mut overdue = everything_wrong();
        overdue.release = release(Stance::Overdue, 30);
        for s in [idle, none, overdue] {
            emitted.extend(brief_of(s).attention.iter().map(|a| a.id.clone()));
        }
        let emitted: std::collections::BTreeSet<&str> =
            emitted.iter().map(String::as_str).collect();
        assert_eq!(
            emitted, conditions,
            "a declared row no condition fires is dead wording"
        );
    }

    #[test]
    fn every_template_is_filled() {
        for a in brief_of(everything_wrong()).attention {
            for text in [&a.fact, &a.remedy, &a.section] {
                let opens = text.matches('{').count();
                // `{owner}` and `{repo}` are gh's own placeholders, filled by gh
                let gh = text.matches("{owner}").count() + text.matches("{repo}").count();
                assert_eq!(opens, gh, "{}: an unfilled placeholder in {text}", a.id);
            }
        }
    }

    #[test]
    fn every_attention_word_is_a_filed_status_word() {
        let tokens = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/design/tokens.yaml"),
        )
        .expect("the design declaration");
        let filed: Vec<String> = tokens
            .lines()
            .skip_while(|l| !l.starts_with("  states:"))
            .skip(1)
            .take_while(|l| l.starts_with("    "))
            .flat_map(|l| {
                l.split_once('[')
                    .and_then(|(_, r)| r.split_once(']'))
                    .map(|(w, _)| {
                        w.split(',')
                            .map(|x| x.trim().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        let mut words: Vec<String> = report::contract()
            .attention
            .iter()
            .map(|r| r.word.clone())
            .collect();
        words.extend(
            [
                BriefVerdict::Current,
                BriefVerdict::Stale,
                BriefVerdict::Partial,
                BriefVerdict::Unread,
                BriefVerdict::Undeclared,
            ]
            .iter()
            .map(|v| v.as_str().to_string()),
        );
        for w in words {
            assert_eq!(
                filed.iter().filter(|f| **f == w).count(),
                1,
                "'{w}' is not filed exactly once in share/design/tokens.yaml status.states"
            );
        }
    }

    #[test]
    fn the_prompt_carries_every_attention_entry_verbatim_and_no_category_list() {
        let b = brief_of(everything_wrong());
        assert!(b.report.prompt.starts_with(&report::contract().marker));
        assert!(b.report.prompt.contains(report::fingerprint()));
        for a in &b.attention {
            let line = report::attention_line(a);
            assert!(b.report.prompt.contains(&line), "the prompt lacks {line}");
            assert!(
                b.report.reminder.contains(&line),
                "the reminder lacks {line}"
            );
        }
        for banned in [
            "two to four lines",
            "a board that could not be read, a superseded server",
            "(this session's task and state, the peers of every board",
        ] {
            for text in [&b.report.prompt, &b.report.reminder, &b.keep_awake] {
                assert!(!text.contains(banned), "a text says '{banned}'");
            }
        }
        // the stale queue, the failing run, the swap and the dirty PR, each with its remedy
        let p = &b.report.prompt;
        assert!(p.contains("[stale] integration: queue observed 17 h ago at 81ef931a3e; origin/master is 5370ee63d1 (+3) — majordomus prs refresh (session)"), "{p}");
        assert!(p.contains("[failing] work: integration/train-10a-20261028: run 40813274272 24/36, failed structure (timed_out)"), "{p}");
        assert!(
            p.contains("[degraded] machine: swap 9.0/10.0 GiB (90 %) — top -o mem"),
            "{p}"
        );
        assert!(
            p.contains(
                "[dirty] work: #2656 conflicts with master — git -C /w/docs/a merge origin/master"
            ),
            "{p}"
        );
        // a person's entries lead
        assert_eq!(b.attention[0].actor, "person", "{:?}", b.attention[0]);
    }

    #[test]
    fn no_attention_says_so_once() {
        let mut s = calm();
        s.session.task_id = Some("t-1".into());
        let b = brief_of(s);
        assert!(b.attention.is_empty(), "{:?}", b.attention);
        assert_eq!(
            b.report.prompt.matches("Attention: none measured.").count(),
            1
        );
        assert!(!b.report.prompt.contains("()"), "an empty parenthetical");
        assert!(
            b.lines[0].starts_with("attention    none measured"),
            "{:?}",
            b.lines
        );
    }

    #[test]
    fn keep_awake_is_rendered_from_the_contract() {
        let k = compose("2026-10-29T09:40:12Z".into(), 7, origin(), calm(), None).keep_awake;
        assert!(k.contains("every 7 minutes"), "{k}");
        assert!(k.contains("/loop 7m /mj-brief"), "{k}");
        assert!(k.contains(report::fingerprint()), "{k}");
        assert!(!k.contains("two to four"), "{k}");
        assert!(!k.contains("/loop 7m majordomus brief"), "{k}");
        for section in &report::contract().sections {
            assert!(
                k.contains(&section.holds),
                "keep_awake lacks the section {}",
                section.key
            );
        }
    }

    // ------------------------------------------------ the lines and the line

    #[test]
    fn the_lines_follow_the_contracts_order() {
        let b = brief_of(calm());
        let labels: Vec<&str> = b
            .lines
            .iter()
            .filter(|l| !l.starts_with(' '))
            .map(|l| l.split("  ").next().unwrap_or_default())
            .collect();
        assert_eq!(
            labels,
            [
                "attention",
                "session",
                "work",
                "peers",
                "mesh",
                "integration",
                "ci",
                "machine",
                "release",
                "derive",
                "answered by"
            ]
        );
        assert!(
            b.lines.iter().all(|l| l.chars().count() <= WRAP_WIDTH),
            "{:?}",
            b.lines
        );
    }

    #[test]
    fn the_line_drops_tokens_with_a_count() {
        let b = brief_of(everything_wrong());
        assert!(b.line.chars().count() <= LINE_WIDTH, "{}", b.line);
        // the worst entry that is not the absent task follows the caller
        let worst = b
            .attention
            .iter()
            .find(|a| a.id != "session.no_task")
            .unwrap();
        let second = b.line.split(" | ").nth(1).unwrap_or_default();
        let lead = format!("{} {} ", worst.section, worst.word);
        assert!(second.starts_with(&lead), "{} / {lead}", b.line);
        let tokens: Vec<String> = (0..30).map(|i| format!("token-{i:02}-xxxxxxxx")).collect();
        let fitted = fit(tokens, LINE_WIDTH);
        assert!(fitted.chars().count() <= LINE_WIDTH, "{fitted}");
        let kept = fitted.matches("token-").count();
        assert!(fitted.ends_with(&format!(" | +{}", 30 - kept)), "{fitted}");
        assert!(!fitted.contains('…'), "{fitted}");
    }

    #[test]
    fn every_stale_or_unread_token_carries_its_word() {
        let b = brief_of(everything_wrong());
        assert!(b.line.contains("prs stale 17h"), "{}", b.line);
        let unread = brief_of(Sections {
            ci: BriefCiSection::default(),
            ..calm()
        });
        assert!(unread.line.contains("ci unread"), "{}", unread.line);
    }

    #[test]
    fn peers_lines_are_never_clipped() {
        let intent = "word ".repeat(80); // 400 bytes
        let peer = |i: usize| BriefPeer {
            id: format!("p{i}"),
            client: "claude-code".into(),
            pid: Some(1000 + i as u32),
            host: Some("lundra".into()),
            socket: Some("/tmp/x.sock".into()),
            branch: Some(format!("fix/{i}")),
            worktree: None,
            branch_source: "announce".into(),
            intent: Some(intent.trim().to_string()),
            other_claims: 0,
            you: i == 2,
        };
        let mut s = calm();
        s.peers.people = (1..=8).map(peer).collect();
        s.peers.named = s.peers.people.iter().map(peer_text).collect();
        s.peers.attached = 12;
        s.peers.summary = "[current] 12 attached on 1 board(s), 0 unread, 0 overlap(s); +4 more: majordomus peers".into();
        let b = brief_of(s);
        let text = b.lines.join(" ");
        let words_in_lines = text.matches("word").count();
        assert!(
            words_in_lines >= 8 * 80,
            "every word of every named intent is there"
        );
        assert!(text.contains("+4 more: majordomus peers"));
        assert!(!text.contains('…'), "nothing is clipped in the lines");
        assert!(
            !text.contains("/tmp/x.sock"),
            "the socket is the JSON's, not the line's"
        );
        assert!(
            text.contains("p2  you, claude-code pid 1002 on lundra @fix/2 (announce):"),
            "{text}"
        );
        assert!(b.lines.iter().all(|l| l.chars().count() <= WRAP_WIDTH));
    }

    #[test]
    fn a_peers_branch_and_intent_come_from_one_claim() {
        let claim = |name: &str, intent: &str, scope: &[&str], at: &str| Announcement {
            name: Some(name.into()),
            intent: intent.into(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
            at: at.into(),
        };
        let claims = vec![
            claim(
                "I4390",
                "the brief",
                &["refs/heads/feature/brief", "docs/BRIEF.md"],
                "2026-10-29T17:35:13Z",
            ),
            claim("batch", "a sibling's batch", &[], "2026-10-29T18:00:00Z"),
            claim(
                "old",
                "an older branch",
                &["refs/heads/fix/old"],
                "2026-10-28T09:00:00Z",
            ),
        ];
        let (c, b) = naming_claim(&claims);
        assert_eq!(
            c.map(|c| c.intent.as_str()),
            Some("the brief"),
            "not the latest claim's intent"
        );
        assert_eq!(
            b.as_deref(),
            Some("feature/brief"),
            "the newest claim naming a branch"
        );
        let unbranched = vec![
            claim("a", "first", &[], "2026-10-29T10:00:00Z"),
            claim("b", "second", &[], "2026-10-29T11:00:00Z"),
        ];
        let (c, b) = naming_claim(&unbranched);
        assert_eq!((c.map(|c| c.intent.as_str()), b), (Some("second"), None));
        assert_eq!(naming_claim(&[]), (None, None));
        let p = BriefPeer {
            id: "p2".into(),
            client: "claude-code".into(),
            pid: Some(4041),
            host: None,
            socket: None,
            branch: Some("feature/brief".into()),
            worktree: None,
            branch_source: "announce".into(),
            intent: Some("the brief".into()),
            other_claims: 2,
            you: true,
        };
        assert_eq!(
            peer_text(&p),
            "p2  you, claude-code pid 4041 @feature/brief (announce): the brief (+2 other claim(s): majordomus peers)"
        );
    }

    #[test]
    fn the_line_names_a_task_by_its_issue_when_its_words_open_with_one() {
        assert_eq!(task_label("t-1", "I2399: do it"), "I2399");
        assert_eq!(task_label("t-1", "I12 do it"), "I12");
        assert_eq!(task_label("t-1", "Improve things"), "t-1");
        assert_eq!(task_label("t-1", ""), "t-1");
    }

    #[test]
    fn clip_cuts_at_a_word_and_encode() {
        assert_eq!(clip("a  b\nc", 10), "a b c");
        assert_eq!(clip("abcdefghij", 5), "abcd…");
        assert_eq!(clip("the quick brown fox jumps", 16), "the quick brown…");
        assert_eq!(percent_encode("a b/c-d_e.f~"), "a%20b%2Fc-d_e.f~");
        assert_eq!(
            serde_json::to_value(BriefSectionState::Partial).unwrap(),
            json!("partial")
        );
    }

    #[test]
    fn the_input_defaults_to_every_board_and_forwarding() {
        let asked: BriefInput = serde_json::from_str("{}").expect("an empty object");
        assert_eq!(asked.checkouts, Checkouts::Repository);
        assert!(!asked.local);
        assert!(asked.session.is_none() && asked.since.is_none() && asked.background.is_empty());
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "brief");
        let c = &m.capabilities[0].capability;
        assert_eq!(c.id.as_str(), "brief.session");
        assert_eq!(
            c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_brief")
        );
        assert_eq!(
            c.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/brief")
        );
        assert_eq!(
            c.exposure.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["brief".to_string()])
        );
    }

    #[test]
    fn the_provider_session_is_read_in_the_lifecycles_order() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            provider_session_from_env(env(&[
                ("CLAUDE_CODE_SESSION_ID", "b"),
                ("MAJORDOMUS_PROVIDER_SESSION", "a")
            ])),
            Some("a".into())
        );
        assert_eq!(
            provider_session_from_env(env(&[("CLAUDE_CODE_SESSION_ID", "b")])),
            Some("b".into())
        );
        assert_eq!(
            provider_session_from_env(env(&[("CLAUDE_CODE_SESSION_ID", " ")])),
            None
        );
    }

    // ------------------------------------------------ budgets

    #[test]
    fn a_section_over_budget_is_unread_not_late() {
        let started = Instant::now();
        let slow = spawn("derive", || {
            std::thread::sleep(Duration::from_millis(600));
            derive_from(false, None, None)
        });
        let fast = spawn("machine", quiet_machine);
        let late_one: BriefDeriveSection = slow.take();
        let budget = budget_of("derive");
        assert!(
            started.elapsed() < budget + Duration::from_millis(50),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(late_one.observed.verdict, BriefVerdict::Unread);
        assert!(
            late_one.summary.contains("took > 100 ms"),
            "{}",
            late_one.summary
        );
        assert!(late_one.observed.elapsed_ms > late_one.observed.budget_ms);
        let read: BriefMachineSection = fast.take();
        assert_eq!(
            read.state,
            BriefSectionState::Read,
            "the other sections are read"
        );
        assert!(
            read.observed.elapsed_ms < budget_of("machine").as_millis() as u64,
            "a section collected after a slow one says what it took itself: {} ms",
            read.observed.elapsed_ms
        );
        let mut s = calm();
        s.derive = late_one;
        let b = brief_of(s);
        assert!(
            b.attention
                .iter()
                .any(|a| a.id == "section.over_budget" && a.fact == "derive took > 100 ms"),
            "{:?}",
            b.attention
        );
        // the line leads with it, naming its section once
        assert!(
            b.line.contains(" | derive unread took > 100 ms"),
            "{}",
            b.line
        );
        assert!(!b.line.contains("derive unread derive"), "{}", b.line);
    }

    #[test]
    fn an_older_servers_answer_still_deserialises() {
        let mut v = serde_json::to_value(brief_of(calm())).unwrap();
        let m = v.as_object_mut().unwrap();
        for k in [
            "work",
            "ci",
            "machine",
            "attention",
            "report",
            "fingerprints",
            "release",
        ] {
            m.remove(k);
        }
        for section in ["session", "peers", "mesh", "integration", "derive"] {
            m[section].as_object_mut().unwrap().remove("observed");
        }
        let b: SessionBrief = serde_json::from_value(v).expect("an older answer parses");
        assert_eq!(b.work.state, BriefSectionState::Unread);
        assert_eq!(b.ci.state, BriefSectionState::Unread);
        assert_eq!(b.machine.state, BriefSectionState::Unread);
        assert_eq!(b.session.observed.verdict, BriefVerdict::Unread);
        assert!(b.report.prompt.is_empty());
        // and a forwarded one is finished here
        let done = finish_forwarded(b, &BriefInput::default());
        assert!(done.report.prompt.starts_with(&report::contract().marker));
    }

    #[test]
    fn the_release_default_is_what_it_always_was() {
        assert_eq!(
            serde_json::to_value(BriefReleaseSection::default()).unwrap(),
            json!({
                "state": "unread",
                "landed": 0,
                "summary": "the server that composed this brief does not measure the release"
            })
        );
    }

    #[test]
    fn a_fingerprint_moves_with_a_fact_and_not_with_the_clock() {
        let a = brief_of(everything_wrong());
        let mut later = everything_wrong();
        later.integration.age_seconds = Some(18 * 3600);
        later.machine.free_bytes = Some(19 << 30);
        later.session.observed.observed_at = Some("2026-10-29T10:40:12Z".into());
        later.ci.observed.age_seconds = Some(29);
        let b = brief_of(later);
        assert_eq!(a.fingerprints["integration"], b.fingerprints["integration"]);
        assert_eq!(a.fingerprints["work"], b.fingerprints["work"]);
        let mut moved = everything_wrong();
        moved.integration.master_now = Some("ffffffffff".into());
        let c = brief_of(moved);
        assert_ne!(a.fingerprints["integration"], c.fingerprints["integration"]);
        assert_eq!(a.fingerprints["mesh"], c.fingerprints["mesh"]);
    }

    // ------------------------------------------------ the derive lock

    #[test]
    fn the_derive_lock_is_read_as_the_machine_lock_writes_it() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let lock = dir.path().join(".derive.lock");
        let priority = dir.path().join(".derive.priority");
        let free = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(free.state, BriefSectionState::Read);
        assert_eq!(free.summary, "lock free");
        assert!(free.holder.is_none());
        std::fs::create_dir(&lock).expect("the lock directory");
        let me = std::process::id();
        std::fs::write(
            lock.join("owner"),
            format!("derive:feature/x pid={me} branch=feature/x\n"),
        )
        .expect("the owner line");
        std::fs::write(&priority, "release/v1\n").expect("the marker");
        let held = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(held.pid, Some(me));
        assert_eq!(held.alive, Some(true));
        assert!(held
            .summary
            .starts_with("lock held by derive:feature/x (pid"));
        assert!(held.summary.contains("running"));
        assert!(held.summary.ends_with("; priority to release/v1"));
        assert_eq!(owner_pid("derive:feature/x 4242"), Some(4242));
        assert_eq!(owner_pid("someone at 10:00"), None);
        std::fs::write(lock.join("owner"), "someone by hand\n").expect("the owner line");
        let unchecked = derive_from(true, Some(&lock), None);
        assert!(unchecked.summary.contains("no pid in its line"));
        let mut s = calm();
        s.derive = held;
        let b = brief_of(s);
        assert!(b.line.contains("derive derive:feature/x 0m"), "{}", b.line);
    }

    #[test]
    fn a_derive_lock_whose_holder_is_gone_is_said_to_be_reclaimable() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let lock = dir.path().join(".derive.lock");
        let priority = dir.path().join(".derive.priority");
        std::fs::create_dir(&lock).expect("the lock directory");
        std::fs::write(
            lock.join("owner"),
            "derive:feature/y pid=99999999 branch=feature/y\n",
        )
        .expect("the owner line");
        std::fs::write(&priority, "\n").expect("the marker");
        let gone = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(gone.pid, Some(99_999_999));
        assert_eq!(gone.alive, Some(false));
        assert!(gone.summary.starts_with(
            "lock held by derive:feature/y (pid 99999999, NOT running: the next derive reclaims it"
        ));
        assert_eq!(gone.priority.as_deref(), Some(""));
        assert!(gone.summary.ends_with("; priority to ?"));
        std::fs::File::open(&lock)
            .and_then(|f| f.set_modified(std::time::SystemTime::now() + Duration::from_secs(3600)))
            .expect("the lock's time");
        let ageless = derive_from(true, Some(&lock), None);
        assert_eq!(ageless.held_seconds, None);
    }

    #[test]
    fn every_state_has_its_word() {
        for (state, word) in [
            (BriefSectionState::Read, "read"),
            (BriefSectionState::Partial, "partial"),
            (BriefSectionState::Unread, "unread"),
            (BriefSectionState::Undeclared, "undeclared"),
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(word));
        }
        for v in [
            BriefVerdict::Current,
            BriefVerdict::Stale,
            BriefVerdict::Partial,
            BriefVerdict::Unread,
            BriefVerdict::Undeclared,
        ] {
            assert_eq!(serde_json::to_value(v).unwrap(), json!(v.as_str()));
        }
    }

    // ------------------------------------------------ owners that answer what a test says

    use crate::peers::Transport;
    use crate::synthetic::SyntheticRepository;
    use std::io::{Read as _, Write as _};

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct Anything {}

    impl BenchmarkCases for Anything {
        fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
            vec![NamedCase::new("default", Anything {})]
        }
    }

    /// What each stand-in owner answers, by the repository it is asked about: the sections
    /// run on threads of their own, so the answer is found by the context's root rather than
    /// by the test's thread.
    static CANNED: std::sync::Mutex<Option<BTreeMap<(String, &'static str), serde_json::Value>>> =
        std::sync::Mutex::new(None);

    thread_local! {
        /// The repository the test on this thread is canning answers for.
        static CANNING: std::cell::RefCell<String> = Default::default();
    }

    fn can(id: &'static str, answer: serde_json::Value) {
        let root = CANNING.with(|c| c.borrow().clone());
        CANNED
            .lock()
            .unwrap()
            .get_or_insert_with(Default::default)
            .insert((root, id), answer);
    }

    fn canned(ctx: &Context, id: &'static str) -> Result<serde_json::Value, CapabilityError> {
        CANNED
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(ctx.index.repository.root.clone(), id)).cloned())
            .ok_or_else(|| CapabilityError::Refused(format!("{id} does not answer")))
    }

    fn canned_episodes(ctx: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        canned(ctx, "lifecycle.episodes")
    }

    fn canned_peers(ctx: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        canned(ctx, "peers.list")
    }

    /// A context whose registry holds the brief and stand-ins for the owners it asks through
    /// the registry, the board's with the HTTP route a leaseholder is asked over.
    fn canned_context(repo: &SyntheticRepository) -> Context {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![module()])
            .with_builtin(vec![
                capability! {
                    id: "lifecycle.episodes", title: "Episodes", description: "Canned.",
                    input: Anything, output: serde_json::Value,
                    stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
                    handler: canned_episodes,
                },
                capability! {
                    id: "peers.list", title: "Peers", description: "Canned.",
                    input: Anything, output: serde_json::Value,
                    stability: Stability::Experimental,
                    exposure: Exposure { http: get("/api/v1/peers"), ..Exposure::default() },
                    tags: [],
                    handler: canned_peers,
                },
            ])
            .build()
            .expect("a registry of stand-ins");
        let ctx = Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        );
        CANNING.with(|c| *c.borrow_mut() = ctx.index.repository.root.clone());
        ctx
    }

    fn bare_context(repo: &SyntheticRepository) -> Context {
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .build()
                    .expect("an empty registry"),
            ),
        )
    }

    /// The task record a checkout keeps, written where the session section reads it.
    fn task_record(repo: &SyntheticRepository, outcome: &str) {
        let root = canonical(repo);
        let dir = root.join(server::local_half(&root)).join("state");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("current.yaml"),
            format!(
                "id: t-1\ntask: \"I2399: cover the brief\"\nprofile: implementation\nowner: \"k\"\nprovider_session: prov-9\nstarted_at: 2026-10-23T19:51:10Z\ncheckpoint_at: 2026-10-27T20:32:54Z\noutcome: {outcome}\n"
            ),
        )
        .unwrap();
    }

    fn two_episodes() -> serde_json::Value {
        json!({
            "present": true,
            "store": ".ai/local/state/sessions-open",
            "worktree": "/w",
            "current": "s-0001",
            "episodes": [
                {"session_id": "s-0001", "path": "a.yaml", "standing": "current",
                 "events": 1, "note": "the pointer resolves to it", "provider_session": "prov-1"},
                {"session_id": "s-0002", "path": "b.yaml", "provider": "claude-code",
                 "provider_session": "prov-2", "standing": "open", "events": 3,
                 "started_at": "2026-10-28T13:25:18Z",
                 "note": "this worktree, not pointed at"},
            ],
        })
    }

    /// A loopback HTTP server that answers each request by its path, for the rest of the
    /// test binary's life; its URL.
    fn serve(answer: impl Fn(&str) -> (u16, String) + Send + 'static) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&head);
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = answer(&path);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        url
    }

    /// An address nothing listens on.
    fn dead_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        drop(listener);
        url
    }

    /// The canonical root the handler reads its lease under.
    fn canonical(repo: &SyntheticRepository) -> PathBuf {
        repo.root()
            .canonicalize()
            .unwrap_or_else(|_| repo.root().to_path_buf())
    }

    /// Write a lease of another process into the checkout, naming `url`.
    fn lease_of_another(repo: &SyntheticRepository, url: Option<&str>) {
        let root = canonical(repo);
        let file = lease::lease_file(&root, &server::local_half(&root));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            json!({
                "schema": lease::SCHEMA,
                "pid": 0,
                "token": "another-process",
                "root": root,
                "url": url,
                "started_at": "2026-10-11T00:00:00Z",
            })
            .to_string(),
        )
        .unwrap();
    }

    fn input(checkouts: Checkouts, session: Option<&str>) -> BriefInput {
        BriefInput {
            session: session.map(str::to_string),
            checkouts,
            ..BriefInput::default()
        }
    }

    // ------------------------------------------------ the session section

    /// The provider session the caller names wins over the checkout's pointer; without one,
    /// the pointer's episode is named as the pointer's, never as this session's.
    #[test]
    fn the_env_session_wins_over_the_pointer() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("lifecycle.episodes", two_episodes());
        task_record(&repo, "active");
        let root = canonical(&repo);

        let me = ctx.peers.attach(Transport::Http);
        let mine = session_section(&ctx.for_caller(me.clone()), &root, Some("prov-2"), &[]);
        assert_eq!(mine.state, BriefSectionState::Read);
        assert_eq!(mine.episode.as_deref(), Some("s-0002"));
        assert_eq!(mine.resolved_by.as_deref(), Some("provider_session"));
        assert_eq!(mine.provider_session.as_deref(), Some("prov-2"));
        assert_eq!(mine.caller.as_deref(), Some(me.as_str()));
        assert_eq!(mine.task_id.as_deref(), Some("t-1"));
        assert_eq!(mine.task_owner_session.as_deref(), Some("prov-9"));
        assert!(
            mine.summary.starts_with(&format!(
                "[current] {} = prov-2 (provider session); episode s-0002, opened ",
                me.as_str()
            )),
            "{}",
            mine.summary
        );
        assert!(
            mine.summary
                .contains("; task t-1 [active]: I2399: cover the brief"),
            "{}",
            mine.summary
        );

        let pointed = session_section(&ctx, &root, None, &[]);
        assert_eq!(pointed.episode.as_deref(), Some("s-0001"));
        assert_eq!(pointed.resolved_by.as_deref(), Some("pointer"));
        assert!(
            pointed
                .summary
                .contains("the checkout's pointer (owner prov-1), not necessarily this session"),
            "{}",
            pointed.summary
        );
    }

    /// `current.yaml` keeps the last task after it finished: such a task is not the session's
    /// task, and the section says what it was (I4293).
    #[test]
    fn a_partial_task_is_not_active() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("lifecycle.episodes", two_episodes());
        let root = canonical(&repo);
        for outcome in [
            "partial",
            "completed",
            "blocked",
            "failed",
            "no_match",
            "handed_over",
        ] {
            task_record(&repo, outcome);
            let s = session_section(&ctx, &root, None, &[]);
            assert_eq!(
                (s.task_id.as_deref(), s.task.as_deref()),
                (None, None),
                "{outcome}"
            );
            assert_eq!(s.task_outcome.as_deref(), Some(outcome));
            assert!(
                s.summary.contains(&format!(
                    "no active task (last t-1 [{outcome}], checkpointed 10-27, owner prov-9)"
                )),
                "{outcome}: {}",
                s.summary
            );
            let line = brief_of(Sections {
                session: s,
                ..calm()
            })
            .line;
            assert!(
                !line.contains("t-1"),
                "a finished task is off the line: {line}"
            );
        }
        task_record(&repo, "active");
        assert_eq!(
            session_section(&ctx, &root, None, &[]).task_id.as_deref(),
            Some("t-1")
        );
    }

    #[test]
    fn a_session_whose_episodes_cannot_be_read_is_partial() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("lifecycle.episodes", json!({ "not": "episodes" }));
        let partial = session_section(&ctx, &canonical(&repo), Some("prov-2"), &[]);
        assert_eq!(partial.state, BriefSectionState::Partial);
        assert_eq!(partial.episode, None);
        assert!(
            partial.summary.contains("episodes unread ("),
            "{}",
            partial.summary
        );
        assert_eq!(partial.observed.verdict, BriefVerdict::Partial);
        assert!(partial.observed.remedy.is_some());
    }

    // ------------------------------------------------ the peers section

    /// The board of a real `peers.list`: nine workers, two of whom claim scopes that meet.
    fn nine_on_a_board() -> serde_json::Value {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        for i in 0..9 {
            let p = ctx.peers.attach(Transport::Http);
            let scope = match i {
                0 => "src/cockpit".to_string(),
                1 => "src/cockpit/pages.rs".to_string(),
                _ => format!("docs/{i}.md"),
            };
            ctx.peers
                .announce(&p, &format!("worker {i} at work"), vec![scope]);
        }
        ctx.execute("peers.list", json!({ "checkouts": "this" }))
            .expect("the board")
    }

    #[test]
    fn the_peers_section_names_eight_counts_the_rest_and_says_what_it_could_not_read() {
        let mut list = nine_on_a_board();
        let mut unread = list["boards"][0].clone();
        unread["reason"] = json!("its server is starting and has no board to read yet");
        let at = if unread.get("checkout").is_some() {
            &mut unread["checkout"]
        } else {
            &mut unread
        };
        at["worktree"] = json!("/elsewhere");
        at["this_checkout"] = json!(false);
        list["boards"].as_array_mut().unwrap().push(unread);
        list["covered"] = json!(2);

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("peers.list", list);
        let s = peers_section(&ctx, Checkouts::Repository, None, None, None);
        assert_eq!(s.state, BriefSectionState::Partial);
        assert_eq!((s.attached, s.boards, s.overlaps), (9, 2, 1));
        assert_eq!(
            s.unread,
            ["/elsewhere: its server is starting and has no board to read yet"]
        );
        assert_eq!((s.named.len(), s.people.len()), (PEERS_NAMED, PEERS_NAMED));
        assert!(
            s.named[0].ends_with("@? : worker 0 at work")
                || s.named[0].ends_with("@?: worker 0 at work"),
            "{}",
            s.named[0]
        );
        assert_eq!(s.people[0].intent.as_deref(), Some("worker 0 at work"));
        assert_eq!(
            s.summary,
            "[partial] 9 attached on 2 board(s), 1 unread, 1 overlap(s); +1 more: majordomus peers"
        );
    }

    #[test]
    fn a_board_nobody_answers_for_is_unread() {
        let repo = SyntheticRepository::small().expect("a repository");
        let bare = bare_context(&repo);
        let s = peers_section(&bare, Checkouts::This, None, None, None);
        assert_eq!(s.state, BriefSectionState::Unread);
        assert_eq!(s.observed.verdict, BriefVerdict::Unread);
        assert_eq!(s.observed.remedy.as_deref(), Some("majordomus peers"));
        assert!(s.summary.starts_with("unread: peers.list did not answer ("));
    }

    // ------------------------------------------------ a leaseholder that did not answer

    #[test]
    fn a_command_line_whose_leaseholder_did_not_answer_reads_its_board_or_says_it_is_unread() {
        // measured 2026-10-29: the primary's leaseholder did not answer the brief, and the
        // command line said "0 attached on 649 board(s)" while five workers were on its board
        let leaseholder = {
            let repo = SyntheticRepository::small().expect("a repository");
            let ctx = repo.context().expect("a context");
            let p = ctx.peers.attach(Transport::Http);
            ctx.peers
                .announce(&p, "the leaseholder's worker", vec!["lib".into()]);
            serde_json::to_value(ctx.peers.list()).expect("its board")
        };
        let board = json!({ "peers": leaseholder }).to_string();
        let url = serve(move |path| {
            if path.starts_with("/api/v1/peers") {
                (200, board.clone())
            } else {
                (404, "{}".into())
            }
        });
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        let mut own = nine_on_a_board();
        own["peers"] = json!([]);
        own["overlaps"] = json!([]);
        can("peers.list", own);

        let s = peers_section(&ctx, Checkouts::This, None, Some(url), None);
        assert_eq!(s.state, BriefSectionState::Read, "{}", s.summary);
        assert_eq!(
            s.attached, 1,
            "the leaseholder's worker is on this checkout's board"
        );
        assert!(
            s.named
                .iter()
                .any(|n| n.contains("the leaseholder's worker")),
            "{:?}",
            s.named
        );

        let s = peers_section(&ctx, Checkouts::This, None, Some(dead_url()), None);
        assert_eq!(s.state, BriefSectionState::Partial, "{}", s.summary);
        assert_eq!(s.unread.len(), 1, "{:?}", s.unread);
        assert!(s.unread[0].starts_with("this checkout's board, held by the leaseholder: "));
        assert!(
            s.summary.starts_with("[partial] 0 attached"),
            "{}",
            s.summary
        );
    }

    #[test]
    fn the_caller_is_named_from_the_board_the_peers_section_read() {
        let psid = "07a5ffde-74b3-4ca6-ab0d-1ad2c9e56d9e";
        let unnamed = || {
            let mut s = BriefSessionSection::over_budget("session", Duration::ZERO, Duration::ZERO);
            s.summary =
                format!("[current] provider session {psid} (not on this board); episode e1");
            s
        };
        let none = || BriefPeersSection::over_budget("peers", Duration::ZERO, Duration::ZERO);
        let mut found = none();
        found.people.push(BriefPeer {
            id: "p2".into(),
            client: "claude-code".into(),
            pid: None,
            host: None,
            socket: None,
            branch: None,
            worktree: None,
            branch_source: "server".into(),
            intent: None,
            other_claims: 0,
            you: true,
        });
        let mut s = unnamed();
        name_the_caller(&mut s, &found, Some(psid));
        assert_eq!(s.caller.as_deref(), Some("p2"));
        assert_eq!(
            s.summary,
            "[current] p2 = 07a5ffde (provider session); episode e1"
        );

        let mut unread = none();
        unread
            .unread
            .push("this checkout's board, held by the leaseholder: x did not answer".into());
        let mut s = unnamed();
        name_the_caller(&mut s, &unread, Some(psid));
        assert_eq!(s.caller, None);
        assert!(
            s.summary.contains("(this checkout's board unread)"),
            "{}",
            s.summary
        );

        let mut s = unnamed();
        name_the_caller(&mut s, &none(), Some(psid));
        assert!(
            s.summary.contains("(not on this board)"),
            "a board that was read and lacks it"
        );
    }

    // ------------------------------------------------ a superseded server

    #[test]
    fn a_superseded_server_adds_the_leaseholders_board_and_says_it_is_superseded() {
        let leaseholder = {
            let repo = SyntheticRepository::small().expect("a repository");
            let ctx = repo.context().expect("a context");
            let p = ctx.peers.attach(Transport::Http);
            ctx.peers
                .announce(&p, "the leaseholder's worker", vec!["lib".into()]);
            serde_json::to_value(ctx.peers.list()).expect("its board")
        };
        let board = json!({ "peers": leaseholder }).to_string();
        let url = serve(move |path| {
            if path.starts_with("/api/v1/peers") {
                (200, board.clone())
            } else {
                (404, "{}".into())
            }
        });

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("lifecycle.episodes", two_episodes());
        task_record(&repo, "active");
        let mut own = nine_on_a_board();
        let first = own["peers"][0].clone();
        own["peers"] = json!([first]);
        own["overlaps"] = json!([]);
        can("peers.list", own);
        lease_of_another(&repo, Some(&url));

        let me = ctx.peers.attach(Transport::Http);
        let b = brief(&ctx.for_caller(me), input(Checkouts::This, None)).expect("a brief");
        assert_eq!(b.answered_by.process, "superseded");
        assert_eq!(b.peers.state, BriefSectionState::Partial);
        assert_eq!(b.peers.attached, 2, "its own worker and the leaseholder's");
        assert!(b
            .peers
            .named
            .iter()
            .any(|n| n.contains("the leaseholder's worker")));
        let said = b.peers.superseded.clone().expect("superseded");
        assert!(said.contains(&format!("superseded by {url}")), "{said}");
        assert!(b.line.contains(" superseded"), "{}", b.line);
        assert!(
            b.attention.iter().any(|a| a.id == "peers.superseded"),
            "{:?}",
            b.attention
        );

        lease_of_another(&repo, Some(&dead_url()));
        let b = brief(
            &ctx.for_caller(ctx.peers.attach(Transport::Http)),
            input(Checkouts::This, None),
        )
        .expect("a brief");
        assert_eq!(b.peers.unread.len(), 1, "{:?}", b.peers.unread);
        assert!(b.peers.unread[0].starts_with("the leaseholder's board: "));
        assert!(b.attention.iter().any(|a| a.id == "peers.unread"));
    }

    // ------------------------------------------------ release

    fn canned_owed(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({
            "stance": "owed", "summary": "3 landed commit(s) wait", "landed": 3, "landings": 1,
            "overdue_after_seconds": 21600,
        }))
    }

    fn canned_unreadable(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({ "stance": "nowhere" }))
    }

    fn currency_context(
        repo: &SyntheticRepository,
        handler: fn(&Context, Anything) -> Result<serde_json::Value, CapabilityError>,
    ) -> Context {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(vec![capability! {
                id: "release.currency", title: "Currency", description: "Canned.",
                input: Anything, output: serde_json::Value,
                stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
                handler: handler,
            }])
            .build()
            .expect("a registry of one stand-in");
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        )
    }

    #[test]
    fn the_release_section_is_what_release_currency_answered() {
        let repo = SyntheticRepository::small().expect("a repository");
        let measured = release_section(&repo.context().expect("a context"));
        assert_eq!(
            (measured.state, measured.stance),
            (BriefSectionState::Unread, Some(Stance::Unknown)),
            "{}",
            measured.summary
        );
        let owed = release_section(&currency_context(&repo, canned_owed));
        assert_eq!(
            (owed.state, owed.stance, owed.landed),
            (BriefSectionState::Read, Some(Stance::Owed), 3)
        );
        for broken in [
            bare_context(&repo),
            currency_context(&repo, canned_unreadable),
        ] {
            let section = release_section(&broken);
            assert_eq!(section.state, BriefSectionState::Unread);
            assert!(section
                .summary
                .starts_with("release.currency did not answer:"));
        }
    }

    #[test]
    fn the_release_is_said_in_the_line() {
        assert_eq!(
            release_token(&release(Stance::Overdue, 4)),
            "release overdue 4"
        );
        assert_eq!(release_token(&release(Stance::Owed, 1)), "release owed 1");
        assert_eq!(
            release_token(&BriefReleaseSection::default()),
            "release unread"
        );
        let current = brief_of(Sections {
            release: release(Stance::Current, 0),
            ..calm()
        })
        .line;
        assert!(current.contains("release current"), "{current}");
        // a release that needs attention leads the line as its worst entry, named by its
        // section, so the line says it even when the release token is dropped at the right
        let overdue = brief_of(Sections {
            release: release(Stance::Overdue, 4),
            ..calm()
        })
        .line;
        assert!(
            overdue
                .split(" | ")
                .nth(1)
                .is_some_and(|t| t.starts_with("release overdue 4 landed")),
            "{overdue}"
        );
    }

    // ------------------------------------------------ asking the leaseholder

    #[test]
    fn the_leaseholder_is_asked_over_the_declared_route_and_every_failure_is_named() {
        let repo = SyntheticRepository::small().expect("a repository");
        let asked = input(Checkouts::This, None);
        let e = forward(&bare_context(&repo), "http://127.0.0.1:1", &asked).expect_err("no route");
        assert_eq!(e, "brief.session declares no HTTP route to ask over");

        let ctx = canned_context(&repo);
        let dead = dead_url();
        let e = forward(&ctx, &dead, &asked).expect_err("nobody listens");
        assert!(
            e.starts_with(&format!("the leaseholder at {dead} did not answer (")),
            "{e}"
        );

        let old = serve(|_| (404, "{}".into()));
        let e = forward(&ctx, &old, &asked).expect_err("an older server");
        assert_eq!(
            e,
            format!("the leaseholder at {old} answered 404 (a server older than this executable has no brief); composed by this process")
        );

        let theirs = brief_of(calm());
        let body = serde_json::to_string(&theirs).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let heard = std::sync::Arc::clone(&seen);
        let good = serve(move |path| {
            *heard.lock().unwrap() = path.to_string();
            (200, body.clone())
        });
        assert_eq!(forward(&ctx, &good, &asked), Ok(theirs));
        assert_eq!(
            seen.lock().unwrap().as_str(),
            "/api/v1/brief?local=true&checkouts=this"
        );
        // since and the provider's background travel with the question
        let full = BriefInput {
            since: Some("2026-10-29T09:20:00Z".into()),
            background: vec![BriefBackground {
                id: "b1".into(),
                description: "x".into(),
                status: String::new(),
            }],
            ..asked
        };
        let _ = forward(&ctx, &good, &full);
        let path = seen.lock().unwrap().clone();
        assert!(path.contains("&since=2026-10-29T09%3A20%3A00Z"), "{path}");
        assert!(
            path.contains("&background=%5B%7B%22id%22%3A%22b1%22"),
            "{path}"
        );
    }

    #[test]
    fn a_leaseholder_that_cannot_brief_leaves_the_brief_to_this_process() {
        let repo = SyntheticRepository::small().expect("a repository");
        let id = crate::repository::identity(&canonical(&repo));
        let url = serve(move |path| {
            if path == "/" {
                (
                    200,
                    json!({ "name": "majordomus", "repository_id": id }).to_string(),
                )
            } else {
                (404, "{}".into())
            }
        });
        lease_of_another(&repo, Some(&url));
        let ctx = canned_context(&repo);
        can("lifecycle.episodes", two_episodes());
        can("peers.list", nine_on_a_board());
        let b = brief(&ctx, input(Checkouts::This, Some("prov-2"))).expect("a brief");
        assert_eq!(b.answered_by.process, "this_process");
        let note = b.answered_by.note.clone().expect("why");
        assert!(
            note.contains(&format!("the leaseholder at {url} answered 404")),
            "{note}"
        );
        assert_eq!(b.session.episode.as_deref(), Some("s-0002"));
        // a composition that is not the leaseholder's reads no forge, and says so
        assert_eq!(b.ci.state, BriefSectionState::Unread);
        assert!(
            b.ci.summary.contains("not the leaseholder"),
            "{}",
            b.ci.summary
        );
        assert!(b.report.prompt.starts_with(&report::contract().marker));
    }

    // ------------------------------------------------ the mesh section

    mod mesh_world {
        pub(super) use crate::mesh::testing::{runtime, Net};
    }

    #[test]
    fn an_active_mesh_is_read_for_its_links_its_claims_and_what_it_addresses_here() {
        use crate::mesh::journal::{ClaimMode, HandoverBody, SessionInfo};
        let net = std::sync::Arc::new(mesh_world::Net::default());
        let (a, b) = (
            mesh_world::runtime(&net, "a:1"),
            mesh_world::runtime(&net, "b:1"),
        );
        a.dial(&["b:1".into()]).expect("B welcomes A");
        a.claim(
            &SessionInfo::named("s1", "cli"),
            vec!["apps".into()],
            None,
            ClaimMode::Advisory,
            None,
        )
        .unwrap();
        a.claim(
            &SessionInfo::named("s2", "cli"),
            vec!["apps/majordomus-cli".into()],
            None,
            ClaimMode::Advisory,
            None,
        )
        .unwrap();
        let me = a.status().runtime.clone().expect("a runtime");
        a.request_review("s1", "feature/x".into(), vec![], None, Some(me), None)
            .unwrap();
        let body = "# Objective\ncover the brief\n".to_string();
        a.publish_handover(HandoverBody {
            id: HandoverBody::digest_of(&body),
            task: None,
            issue: None,
            milestone: None,
            branch: Some("feature/x".into()),
            head: None,
            created_at: None,
            name: None,
            body,
        })
        .unwrap();
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&a));
        let fresh = mesh_section(&ctx, None);
        assert_eq!(fresh.state, BriefSectionState::Read);
        assert_eq!((fresh.links, fresh.connected, fresh.failing), (1, 1, 0));
        assert_eq!((fresh.claims_held, fresh.own_claims), (2, 2));
        assert_eq!(
            (
                fresh.reviews_for_you,
                fresh.handovers_waiting,
                fresh.overlaps_for_you
            ),
            (1, 0, 0)
        );
        assert_eq!(
            fresh.summary,
            "1 of 1 link(s) connected; 2 claim(s) held, 2 of them this runtime's; \
             for this runtime: 1 review(s), 0 handover(s) waiting, 0 overlap(s)"
        );
        std::thread::sleep(Duration::from_millis(3500));
        let stale = mesh_section(&ctx, None);
        assert_eq!((stale.connected, stale.failing), (0, 1));
        for r in [a, b] {
            r.stop();
        }
    }

    #[test]
    fn an_overlap_among_this_runtimes_own_claims_is_not_for_it() {
        use crate::mesh::journal::{ClaimMode, SessionInfo};
        let net = std::sync::Arc::new(mesh_world::Net::default());
        let (a, b) = (
            mesh_world::runtime(&net, "a:1"),
            mesh_world::runtime(&net, "b:1"),
        );
        let key = a.dial(&["b:1".into()]).expect("B welcomes A");
        let claim = |r: &crate::mesh::cooperation::Cooperation, session: &str, scope: &str| {
            r.claim(
                &SessionInfo::named(session, "cli"),
                vec![scope.into()],
                None,
                ClaimMode::Advisory,
                None,
            )
            .unwrap();
        };
        let sync = || {
            for _ in 0..2 {
                b.journal().beat_own();
                a.journal().beat_own();
                a.sync_with(&key).unwrap();
            }
        };
        for (session, scope) in [("s1", "apps"), ("s2", "apps/majordomus-cli")] {
            claim(&a, session, scope);
        }
        sync();
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&a));
        let own = mesh_section(&ctx, None);
        assert_eq!(own.overlaps_for_you, 0, "{}", own.summary);
        claim(&b, "s9", "apps/majordomus-cli/src");
        sync();
        assert_eq!(mesh_section(&ctx, None).overlaps_for_you, 2);
        for r in [a, b] {
            r.stop();
        }
    }

    #[test]
    fn a_mesh_that_is_not_running_says_why() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.decline_cooperation("no endpoint to be dialed at");
        let off = mesh_section(&ctx, None);
        assert_eq!(off.state, BriefSectionState::Undeclared);
        assert_eq!(off.summary, "not active here: no endpoint to be dialed at");
        // the cooperation status words its reason "not active: ..."; the section says it once
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh
            .decline_cooperation("not active: no shared server activated the mesh in this process");
        let once = mesh_section(&ctx, None);
        assert_eq!(
            once.summary,
            "not active here: no shared server activated the mesh in this process"
        );
        // a command line whose leaseholder did not answer did not read the mesh: unread, not off
        let unread = mesh_section(&ctx, Some("http://0.0.0.0:62328"));
        assert_eq!(unread.state, BriefSectionState::Unread);
        assert_eq!(unread.observed.verdict, BriefVerdict::Unread);
        assert!(unread
            .summary
            .starts_with("unread: the mesh runs in the leaseholder at http://0.0.0.0:62328"));
        let line = brief_of(Sections {
            mesh: unread,
            ..calm()
        })
        .line;
        assert!(line.contains("mesh unread"), "{line}");
    }

    #[test]
    fn every_section_line_leads_with_its_status_word() {
        assert_eq!(worded("current", "lock free"), "[current] lock free");
        assert_eq!(
            worded("current", "[stale] queue observed 2 h ago"),
            "[stale] queue observed 2 h ago"
        );
        assert_eq!(
            worded("unread", "unread: not the leaseholder"),
            "unread: not the leaseholder"
        );
        assert_eq!(release_word(&release(Stance::Owed, 2)), "owed");
        assert_eq!(release_word(&release(Stance::Overdue, 2)), "overdue");
        assert_eq!(release_word(&BriefReleaseSection::default()), "unread");
        let b = brief_of(Sections {
            release: release(Stance::Owed, 2),
            ..calm()
        });
        let release_line = b
            .lines
            .iter()
            .find(|l| l.starts_with("release "))
            .expect("a release line");
        assert!(
            release_line.starts_with("release      [owed] "),
            "{release_line}"
        );
        let derive_line = b
            .lines
            .iter()
            .find(|l| l.starts_with("derive "))
            .expect("a derive line");
        assert!(derive_line.contains("] "), "{derive_line}");
        // every section's first line leads with a status word, bracketed or `unread`
        for name in [
            "session",
            "work",
            "peers",
            "mesh",
            "integration",
            "ci",
            "machine",
            "release",
            "derive",
        ] {
            let line = b
                .lines
                .iter()
                .find(|l| l.starts_with(&format!("{name} ")))
                .expect("a section line");
            let text = line[LABEL_WIDTH..].trim_start();
            assert!(
                text.starts_with('[') || text.starts_with("unread"),
                "{line}"
            );
        }
        // a checkout that never observed the forge has its queue unread, not undeclared
        let repo = SyntheticRepository::small().expect("a repository");
        let root = PathBuf::from(&repo.context().expect("a context").index.repository.root);
        let never = integration_section(&root);
        assert_eq!(never.state, BriefSectionState::Undeclared);
        assert_eq!(integration_word(&never), "unread");
        let line = worded(integration_word(&never), &never.summary);
        assert_eq!(
            line,
            "[unread] this checkout never observed the forge: majordomus prs refresh"
        );
    }

    // ------------------------------------------------ integration

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A clone whose origin/master is `tip`'s subject, and an observation of `observed`
    /// made `ago` seconds before now.
    fn observed_clone(observed_moves: bool, ago: i64, tip: &str) -> (tempfile::TempDir, PathBuf) {
        use crate::integration::{store_observation, ForgeObservation, OBSERVATION_SCHEMA};
        let dir = tempfile::tempdir().expect("a scratch directory");
        let root = dir.path().join("clone");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "master"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", tip]);
        let now_sha = git(&root, &["rev-parse", "HEAD"]);
        git(
            &root,
            &["update-ref", "refs/remotes/origin/master", &now_sha],
        );
        let at =
            crate::peers::rfc3339(std::time::SystemTime::now() - Duration::from_secs(ago as u64));
        store_observation(
            &root,
            &ForgeObservation {
                schema: OBSERVATION_SCHEMA,
                repository: "owner/repo".into(),
                base: "master".into(),
                base_sha: if observed_moves { base } else { now_sha },
                observed_at: at,
                required_checks: Some(vec!["ci".into()]),
                review_policy: Some(Default::default()),
                up_to_date_required: Some(true),
                merge_methods: Some(vec!["merge".into()]),
                pull_requests: Vec::new(),
                resolved: Default::default(),
                delete_branch_on_merge: None,
            },
        )
        .expect("an observation");
        (dir, root)
    }

    #[test]
    fn an_observation_past_its_bound_is_stale() {
        let repo = SyntheticRepository::small().expect("a repository");
        let never = integration_section(repo.root());
        assert_eq!(never.state, BriefSectionState::Undeclared);
        assert_eq!(
            never.observed.remedy.as_deref(),
            Some("majordomus prs refresh")
        );

        let (_d, old) = observed_clone(false, 2 * 3600, "two");
        let s = integration_section(&old);
        assert!(s.stale, "{}", s.summary);
        assert_eq!(s.observed.verdict, BriefVerdict::Stale);
        assert_eq!(s.remedy.as_deref(), Some("majordomus prs refresh"));
        assert!(
            s.summary.starts_with("[stale] queue observed 2 h ago"),
            "{}",
            s.summary
        );
        let line = brief_of(Sections {
            integration: s,
            ..calm()
        })
        .line;
        assert!(line.contains("prs stale 2h"), "{line}");
        assert!(
            !line.contains(" open, next #"),
            "never the bare count: {line}"
        );

        let (_d, moved) = observed_clone(true, 60, "moved");
        let s = integration_section(&moved);
        assert!(
            s.stale,
            "master moved past the observation within the bound: {}",
            s.summary
        );
        assert_eq!(s.moved_by, Some(1));
        let fresh_dir = observed_clone(false, 60, "fresh");
        let fresh = integration_section(&fresh_dir.1);
        assert!(!fresh.stale, "{}", fresh.summary);
        assert_eq!(fresh.observed.verdict, BriefVerdict::Current);
    }

    #[test]
    fn the_last_merge_is_masters_not_the_trails() {
        use crate::integration::drain;
        let (_d, root) = observed_clone(false, 60, "Merge pull request #2653 from korczis/chore/x");
        drain::record(
            &root,
            drain::IntegrationEvent {
                at: "2026-10-23T00:00:00Z".into(),
                pr: Some(2366),
                ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeSucceeded)
            },
        )
        .expect("recorded");
        let s = integration_section(&root);
        assert_eq!(
            s.last_merge.as_ref().and_then(|m| m.pr),
            Some(2653),
            "{}",
            s.summary
        );
        assert!(
            s.summary.contains("last merge on master #2653"),
            "{}",
            s.summary
        );
        assert!(
            s.summary.contains("last executor merge #2366"),
            "{}",
            s.summary
        );
    }
}
