//! The forge side of run supervision (I3903): observe the in-flight runs and the open pull
//! requests' heads, decide with [`super::supervise::decide`], and — only with `--apply` —
//! force-cancel what was decided, each act re-checked and recorded first.
//!
//! The discipline is the drain's. A decision is taken over one observation and acted on
//! after another: immediately before each cancel the run and its pull request's head are
//! read again, and the run is cancelled only if the same reason still holds
//! ([`super::supervise::still_holds`]). The act is appended to the integration audit trail
//! (`run_cancel_attempted`) before it is taken; a trail that cannot be written stops every
//! act. The outcome follows it: `run_cancelled`, `run_cancel_failed`, or `run_already_ended`
//! when the forge refused because the run had completed (a 409, or the run read again
//! completed): two lanes that decided the same cancel in one burst skip the second, not fail it.
//!
//! The cancel is GitHub's `force-cancel`, not `gh run cancel`: a cancel GitHub accepted for a
//! queued run has returned success and done nothing, and a cancelled run's job has gone on
//! holding its runner; `POST repos/<repo>/actions/runs/<id>/force-cancel` ends it at once
//! (docs/CI.md).
//!
//! What it costs is bounded. Every workflow of the repository spends one `GITHUB_TOKEN` budget
//! (1,000 requests an hour), so a supervisor that read every in-flight run's jobs on every
//! requested validate run would spend it for all of them: a run's jobs are read only when a
//! rule could act on it ([`super::supervise::needs_jobs`]). Each `gh` call is ended after
//! [`GH_CALL_SECONDS`], and the whole observation after [`BUDGET_SECONDS`], so a degraded forge
//! is a failed apply (exit 12) within its step's own four minutes, never a job killed mid-act.
//! A lane that only covers for another passes `--unless-supervised-within`: a supervision
//! that succeeded that recently is the one that counts, and this one reads it and stops.
//!
//! It also reports whether the supervisor itself is alive ([`super::supervise::liveness_of`]):
//! the newest successful job of the supervising workflow, older than thirty minutes, is
//! `supervise_stale` — the one failure a supervisor cannot report about itself, so the queue
//! workflow fails on it. With `--supervised-by <lane>` and a window of the whole stale bound it
//! is that lane's liveness: a lane that always fails is not
//! hidden by the other lane's successes. The runs are read past those whose every job was
//! cancelled while waiting ([`SUCCESS_PAGES`], [`SUCCESS_JOB_READS`]).
//!
//! Each act re-checks its decision against what this pass has already done: a run it cancelled
//! is judged as cancelled by every later act, so no decision rests on a run already ended.
//!
//! A cancel paired with a re-run ([`super::supervise::Decision::rerun`]: the last run of a pull
//! request's head, a job of it past its bound) asks for the re-run as soon as the cancel is
//! taken, unless the head has its verdict already: the commit's runs are read whole first
//! ([`RunForge::runs_at`]), and when a run of the same workflow and branch completed with a
//! verdict the hung run is ended and not run again, so that no newer `ci` is written over that
//! verdict (#2678's shape). Nor is a run asked again once GitHub's own `run_attempt` of it has
//! reached [`MAX_ATTEMPTS`]. The forge re-runs only a run that has ended, and accepts a
//! force-cancel before it has, so [`RunForge::rerun`] reads the run until it reads completed
//! ([`RERUN_READS`] reads, [`RERUN_WAIT_SECONDS`] apart) and then asks once, unless the run
//! completed with a verdict of its own ([`Rerun::Decided`]). Its outcome is on the trail after
//! the cancel's (`run_rerun_attempted`, then `run_rerun_requested` or `run_rerun_failed`), and a
//! re-run that could not be asked is a failed act (exit 12). It is asked whenever the run reads
//! completed without a verdict after the act, whatever the forge answered the cancel: a `gh`
//! that timed out or met a 502 while the cancel landed leaves the run ended all the same.
//!
//! That can still leave a head bare: the re-run refused, a cancel the forge reported failed that
//! landed a minute later, a cancel by a person or by another lane. The observation lists runs in
//! flight, so a later pass would never see the head again, and the trail cannot find it either:
//! lundra has three runners, each with a checkout and a trail of its own, and the hosted lane
//! starts from a fresh checkout every time, so the runner that left a head bare need not be the
//! one that takes the next pass. So recovery is stateless, judged from what the forge holds, and
//! any runner of any lane recovers any head ([`recover`]). Each pass reads the open pull
//! requests' heads that have no run of a bounded workflow in flight, each commit's runs whole,
//! and judges each workflow's runs of the pull request's branch ([`head_need`]): none in flight,
//! none with a verdict, the newest completed `cancelled` (by this supervisor, another lane or a
//! person) within [`RECOVERY_WINDOW_MINUTES`], and its `run_attempt` below [`MAX_ATTEMPTS`]: that
//! run is asked again. At [`MAX_ATTEMPTS`] the head is held as `head_exhausted`, a warning that
//! names the pull request on every pass that finds it and never a failed pass: one stuck pull
//! request is not a supervisor that stopped. So is a recovery re-run the forge refused, which
//! the next pass asks again: the pass whose own act bared the head was the red one. The trail
//! records each ask and decides nothing.
//!
//! Which heads are read is bounded as well ([`heads_to_read`]): first those with a run cancelled
//! within the window, from one listing of the pull requests' cancelled runs created in the last
//! [`CANCELLED_REACH_MINUTES`] ([`RunForge::cancelled_since`]), at most [`RECOVERY_READS`]; then
//! [`ROTATING_READS`] of the others, a different few every ten minutes, so that a head whose run
//! was created before the listing reaches is read within hours all the same. One pass asks at
//! most [`RECOVERY_RERUNS`] re-runs; the rest wait for the next.
//!
//! A run that reads `cancelled` can carry its head's verdict (the review of #2645 at
//! 859039b21de): a pull request's validate run cancels itself at its first red (I2664) and its
//! `if: always()` `ci` writes the failure, so the listing says `cancelled` of a head whose red is
//! written. Its jobs say so ([`RunObservation::verdict_in_jobs`]), and a run with a verdict there
//! is a verdict in [`head_need`], in [`paired_rerun_needed`] and in [`RunForge::rerun`]'s last
//! check: nothing is asked or held over it. The jobs are read only where the runs alone would ask
//! or hold, newest cancelled run first until one shows a verdict, at most [`VERDICT_READS`] in a
//! pass, so a pass spends about one more request per head it would otherwise ask or hold.
//!
//! A pull request labelled [`YIELDED_LABEL`] is the release path's (I2710): its run yielded,
//! cancelled itself, and `scripts/ci/release-path resume` runs it again once the release is
//! through and takes the label off. Recovery reads no such head, so it neither asks it again
//! while the release is busy, nor holds it, and no paired re-run is asked for it. The attempts the
//! release path took are not this supervisor's and do not count toward [`MAX_ATTEMPTS`]
//! ([`within_attempts`]): counting them would let a head that yielded twice reach the bound on its
//! first hang and stay with no run, which breaks the invariant that every open head gets its run
//! back. They are counted from the forge, as the attempts are: one per note the release path left
//! on the pull request naming the run ([`RunForge::yields`]), read only for a run at the bound.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::drain::{self, IntegrationAction, IntegrationEvent};
use super::forge::{gh_within, ForgeError};
use super::supervise::{
    self, Decision, Hold, JobObservation, Liveness, RunObservation, Situation, SuperviseModel,
    FULL_LABEL, IN_FLIGHT, YIELDED_LABEL,
};

/// What the forge said about the runs, at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunsObservation {
    /// `owner/name`.
    pub repository: String,
    /// The default branch.
    pub default_branch: String,
    /// Every run of [`IN_FLIGHT`], without its jobs, which [`supervise`] reads only for the
    /// runs a rule could act on. A duplicate is judged against an older run in flight only, so
    /// no completed run is read.
    pub runs: Vec<RunObservation>,
    /// Each open pull request's head.
    pub pr_heads: BTreeMap<u64, String>,
    /// The open pull requests labelled [`FULL_LABEL`].
    #[serde(default)]
    pub full: BTreeSet<u64>,
    /// Each open pull request's head branch: a recovered head's runs are its branch's.
    #[serde(default)]
    pub pr_branches: BTreeMap<u64, String>,
    /// The open pull requests labelled [`YIELDED_LABEL`]: their heads are the release path's to
    /// give back, so recovery reads none of them and no paired re-run is asked for one.
    #[serde(default)]
    pub yielded: BTreeSet<u64>,
}

/// A source of run observations and the one act on them. The command line uses
/// [`GhRunForge`]; tests use a value.
pub trait RunForge {
    /// Observe every in-flight run and every open pull request's head, without jobs.
    fn observe(&self) -> Result<RunsObservation, ForgeError>;
    /// Read one run's jobs, its latest attempt's.
    fn jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError>;
    /// Read one run again, with its jobs.
    fn run(&self, id: u64) -> Result<RunObservation, ForgeError>;
    /// Read one pull request's head again; `None` when it is no longer open.
    fn pr_head(&self, number: u64) -> Result<Option<String>, ForgeError>;
    /// End one run at once.
    fn force_cancel(&mut self, id: u64) -> Result<(), ForgeError>;
    /// Run again a run just ended: once it reads completed, ask for its re-run, every job of
    /// it, unless it completed with a verdict ([`Rerun::Decided`]). Err when it did not end in
    /// time or the forge refused.
    fn rerun(&mut self, id: u64) -> Result<Rerun, ForgeError>;
    /// Every run of commit `sha`, completed ones among them, without jobs: whether a head has a
    /// run in flight, a verdict, or neither ([`recover`]).
    fn runs_at(&self, sha: &str) -> Result<Vec<RunObservation>, ForgeError>;
    /// The pull requests' runs that completed `cancelled` and were created at or after
    /// `created_since` (seconds since the epoch), newest first, without jobs: the heads a pass
    /// reads first for recovery ([`heads_to_read`]).
    fn cancelled_since(&self, created_since: u64) -> Result<Vec<RunObservation>, ForgeError>;
    /// How many times run `run` of #`pr` yielded to the release path: the notes
    /// `scripts/ci/release-path` left on the pull request when it labelled it and cancelled the
    /// run ([`YIELD_NOTE`]), each followed by the attempt `resume` asked. Read only for a run
    /// at [`MAX_ATTEMPTS`] or past it: those attempts are the release path's, not this
    /// supervisor's, and are not counted against the bound ([`within_attempts`]).
    fn yields(&self, pr: u64, run: u64) -> Result<u64, ForgeError>;
    /// When the newest successful job of `workflow` ended, RFC 3339; `None` when there is none
    /// among its recent runs, or none of them `among` admits.
    fn newest_success(
        &self,
        workflow: &str,
        among: Successes<'_>,
    ) -> Result<Option<String>, ForgeError>;
}

/// What [`RunForge::rerun`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rerun {
    /// The re-run was asked.
    Asked,
    /// The run read completed with a verdict: `success`, `failure` or `timed_out`, or
    /// `cancelled` with a verdict in its jobs ([`RunObservation::verdict_in_jobs`]: it went red
    /// and cancelled itself, or its `ci` was written after a cancel that let it run). Its head
    /// has it, and nothing was asked, since a re-run would write a newer `ci` over it.
    Decided,
}

/// Which successful jobs [`RunForge::newest_success`] counts; the default counts every one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Successes<'a> {
    /// Only a job of this lane: one named `<anything> (<lane>)`, as a matrix job of
    /// `name: supervise (${{ matrix.lane }})` is.
    pub lane: Option<&'a str>,
    /// Only among the runs last updated at or after this time, in seconds since the epoch. A
    /// job that ended then belongs to a run updated no earlier, so the other runs' jobs need
    /// not be read: a lane that may have stopped long ago costs one page, not twenty.
    pub since: Option<u64>,
}

/// The workflow key of a run: its file name without the extension
/// (`.github/workflows/validate.yml` is `validate`); a dynamic workflow's last segment.
pub fn workflow_of(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    last.strip_suffix(".yml")
        .or_else(|| last.strip_suffix(".yaml"))
        .unwrap_or(last)
        .to_string()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// One job, from the forge's `jobs[]` entry.
pub fn job_of(v: &Value) -> Option<JobObservation> {
    Some(JobObservation {
        name: text(v, "name").unwrap_or_default(),
        status: text(v, "status")?,
        started_at: text(v, "started_at"),
        created_at: text(v, "created_at"),
        conclusion: text(v, "conclusion"),
    })
}

/// The pull request a run belongs to, among the forge's `pull_requests` of it: the one whose
/// head is the branch the run ran on, from the repository it ran from. The forge lists every
/// open pull request whose head matches the run's sha or branch, so a train opened at its last
/// member's tip lists the member too, and the first entry is not the run's own; `None` when no
/// entry is, and [`attach_pull_requests`] then reads the branch.
fn pull_request_of(v: &Value, branch: &str) -> Option<u64> {
    let repo = v.pointer("/head_repository/id").and_then(Value::as_u64);
    v.get("pull_requests")
        .and_then(Value::as_array)?
        .iter()
        .find(|p| {
            p.pointer("/head/ref").and_then(Value::as_str) == Some(branch)
                && match (repo, p.pointer("/head/repo/id").and_then(Value::as_u64)) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                }
        })
        .and_then(|p| p.get("number"))
        .and_then(Value::as_u64)
}

/// One run, from the forge's `workflow_runs[]` entry; its jobs are read separately.
pub fn run_of(v: &Value) -> Option<RunObservation> {
    let head_branch = text(v, "head_branch").unwrap_or_default();
    Some(RunObservation {
        id: v.get("id").and_then(Value::as_u64)?,
        workflow: workflow_of(&text(v, "path").or_else(|| text(v, "name"))?),
        event: text(v, "event")?,
        pull_request: pull_request_of(v, &head_branch),
        head_branch,
        head_sha: text(v, "head_sha")?,
        status: text(v, "status")?,
        conclusion: text(v, "conclusion"),
        created_at: text(v, "created_at")?,
        run_started_at: text(v, "run_started_at"),
        run_attempt: v
            .get("run_attempt")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
            .unwrap_or(1),
        updated_at: text(v, "updated_at"),
        jobs: Vec::new(),
    })
}

/// The lines of a `gh api --paginate --jq '.x[]'` answer, each a JSON value.
fn lines_of(out: &str, what: &str) -> Result<Vec<Value>, ForgeError> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| ForgeError(format!("{what}: {e}: {l}"))))
        .collect()
}

/// A pull request's run whose `pull_requests` the forge left empty (a fork's, or a branch's
/// whose pull request was opened after the run) is given the open pull request whose head
/// branch it ran on.
pub fn attach_pull_requests(runs: &mut [RunObservation], branches: &BTreeMap<String, u64>) {
    for r in runs.iter_mut() {
        if r.pull_request.is_none() && r.event == "pull_request" {
            r.pull_request = branches.get(&r.head_branch).copied();
        }
    }
}

/// The longest one `gh` call may take before it is ended as timed out.
pub const GH_CALL_SECONDS: u64 = 20;

/// The longest the forge may be read for, from [`GhRunForge::connect`]: the supervise step is
/// bounded at four minutes of its own, so that the budget ends first and no act is cut off.
pub const BUDGET_SECONDS: u64 = 180;

/// How many times a run just force-cancelled is read for its end before its re-run is asked.
pub const RERUN_READS: u32 = 10;

/// How long apart those reads are: ten of them are half a minute, inside the forge's budget.
pub const RERUN_WAIT_SECONDS: u64 = 3;

/// GitHub's own `run_attempt` past which a run is not asked to run again: a re-run is asked only
/// of a run whose attempt is below it, so a head gets two re-runs at most, the one paired with a
/// cancel or a later pass's. The forge counts them, so every runner and lane reads the same
/// count, and a head whose newest run reached it is held as `head_exhausted`.
pub const MAX_ATTEMPTS: u64 = 3;

/// How long after its newest run was cancelled a head is still recovered, in minutes. A cancel
/// older than this was no pass's transient: a live supervisor passes every ten minutes, and the
/// queue reports one that has not for thirty. On 2026-10-29 at 13:42Z, 93 of the 127 open pull
/// requests' heads had only cancelled validate runs, most cancelled days before, and a recovery
/// without a window would have asked every one of them again at once.
pub const RECOVERY_WINDOW_MINUTES: u64 = 360;

/// How far back the listing of cancelled runs reaches, by the runs' creation, in minutes: the
/// window and a day, the longest a validate run lives from its creation to its end (240 minutes
/// queued, a chain of jobs that took 579 minutes in 40576851731). The listing orders by
/// creation; a run re-run long after it was created is found by [`ROTATING_READS`] instead.
pub const CANCELLED_REACH_MINUTES: u64 = RECOVERY_WINDOW_MINUTES + 24 * 60;

/// How many pages of that listing, a hundred runs each, are read at most: a day of cancels on
/// 2026-10-29 was one page.
pub const CANCELLED_PAGES: u32 = 3;

/// How many heads with a run cancelled within the window one pass reads whole, at most.
pub const RECOVERY_READS: usize = 12;

/// How many of the other heads without a run in flight one pass reads besides, a different few
/// every ten minutes: with 120 of them, each is read within five hours, inside the window.
pub const ROTATING_READS: usize = 4;

/// How many runs' jobs one pass reads, at most, for a verdict a cancelled run carries
/// ([`RunObservation::verdict_in_jobs`]). The run listing says `cancelled` of a run that went
/// red and cancelled itself (I2664), and only its jobs tell; a twin cancelled while it waited in
/// its group has no job at all, so a head's cancelled runs are read newest first until one shows
/// a verdict or none is left, past every such twin. Only a head the runs alone would ask again or
/// hold is read, usually with one request; a head whose reads this would pass is not asked in
/// this pass, said in `unread`, and read by the next.
pub const VERDICT_READS: usize = 24;

/// The words `scripts/ci/release-path` opens its note with when a validate run yields, followed
/// by the run's id; [`RunForge::yields`] counts the notes that open with them.
pub const YIELD_NOTE: &str = "Validate run ";

/// What follows the run's id in that note.
pub const YIELD_NOTE_AFTER: &str = " of this pull request yielded to the release path";

/// How many pages of a pull request's comments, a hundred each, are read for its yield notes.
pub const YIELD_PAGES: u32 = 3;

/// How many re-runs one pass asks for recovery, at most: a burst of cancels re-runs a few heads
/// a pass, not the queue's worth at once.
pub const RECOVERY_RERUNS: usize = 3;

/// How many pages of the supervising workflow's runs, a hundred each, are read for its newest
/// success: a day of them at a request every few minutes, newest first.
pub const SUCCESS_PAGES: u32 = 3;

/// How many of those runs' jobs are read for it at most. A burst of requests leaves runs whose
/// every job was cancelled while it waited, and they are read past; more runs than this inside
/// the window with no success among them is an answer that could not be read, not a stale one.
pub const SUCCESS_JOB_READS: usize = 40;

/// The GitHub CLI, run in the repository.
pub struct GhRunForge<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// `owner/name`.
    pub repository: String,
    /// Its default branch.
    pub default_branch: String,
    /// After when no call is started: [`BUDGET_SECONDS`] from connecting.
    pub deadline: std::time::Instant,
}

/// One `gh` call within `deadline`, at most [`GH_CALL_SECONDS`]; with `retrying`, asked
/// again on a transient failure, a timeout among them, while the budget lasts.
fn call(
    root: &Path,
    deadline: std::time::Instant,
    args: &[&str],
    retrying: bool,
) -> Result<(bool, String, String), ForgeError> {
    let once = || {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err(ForgeError(format!(
                "the forge's budget of {BUDGET_SECONDS} s is spent; gh {} was not asked",
                args.join(" ")
            )));
        }
        gh_within(
            root,
            args,
            left.min(std::time::Duration::from_secs(GH_CALL_SECONDS)),
        )
    };
    if !retrying {
        return once();
    }
    super::retry::forge(|| {
        let (ok, out, err) = once().map_err(|e| e.0)?;
        if !ok && super::retry::transient(&err) {
            Err(format!("gh {} failed: {}", args.join(" "), err.trim()))
        } else {
            Ok((ok, out, err))
        }
    })
    .map_err(ForgeError)
}

impl<'a> GhRunForge<'a> {
    /// Ask the forge which repository `root` is; the budget starts now.
    pub fn connect(root: &'a Path) -> Result<Self, ForgeError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(BUDGET_SECONDS);
        let (ok, out, err) = call(
            root,
            deadline,
            &["repo", "view", "--json", "nameWithOwner,defaultBranchRef"],
            true,
        )?;
        if !ok {
            return Err(ForgeError(format!("gh repo view failed: {}", err.trim())));
        }
        let v: Value =
            serde_json::from_str(&out).map_err(|e| ForgeError(format!("gh repo view: {e}")))?;
        Ok(GhRunForge {
            root,
            repository: text(&v, "nameWithOwner")
                .ok_or_else(|| ForgeError("gh repo view named no repository".into()))?,
            default_branch: v
                .pointer("/defaultBranchRef/name")
                .and_then(Value::as_str)
                .ok_or_else(|| ForgeError("gh repo view named no default branch".into()))?
                .to_string(),
            deadline,
        })
    }

    fn read(&self, args: &[&str]) -> Result<(bool, String, String), ForgeError> {
        call(self.root, self.deadline, args, true)
    }

    fn paged(&self, endpoint: &str, jq: &str) -> Result<Vec<Value>, ForgeError> {
        let args = ["api", "--paginate", endpoint, "--jq", jq];
        let (ok, out, err) = self.read(&args)?;
        if !ok {
            return Err(ForgeError(format!(
                "gh api {endpoint} failed: {}",
                err.trim()
            )));
        }
        lines_of(&out, endpoint)
    }

    fn one(&self, endpoint: &str) -> Result<Value, ForgeError> {
        let (ok, out, err) = self.read(&["api", endpoint])?;
        if !ok {
            return Err(ForgeError(format!(
                "gh api {endpoint} failed: {}",
                err.trim()
            )));
        }
        serde_json::from_str(&out).map_err(|e| ForgeError(format!("{endpoint}: {e}")))
    }

    fn jobs_raw(&self, id: u64) -> Result<Vec<Value>, ForgeError> {
        let endpoint = format!(
            "repos/{}/actions/runs/{id}/jobs?per_page=100",
            self.repository
        );
        self.paged(&endpoint, ".jobs[]")
    }

    fn read_jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError> {
        Ok(self.jobs_raw(id)?.iter().filter_map(job_of).collect())
    }
}

impl RunForge for GhRunForge<'_> {
    fn observe(&self) -> Result<RunsObservation, ForgeError> {
        let mut by_id = BTreeMap::new();
        for status in IN_FLIGHT {
            let endpoint = format!(
                "repos/{}/actions/runs?status={status}&per_page=100",
                self.repository
            );
            for v in self.paged(&endpoint, ".workflow_runs[]")? {
                let run = run_of(&v).ok_or_else(|| {
                    ForgeError(format!("{endpoint}: a run this cannot read: {v}"))
                })?;
                by_id.insert(run.id, run);
            }
        }
        let endpoint = format!("repos/{}/pulls?state=open&per_page=100", self.repository);
        let mut pr_heads = BTreeMap::new();
        let mut branches = BTreeMap::new();
        let mut pr_branches = BTreeMap::new();
        let mut full = BTreeSet::new();
        let mut yielded = BTreeSet::new();
        for v in self.paged(
            &endpoint,
            ".[] | {number: .number, sha: .head.sha, branch: .head.ref, labels: [.labels[].name]}",
        )? {
            let (Some(n), Some(sha)) = (v.get("number").and_then(Value::as_u64), text(&v, "sha"))
            else {
                return Err(ForgeError(format!(
                    "{endpoint}: a pull request without a head: {v}"
                )));
            };
            if let Some(b) = text(&v, "branch") {
                pr_branches.insert(n, b.clone());
                branches.insert(b, n);
            }
            let labelled = |label: &str| {
                v.get("labels")
                    .and_then(Value::as_array)
                    .is_some_and(|a| a.iter().any(|l| l.as_str() == Some(label)))
            };
            if labelled(FULL_LABEL) {
                full.insert(n);
            }
            if labelled(YIELDED_LABEL) {
                yielded.insert(n);
            }
            pr_heads.insert(n, sha);
        }
        let mut runs: Vec<RunObservation> = by_id.into_values().collect();
        attach_pull_requests(&mut runs, &branches);
        Ok(RunsObservation {
            repository: self.repository.clone(),
            default_branch: self.default_branch.clone(),
            runs,
            pr_heads,
            full,
            pr_branches,
            yielded,
        })
    }

    fn jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError> {
        self.read_jobs(id)
    }

    fn run(&self, id: u64) -> Result<RunObservation, ForgeError> {
        let endpoint = format!("repos/{}/actions/runs/{id}", self.repository);
        let (ok, out, err) = self.read(&["api", &endpoint])?;
        if !ok {
            return Err(ForgeError(format!(
                "gh api {endpoint} failed: {}",
                err.trim()
            )));
        }
        let v: Value =
            serde_json::from_str(&out).map_err(|e| ForgeError(format!("{endpoint}: {e}")))?;
        let mut run =
            run_of(&v).ok_or_else(|| ForgeError(format!("{endpoint}: a run this cannot read")))?;
        run.jobs = self.read_jobs(id)?;
        Ok(run)
    }

    fn pr_head(&self, number: u64) -> Result<Option<String>, ForgeError> {
        let endpoint = format!("repos/{}/pulls/{number}", self.repository);
        let (ok, out, err) = self.read(&["api", &endpoint])?;
        if !ok {
            return Err(ForgeError(format!(
                "gh api {endpoint} failed: {}",
                err.trim()
            )));
        }
        let v: Value =
            serde_json::from_str(&out).map_err(|e| ForgeError(format!("{endpoint}: {e}")))?;
        if v.get("state").and_then(Value::as_str) != Some("open") {
            return Ok(None);
        }
        Ok(v.pointer("/head/sha")
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    fn force_cancel(&mut self, id: u64) -> Result<(), ForgeError> {
        // the one write: asked once, never retried — the next tick observes again
        let endpoint = format!("repos/{}/actions/runs/{id}/force-cancel", self.repository);
        let (ok, _, err) = call(
            self.root,
            self.deadline,
            &["api", "-X", "POST", &endpoint],
            false,
        )?;
        if ok {
            Ok(())
        } else {
            Err(ForgeError(format!(
                "gh api -X POST {endpoint} failed: {}",
                err.trim()
            )))
        }
    }

    fn rerun(&mut self, id: u64) -> Result<Rerun, ForgeError> {
        // a force-cancel is accepted before the run has ended, and the forge re-runs only a run
        // that has: read it until it reads completed, then ask once, never again; a run that
        // reached a verdict before its cancel did has it, and is not asked, and neither is one
        // that reads `cancelled` with its verdict in its jobs (one more read, of a cancelled run
        // only: it went red and cancelled itself, or its `ci` ran after a cancel)
        let endpoint = format!("repos/{}/actions/runs/{id}", self.repository);
        let mut ended = None;
        for read in 0..RERUN_READS {
            if read > 0 {
                std::thread::sleep(std::time::Duration::from_secs(RERUN_WAIT_SECONDS));
            }
            let v = self.one(&endpoint)?;
            if v.get("status").and_then(Value::as_str) == Some("completed") {
                ended = Some(v);
                break;
            }
        }
        let Some(v) = ended else {
            return Err(ForgeError(format!(
                "run {id} still read in flight {RERUN_READS} times after its cancel; its re-run \
                 was not asked"
            )));
        };
        let mut run =
            run_of(&v).ok_or_else(|| ForgeError(format!("{endpoint}: a run this cannot read")))?;
        if run.conclusion.as_deref() == Some("cancelled") {
            run.jobs = self.read_jobs(id)?;
        }
        if run.verdict_written() {
            return Ok(Rerun::Decided);
        }
        let again = format!("{endpoint}/rerun");
        let (ok, _, err) = call(
            self.root,
            self.deadline,
            &["api", "-X", "POST", &again],
            false,
        )?;
        if ok {
            Ok(Rerun::Asked)
        } else {
            Err(ForgeError(format!(
                "gh api -X POST {again} failed: {}",
                err.trim()
            )))
        }
    }

    fn runs_at(&self, sha: &str) -> Result<Vec<RunObservation>, ForgeError> {
        let endpoint = format!(
            "repos/{}/actions/runs?head_sha={sha}&per_page=100",
            self.repository
        );
        self.paged(&endpoint, ".workflow_runs[]")?
            .iter()
            .map(|v| {
                run_of(v)
                    .ok_or_else(|| ForgeError(format!("{endpoint}: a run this cannot read: {v}")))
            })
            .collect()
    }

    fn cancelled_since(&self, created_since: u64) -> Result<Vec<RunObservation>, ForgeError> {
        // newest first, page by page, never past CANCELLED_PAGES: the creation filter bounds it,
        // and the pages bound a day of cancels that filled it
        let since = crate::peers::rfc3339(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(created_since),
        );
        let mut out = Vec::new();
        for page in 1..=CANCELLED_PAGES {
            let endpoint = format!(
                "repos/{}/actions/runs?status=cancelled&event=pull_request&per_page=100\
                 &page={page}&created=%3E%3D{since}",
                self.repository
            );
            let listed = self.one(&endpoint)?;
            let runs = listed
                .get("workflow_runs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for v in &runs {
                out.push(run_of(v).ok_or_else(|| {
                    ForgeError(format!("{endpoint}: a run this cannot read: {v}"))
                })?);
            }
            if runs.len() < 100 {
                break;
            }
        }
        Ok(out)
    }

    fn yields(&self, pr: u64, run: u64) -> Result<u64, ForgeError> {
        // the pull request's comments, oldest first, never past YIELD_PAGES
        let mut n = 0;
        for page in 1..=YIELD_PAGES {
            let endpoint = format!(
                "repos/{}/issues/{pr}/comments?per_page=100&page={page}",
                self.repository
            );
            let listed = self.one(&endpoint)?;
            let notes = listed.as_array().cloned().unwrap_or_default();
            n += notes
                .iter()
                .filter(|c| text(c, "body").is_some_and(|b| is_yield_note(&b, run)))
                .count() as u64;
            if notes.len() < 100 {
                break;
            }
        }
        Ok(n)
    }

    fn newest_success(
        &self,
        workflow: &str,
        among: Successes<'_>,
    ) -> Result<Option<String>, ForgeError> {
        // its runs newest first, page by page: a run whose other lane waits for a runner has
        // not concluded, so the success is its job's, not the run's; and a run whose jobs were
        // all cancelled while they waited is read past, not taken for the end
        let mut read = 0;
        for page in 1..=SUCCESS_PAGES {
            let endpoint = format!(
                "repos/{}/actions/workflows/{workflow}.yml/runs?per_page=100&page={page}\
                 &exclude_pull_requests=true",
                self.repository
            );
            let listed = self.one(&endpoint)?;
            let ids = runs_updated_since(&listed, among.since);
            for id in ids {
                if read == SUCCESS_JOB_READS {
                    return Err(ForgeError(format!(
                        "{SUCCESS_JOB_READS} runs of {workflow} were read with no successful \
                         job{} among them",
                        among.lane.map(|l| format!(" of {l}")).unwrap_or_default()
                    )));
                }
                read += 1;
                let newest = newest_success_of(&self.jobs_raw(id)?, among.lane);
                if newest.is_some() {
                    return Ok(newest);
                }
            }
            let listed_here = listed
                .get("workflow_runs")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if listed_here < 100 {
                break;
            }
        }
        Ok(None)
    }
}

/// Whether a comment's `body` is the release path's note that run `run` yielded to it.
pub fn is_yield_note(body: &str, run: u64) -> bool {
    body.strip_prefix(YIELD_NOTE)
        .and_then(|rest| rest.strip_prefix(run.to_string().as_str()))
        .is_some_and(|rest| rest.starts_with(YIELD_NOTE_AFTER))
}

/// Whether a run at GitHub's `run_attempt` `attempt` may be asked to run again: the attempts the
/// release path took back (`yields`, [`RunForge::yields`]) are not this supervisor's, so they do
/// not count against [`MAX_ATTEMPTS`]. Counting them would let a head that yielded twice reach
/// the bound on its first hang and stay without a run, which is the invariant broken.
pub fn within_attempts(attempt: u64, yields: u64) -> bool {
    attempt.saturating_sub(yields) < MAX_ATTEMPTS
}

/// The ids of a page of runs, in its order; with `since`, only the runs last updated at or
/// after it (a run whose update is not a time is read).
fn runs_updated_since(page: &Value, since: Option<u64>) -> Vec<u64> {
    page.get("workflow_runs")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|r| match since {
                    Some(t) => text(r, "updated_at")
                        .and_then(|u| crate::peers::parse_rfc3339(&u))
                        .is_none_or(|u| u >= t),
                    None => true,
                })
                .filter_map(|r| r.get("id")?.as_u64())
                .collect()
        })
        .unwrap_or_default()
}

/// When the newest successful job among `jobs` ended; with `lane`, only a job named
/// `<anything> (<lane>)` counts.
fn newest_success_of(jobs: &[Value], lane: Option<&str>) -> Option<String> {
    let suffix = lane.map(|l| format!("({l})"));
    jobs.iter()
        .filter(|j| j.get("conclusion").and_then(Value::as_str) == Some("success"))
        .filter(|j| match &suffix {
            Some(s) => text(j, "name").is_some_and(|n| n.ends_with(s.as_str())),
            None => true,
        })
        .filter_map(|j| text(j, "completed_at"))
        .max()
}

/// What became of one decision under `--apply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActOutcome {
    /// The run was force-cancelled.
    Cancelled,
    /// The run was force-cancelled and asked to run again: the last run of its pull request's
    /// head, with a job past its bound ([`super::supervise::Decision::rerun`]).
    CancelledAndRerun,
    /// The run was force-cancelled and its re-run could not be asked: its pull request's head
    /// has no run until somebody re-runs it.
    RerunFailed,
    /// Read again, the decision no longer held: nothing was done.
    Skipped,
    /// The run or its pull request could not be read again: nothing was done.
    Unobserved,
    /// The trail could not record the act: nothing was done, and no other act was taken.
    Unrecorded,
    /// The forge refused the cancel.
    Failed,
}

/// What became of a head found with no run, on a pass after the one that left it so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOutcome {
    /// Its newest run was asked to run again.
    Rerun,
    /// Its newest run read completed with a verdict when it was to be asked: nothing was asked.
    Decided,
    /// The forge refused the re-run: the head still has no run, and the next pass asks again.
    /// A warning naming the pull request, never a failed pass: a refusal that persists comes
    /// back on every pass, and the pass whose act bared the head was the red one.
    RerunFailed,
    /// This pass had asked [`RECOVERY_RERUNS`] re-runs already: the next pass asks.
    Deferred,
    /// The trail could not record the attempt, or its outcome: a failed pass, since a trail that
    /// refuses writes is this supervisor's own fault.
    Unrecorded,
}

/// An open pull request's head with no run in flight and no verdict, whose newest run was
/// cancelled within [`RECOVERY_WINDOW_MINUTES`] at an attempt below [`MAX_ATTEMPTS`], and what
/// this pass did for it ([`recover`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recovery {
    /// The pull request.
    pub pull_request: u64,
    /// Its head.
    pub head_sha: String,
    /// The head's newest run.
    pub run: u64,
    /// The attempt of it this asked for: its `run_attempt` and one.
    pub attempt: u64,
    /// What became of it.
    pub outcome: RecoveryOutcome,
    /// What happened, for a person.
    pub detail: String,
}

/// One act, or one act not taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Act {
    /// The run.
    pub run: u64,
    /// The reason it was decided on, as a word.
    pub reason: String,
    /// What became of it.
    pub outcome: ActOutcome,
    /// What happened, for a person.
    pub detail: String,
}

/// What `prs supervise` found and did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SuperviseReport {
    /// `owner/name`.
    pub repository: String,
    /// When it observed, RFC 3339.
    pub observed_at: String,
    /// Whether it acted (`--apply`).
    pub applied: bool,
    /// How many in-flight runs it judged.
    pub in_flight: usize,
    /// The runs to cancel, with their reasons.
    pub decisions: Vec<Decision>,
    /// The runs left alone on purpose, with their reasons.
    pub held: Vec<Hold>,
    /// Under `--apply`, what became of each decision.
    #[serde(default)]
    pub acts: Vec<Act>,
    /// Under `--apply`, the heads found with no run in flight and no verdict whose newest run
    /// was cancelled, by anyone, within the window, and what was done for each ([`recover`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovered: Vec<Recovery>,
    /// Under `--apply`, what recovery could not read, each for a person: a head's runs, or the
    /// listing of cancelled runs. A warning, never a failed pass: the next pass reads again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unread: Vec<String>,
    /// How many runs' jobs were read: only those a rule could act on.
    #[serde(default)]
    pub jobs_read: usize,
    /// Why nothing was observed: a supervision succeeded within the minutes
    /// `--unless-supervised-within` allows, and this one stood aside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Whether the supervisor itself is alive; a `finding` here (`supervise_stale`,
    /// `supervise_unobserved`) is what the queue workflow fails on. It never changes the exit:
    /// the run that reports it is not the one that stopped.
    #[serde(default)]
    pub liveness: Liveness,
}

impl SuperviseReport {
    /// 0 when nothing needed doing or every act succeeded or was rightly skipped; 10 for
    /// decisions a dry run found; 12 when an act of this pass could not be taken, its paired
    /// re-run among them (each names the pull request), or the trail refused a write. What
    /// recovery finds is a pull request's state, not this pass's: a head held as
    /// `head_exhausted`, a head or listing it could not read, and a recovery re-run the forge
    /// refused never make it 12. They are warnings, reported on every pass that finds them, since
    /// a lane that failed on every pass for one stuck pull request would read as a supervisor
    /// that stopped (the hosted lane would stop standing aside, the queue would report
    /// `supervise_stale`), and which pass found one first is nothing the forge says. The pass
    /// whose own act bared a head and could not give it its run back is the one that is red.
    pub fn exit_code(&self) -> u8 {
        if !self.applied {
            return if self.decisions.is_empty() { 0 } else { 10 };
        }
        let acts = self.acts.iter().all(|a| {
            matches!(
                a.outcome,
                ActOutcome::Cancelled | ActOutcome::CancelledAndRerun | ActOutcome::Skipped
            )
        });
        let recovered = self
            .recovered
            .iter()
            .all(|r| r.outcome != RecoveryOutcome::Unrecorded);
        if acts && recovered {
            0
        } else {
            12
        }
    }
}

/// What a paired cancel's `run_cancel_attempted` says after its reason, for a person reading the
/// trail; nothing reads it back.
const PAIRED: &str = "; the last run of its pull request's head, so it is run again once ended";

fn subject(d: &Decision) -> String {
    format!(
        "run {} of {} ({} on {}): {}: {}{}",
        d.run,
        d.workflow,
        d.event,
        d.head_branch,
        d.reason.word(),
        d.reason.explain(),
        if d.rerun { PAIRED } else { "" }
    )
}

fn run_event(
    action: IntegrationAction,
    pr: Option<u64>,
    head_sha: &str,
    run: u64,
    detail: String,
) -> IntegrationEvent {
    IntegrationEvent {
        pr,
        head_sha: Some(head_sha.to_string()),
        run: Some(run),
        detail,
        ..IntegrationEvent::of(action)
    }
}

fn event(action: IntegrationAction, d: &Decision, detail: String) -> IntegrationEvent {
    run_event(action, d.pull_request, &d.head_sha, d.run, detail)
}

/// What became of one re-run asked by [`ask_rerun`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    /// Asked: the head has its run again.
    Rerun,
    /// The run had a verdict: nothing was asked.
    Decided,
    /// Not asked, or refused: the head has no run.
    Failed,
}

/// Ask run `run` of #`pr` at `head_sha` to run again, as attempt `attempt` of at most `bound`
/// ([`MAX_ATTEMPTS`] and the attempts the release path took, [`within_attempts`]): on the trail
/// before
/// it is asked (`run_rerun_attempted`), or not asked, and its outcome after it
/// (`run_rerun_requested`, or `run_rerun_failed` naming the pull request). A run asked again
/// reads as requested in `current`. The words for the act's detail come back with the outcome.
#[allow(clippy::too_many_arguments)]
fn ask_rerun(
    root: &Path,
    forge: &mut dyn RunForge,
    pr: Option<u64>,
    head_sha: &str,
    run: u64,
    workflow: &str,
    (attempt, bound): (u64, u64),
    current: &mut [RunObservation],
    trail_broken: &mut bool,
) -> (Asked, String) {
    let n = pr.unwrap_or_default();
    let what = format!("run {run} of {workflow}, #{n}'s at {head_sha}");
    if let Err(e) = drain::record(
        root,
        run_event(
            IntegrationAction::RunRerunAttempted,
            pr,
            head_sha,
            run,
            format!("attempt {attempt} of at most {bound} of {what}"),
        ),
    ) {
        *trail_broken = true;
        return (
            Asked::Failed,
            format!(
                "did not re-run {what}: the trail could not record it, and #{n} has no run of \
                 its head: {e}"
            ),
        );
    }
    let (action, asked, said) = match forge.rerun(run) {
        Ok(Rerun::Asked) => {
            if let Some(r) = current.iter_mut().find(|r| r.id == run) {
                r.status = "requested".into();
                r.conclusion = None;
                r.run_attempt += 1;
                r.jobs.clear();
            }
            (
                IntegrationAction::RunRerunRequested,
                Asked::Rerun,
                format!("re-requested {what} (attempt {attempt} of at most {bound})"),
            )
        }
        Ok(Rerun::Decided) => (
            IntegrationAction::RunRerunRequested,
            Asked::Decided,
            format!("did not re-run {what}: it has its verdict, which a re-run would write over"),
        ),
        Err(e) => (
            IntegrationAction::RunRerunFailed,
            Asked::Failed,
            format!(
                "could not re-run {what} (attempt {attempt} of at most {bound}), and #{n} has no \
                 run of its head: {e}"
            ),
        ),
    };
    match drain::record(root, run_event(action, pr, head_sha, run, said.clone())) {
        Ok(_) => (asked, said),
        Err(e) => {
            *trail_broken = true;
            (
                asked,
                format!("{said} (the trail could not record it: {e})"),
            )
        }
    }
}

/// The runs of `runs` that judge #`pr`'s head for `workflow`: its `pull_request` runs of the
/// pull request's branch, or, when the branch is not known, the runs the forge attributed to it.
fn of_head<'a>(
    runs: &'a [RunObservation],
    workflow: &str,
    pr: u64,
    branch: Option<&str>,
) -> Vec<&'a RunObservation> {
    runs.iter()
        .filter(|r| r.workflow == workflow && r.event == "pull_request")
        .filter(|r| match branch {
            Some(b) => r.head_branch == b,
            None => r.pull_request == Some(pr),
        })
        .collect()
}

/// Read the jobs of the runs `ids` of `runs`, newest first as given, into them, until one shows a
/// verdict ([`RunObservation::verdict_in_jobs`]) or none is left, each read taken from `reads`,
/// the pass's [`VERDICT_READS`]. `Ok(true)` when a verdict was found or every one was read,
/// `Ok(false)` when `reads` ran out first with no verdict among those read. Err when a read
/// failed.
fn read_verdicts(
    forge: &dyn RunForge,
    runs: &mut [RunObservation],
    ids: &[u64],
    reads: &mut usize,
) -> Result<bool, ForgeError> {
    for id in ids {
        if *reads == 0 {
            return Ok(false);
        }
        *reads -= 1;
        let jobs = forge.jobs(*id)?;
        if let Some(r) = runs.iter_mut().find(|r| r.id == *id) {
            r.jobs = jobs;
            if r.verdict_in_jobs() {
                return Ok(true);
            }
        }
    }
    Ok(true)
}

/// The completed `cancelled` runs among `runs`, newest first by the attempt's start, then by id:
/// the runs whose verdict, if any, is in their jobs.
fn cancelled_newest_first(runs: &[&RunObservation]) -> Vec<u64> {
    runs.iter()
        .filter(|r| r.status == "completed" && r.conclusion.as_deref() == Some("cancelled"))
        .map(|r| std::cmp::Reverse((r.run_started_at.clone().unwrap_or_default(), r.id)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|std::cmp::Reverse((_, id))| id)
        .collect()
}

/// What one workflow's runs of a head say it needs ([`head_need`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadNeed {
    /// A run of it is in flight, or one has its verdict: nothing.
    Covered,
    /// It has no run, or its newest did not end `cancelled` (it was skipped, or awaits an
    /// approval), or it ended more than [`RECOVERY_WINDOW_MINUTES`] ago: nobody's transient, and
    /// nothing is asked.
    Settled,
    /// Its newest run, `run`, ended `cancelled` within the window at attempt `attempts`, at or
    /// past [`MAX_ATTEMPTS`]: held as `head_exhausted`.
    Exhausted {
        /// The run.
        run: u64,
        /// Its `run_attempt`.
        attempts: u64,
    },
    /// Its newest run, `run`, ended `cancelled` within the window at attempt `attempts`, below
    /// [`MAX_ATTEMPTS`]: asked to run again.
    Rerun {
        /// The run.
        run: u64,
        /// Its `run_attempt`.
        attempts: u64,
    },
}

/// What a head needs, from one workflow's runs of it, completed ones among them, at `now`
/// (seconds since the epoch). Pure: judged from the runs the forge lists, whoever cancelled
/// them, and never from a trail. The newest is the one whose attempt started last. A run that
/// reads `cancelled` is a verdict when its jobs, where they were read, show one
/// ([`RunObservation::verdict_written`]); [`recover`] reads them before it asks anything.
pub fn head_need(runs: &[&RunObservation], now: u64) -> HeadNeed {
    if runs.iter().any(|r| r.in_flight() || r.verdict_written()) {
        return HeadNeed::Covered;
    }
    let Some(newest) = runs
        .iter()
        .max_by_key(|r| (r.run_started_at.clone().unwrap_or_default(), r.id))
    else {
        return HeadNeed::Settled;
    };
    let recent = newest
        .ended_at()
        .is_some_and(|t| t.saturating_add(RECOVERY_WINDOW_MINUTES * 60) >= now);
    if newest.conclusion.as_deref() != Some("cancelled") || !recent {
        return HeadNeed::Settled;
    }
    if newest.run_attempt >= MAX_ATTEMPTS {
        HeadNeed::Exhausted {
            run: newest.id,
            attempts: newest.run_attempt,
        }
    } else {
        HeadNeed::Rerun {
            run: newest.id,
            attempts: newest.run_attempt,
        }
    }
}

/// The open pull requests' heads a pass reads whole for recovery, in order: those of `obs` with
/// no `pull_request` run of a bounded workflow in flight in `current` (which is also not among
/// `skip`, the pull requests this pass acted on, nor labelled [`YIELDED_LABEL`], whose heads the
/// release path gives back), first the ones a run in `cancelled` names,
/// cancelled within the window, newest cancel first, at most [`RECOVERY_READS`]; then
/// [`ROTATING_READS`] of the rest, by pull request, from an offset the clock moves every ten
/// minutes. Stateless: two runners at the same ten minutes read the same heads, and every head
/// is read within `rest / ROTATING_READS` ten-minute slots.
pub fn heads_to_read(
    obs: &RunsObservation,
    current: &[RunObservation],
    cancelled: &[RunObservation],
    model: &SuperviseModel,
    skip: &BTreeSet<u64>,
    now: u64,
) -> Vec<(u64, String)> {
    let bounded = |w: &str| model.bounds.contains_key(w);
    let branch = |n: u64| obs.pr_branches.get(&n).map(String::as_str);
    let candidates: Vec<(u64, &String)> = obs
        .pr_heads
        .iter()
        .filter(|(n, _)| !skip.contains(n) && !obs.yielded.contains(n))
        .filter(|(n, head)| {
            !model.bounds.keys().any(|w| {
                of_head(current, w, **n, branch(**n))
                    .iter()
                    .any(|r| r.head_sha == **head && r.in_flight())
            })
        })
        .map(|(n, h)| (*n, h))
        .collect();
    // newest cancel first, then by pull request: a set's own order, the same on every runner
    let first: Vec<(u64, u64)> = candidates
        .iter()
        .filter_map(|(n, head)| {
            cancelled
                .iter()
                .filter(|r| r.head_sha == **head && bounded(&r.workflow))
                .filter(|r| r.event == "pull_request")
                .filter(|r| match branch(*n) {
                    Some(b) => r.head_branch == b,
                    None => r.pull_request == Some(*n),
                })
                .filter_map(RunObservation::ended_at)
                .filter(|t| t.saturating_add(RECOVERY_WINDOW_MINUTES * 60) >= now)
                .max()
                .map(|t| (std::cmp::Reverse(t), *n))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(RECOVERY_READS)
        .map(|(std::cmp::Reverse(t), n)| (n, t))
        .collect();
    let chosen: BTreeSet<u64> = first.iter().map(|(n, _)| *n).collect();
    let rest: Vec<u64> = candidates
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !chosen.contains(n))
        .collect();
    let mut out: Vec<u64> = first.into_iter().map(|(n, _)| n).collect();
    if !rest.is_empty() {
        let slot = usize::try_from(now / 600).unwrap_or(usize::MAX);
        let start = slot.wrapping_mul(ROTATING_READS) % rest.len();
        out.extend(
            rest.iter()
                .cycle()
                .skip(start)
                .take(ROTATING_READS.min(rest.len())),
        );
    }
    out.into_iter()
        .filter_map(|n| obs.pr_heads.get(&n).map(|h| (n, h.clone())))
        .collect()
}

/// The durable half of the invariant, judged from the forge alone: every open pull request's
/// head [`heads_to_read`] names has its commit's runs read whole ([`RunForge::runs_at`]), and
/// each bounded workflow's runs of its branch judged ([`head_need`]). A newest run cancelled
/// within the window, by anyone, below [`MAX_ATTEMPTS`], is asked to run again, at most
/// [`RECOVERY_RERUNS`] in the pass; one at [`MAX_ATTEMPTS`] is held as `head_exhausted` in
/// `held`; a head or listing that cannot be read is said in `unread`. Any runner of any lane
/// finds the same heads: nothing here reads the trail.
#[allow(clippy::too_many_arguments)]
fn recover(
    root: &Path,
    forge: &mut dyn RunForge,
    obs: &RunsObservation,
    model: &SuperviseModel,
    now: u64,
    current: &mut [RunObservation],
    acted: &BTreeSet<u64>,
    trail_broken: &mut bool,
    held: &mut Vec<Hold>,
    unread: &mut Vec<String>,
    reads: &mut usize,
) -> Vec<Recovery> {
    let reach = now.saturating_sub(CANCELLED_REACH_MINUTES * 60);
    let cancelled = match forge.cancelled_since(reach) {
        Ok(c) => c,
        Err(e) => {
            unread.push(format!(
                "the pull requests' cancelled runs could not be listed; only the rotating few \
                 heads were read for recovery: {e}"
            ));
            Vec::new()
        }
    };
    let heads = heads_to_read(obs, current, &cancelled, model, acted, now);
    let mut out = Vec::new();
    let mut asked = 0;
    for (pr, head) in heads {
        let mut runs = match forge.runs_at(&head) {
            Ok(r) => r,
            Err(e) => {
                unread.push(format!(
                    "#{pr}'s head {head}: its runs could not be read, so whether it has one is \
                     not known: {e}"
                ));
                continue;
            }
        };
        let branch = obs.pr_branches.get(&pr).map(String::as_str);
        for workflow in model.bounds.keys() {
            let first = head_need(&of_head(&runs, workflow, pr, branch), now);
            if matches!(first, HeadNeed::Covered | HeadNeed::Settled) {
                continue;
            }
            // the listing says `cancelled` of a run that went red and cancelled itself: its jobs
            // say whether the head has its verdict, and a re-run would write a newer `ci` over it
            let ids = cancelled_newest_first(&of_head(&runs, workflow, pr, branch));
            match read_verdicts(forge, &mut runs, &ids, reads) {
                Ok(true) => {}
                Ok(false) => {
                    unread.push(format!(
                        "#{pr}'s head {head}: this pass's {VERDICT_READS} reads of jobs were \
                         spent before its cancelled runs of {workflow} were read, so whether it \
                         has a verdict is not known here and the next pass reads it"
                    ));
                    continue;
                }
                Err(e) => {
                    unread.push(format!(
                        "#{pr}'s head {head}: the jobs of its cancelled runs of {workflow} could \
                         not be read, so whether it has a verdict is not known: {e}"
                    ));
                    continue;
                }
            }
            let (run, attempts, bound) = match head_need(&of_head(&runs, workflow, pr, branch), now)
            {
                HeadNeed::Covered | HeadNeed::Settled => continue,
                HeadNeed::Exhausted { run, attempts } => match forge.yields(pr, run) {
                    Ok(y) if within_attempts(attempts, y) => (run, attempts, MAX_ATTEMPTS + y),
                    Ok(_) => {
                        held.push(Hold {
                            run,
                            workflow: workflow.clone(),
                            event: "pull_request".into(),
                            reason: supervise::HoldReason::HeadExhausted {
                                pull_request: pr,
                                attempts,
                            },
                        });
                        continue;
                    }
                    Err(e) => {
                        unread.push(format!(
                            "#{pr}'s head {head}: run {run} is at attempt {attempts}, and how many \
                             of them yielded to the release path could not be read: {e}"
                        ));
                        continue;
                    }
                },
                HeadNeed::Rerun { run, attempts } => (run, attempts, MAX_ATTEMPTS),
            };
            let attempt = attempts + 1;
            let recovery = |outcome, detail| Recovery {
                pull_request: pr,
                head_sha: head.clone(),
                run,
                attempt,
                outcome,
                detail,
            };
            if asked == RECOVERY_RERUNS {
                out.push(recovery(
                    RecoveryOutcome::Deferred,
                    format!(
                        "#{pr} has no run of its head {head}: run {run} of {workflow} waits for \
                         the next pass, which asks it again ({RECOVERY_RERUNS} were asked in \
                         this one)"
                    ),
                ));
                continue;
            }
            if *trail_broken {
                out.push(recovery(
                    RecoveryOutcome::Unrecorded,
                    format!(
                        "#{pr} has no run of its head {head}: run {run} was not asked to run \
                         again, since the trail refused an earlier write"
                    ),
                ));
                continue;
            }
            asked += 1;
            let broken_before = *trail_broken;
            let (a, said) = ask_rerun(
                root,
                forge,
                Some(pr),
                &head,
                run,
                workflow,
                (attempt, bound),
                current,
                trail_broken,
            );
            let outcome = match a {
                _ if *trail_broken && !broken_before => RecoveryOutcome::Unrecorded,
                Asked::Rerun => RecoveryOutcome::Rerun,
                Asked::Decided => RecoveryOutcome::Decided,
                Asked::Failed => RecoveryOutcome::RerunFailed,
            };
            out.push(recovery(outcome, said));
        }
    }
    out
}

/// Why a cancel the forge refused found nothing left to do, with the conclusion the run is known
/// to have ended with, or `None` when the refusal stands: the run read again has completed, or the
/// forge answered 409, which it gives for a run that can no longer be cancelled because it
/// completed. A second supervision that decided the same cancel in the same minute (two lanes
/// in one burst) meets exactly this, and so does a cancel that landed although `gh` reported an
/// error (a timeout, a 502).
fn already_ended(
    forge: &dyn RunForge,
    id: u64,
    refusal: &ForgeError,
) -> Option<(String, Option<String>)> {
    match forge.run(id) {
        Ok(r) if !r.in_flight() => Some((
            format!(
                "it had already ended ({}{})",
                r.status,
                r.conclusion
                    .as_deref()
                    .map(|c| format!(", {c}"))
                    .unwrap_or_default()
            ),
            r.conclusion,
        )),
        _ if refusal.0.contains("HTTP 409") => Some((
            format!(
                "the forge answered that it can no longer be cancelled: {}",
                refusal.0.trim()
            ),
            None,
        )),
        _ => None,
    }
}

/// Whether the paired re-run of run `d.run` is asked ([`paired_rerun_needed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Paired {
    /// Asked: of at most [`MAX_ATTEMPTS`] and `yields` attempts.
    Ask {
        /// The attempts of it the release path took.
        yields: u64,
    },
    /// Not asked, and why; `exhausted` when its attempts are spent.
    Not {
        /// Why, for a person.
        why: String,
        /// Whether the head is held as `head_exhausted`.
        exhausted: bool,
    },
}

/// Whether the paired re-run of run `d.run`, just ended, is to be asked ([`Paired::Ask`]), or
/// the reason it is not. A pull request labelled [`YIELDED_LABEL`] (`yielded`) is the release
/// path's: its `resume` runs the head again, and nothing is asked here. Otherwise the head's
/// commit is read whole first, and the jobs of its cancelled runs ([`read_verdicts`]), so that a
/// run of the same workflow and branch that has its verdict (an older twin that passed, #2678's;
/// one that went red and cancelled itself, I2664) or is on its way to one keeps the head, and
/// nothing writes a newer `ci` over that verdict; and a run whose `run_attempt`, less the
/// attempts the release path took ([`RunForge::yields`]), reached [`MAX_ATTEMPTS`] is not asked
/// (`exhausted`: the head is held as `head_exhausted`). Err when the commit's runs,
/// their jobs or the yields could not be read: nothing is asked.
fn paired_rerun_needed(
    forge: &dyn RunForge,
    d: &Decision,
    fresh: &RunObservation,
    branch: Option<&str>,
    yielded: bool,
    reads: &mut usize,
) -> Result<Paired, ForgeError> {
    let n = d.pull_request.unwrap_or_default();
    let not = |why: String, exhausted| Ok(Paired::Not { why, exhausted });
    if yielded {
        return not(
            format!(
                "not run again: #{n} is labelled {YIELDED_LABEL}, and the release path's `resume` \
                 runs its head again once the path is clear"
            ),
            false,
        );
    }
    let mut runs = forge.runs_at(&d.head_sha)?;
    let twins: Vec<&RunObservation> = of_head(&runs, &d.workflow, n, branch)
        .into_iter()
        .filter(|r| r.id != d.run)
        .collect();
    let ids = cancelled_newest_first(&twins);
    if !read_verdicts(forge, &mut runs, &ids, reads)? {
        return Err(ForgeError(format!(
            "this pass's {VERDICT_READS} reads of jobs were spent before the cancelled runs of \
             #{n}'s head were read, so whether it has a verdict is not known"
        )));
    }
    let others: Vec<&RunObservation> = of_head(&runs, &d.workflow, n, branch)
        .into_iter()
        .filter(|r| r.id != d.run)
        .collect();
    if let Some(v) = others.iter().find(|r| r.verdict_written()) {
        return not(
            format!(
                "not run again: run {} of #{n}'s head has its verdict ({}{}), which a re-run \
                 would write a newer `ci` over",
                v.id,
                v.conclusion.as_deref().unwrap_or_default(),
                if v.verdict_in_jobs() {
                    ", in its jobs"
                } else {
                    ""
                }
            ),
            false,
        );
    }
    if let Some(o) = others.iter().find(|r| r.toward_verdict()) {
        return not(
            format!(
                "not run again: run {} of #{n}'s head is on its way to a verdict",
                o.id
            ),
            false,
        );
    }
    // the release path's attempts are read only for a run at the bound: below it, none matter
    let yields = if within_attempts(fresh.run_attempt, 0) {
        0
    } else {
        forge.yields(n, d.run)?
    };
    if !within_attempts(fresh.run_attempt, yields) {
        return not(
            format!(
                "not run again: it was attempt {} of at most {}, and #{n} has no run of its head \
                 until a person re-runs it or pushes",
                fresh.run_attempt,
                MAX_ATTEMPTS + yields
            ),
            true,
        );
    }
    Ok(Paired::Ask { yields })
}

/// When a lane that covers for another stands aside (`--unless-supervised-within`,
/// `--supervised-by`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandAside {
    /// A supervision that succeeded at most this many minutes ago is the one that counts.
    pub within_minutes: u64,
    /// Only a supervision by this lane's job counts. A covering lane names the lane it covers
    /// for: a job that stood aside also succeeded, and a lane that counted its own would stand
    /// aside behind itself for ever once the lane it covers for had stopped.
    pub lane: Option<String>,
}

/// Observe, decide, and — with `apply` — act. `now` is asked once for the decision and again
/// before each act. With `unless`, a supervision that succeeded at most that many minutes ago
/// (by its lane, when it names one) is read first, and if there is one nothing else is: the
/// report says so and exits 0. Err only when the forge could not be observed.
pub fn supervise(
    root: &Path,
    forge: &mut dyn RunForge,
    model: &SuperviseModel,
    now: &dyn Fn() -> u64,
    self_run: Option<u64>,
    apply: bool,
    unless: Option<&StandAside>,
) -> Result<SuperviseReport, ForgeError> {
    // read once: the answer that decides whether to stand aside is the one reported, unless it
    // was one lane's, which is not the supervisor's liveness
    let mut early = None;
    if let Some(StandAside {
        within_minutes: within,
        lane,
    }) = unless
    {
        let within = *within;
        let at = now();
        // with a lane, only the runs updated in the window (a minute of margin for the
        // rounding to minutes): a success older than it does not stand this one aside anyway
        let read = forge.newest_success(
            &model.supervisor,
            Successes {
                lane: lane.as_deref(),
                since: lane
                    .as_ref()
                    .map(|_| at.saturating_sub(within.saturating_add(1).saturating_mul(60))),
            },
        );
        let liveness = supervise::liveness_of(
            read.as_ref()
                .map(Option::as_deref)
                .map_err(ToString::to_string),
            at,
            lane.as_deref(),
            lane.as_ref().map(|_| within.saturating_add(1)),
        );
        if let Some(minutes) = liveness.minutes.filter(|m| *m <= within) {
            return Ok(SuperviseReport {
                repository: String::new(),
                observed_at: crate::peers::rfc3339(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(at),
                ),
                applied: apply,
                in_flight: 0,
                decisions: Vec::new(),
                held: Vec::new(),
                acts: Vec::new(),
                recovered: Vec::new(),
                unread: Vec::new(),
                jobs_read: 0,
                skipped: Some(format!(
                    "a supervision{} succeeded {minutes} min ago, within {within}: nothing \
                     observed",
                    lane.as_deref()
                        .map(|l| format!(" by {l}"))
                        .unwrap_or_default()
                )),
                liveness,
            });
        }
        // a lane's answer is the report's when its window holds the whole stale bound: the
        // queue's alive job names the lane it holds to account, so that a lane that always fails
        // is not hidden by the other lane's successes
        if lane.is_none() || within >= supervise::STALE_MINUTES {
            early = Some(liveness);
        }
    }
    let mut obs = forge.observe()?;
    let first = now();
    // a run's jobs only where a rule could act on it: the rest are judged without them, and a
    // run judged without jobs is too young for a bound and has no twin to duplicate
    let wanted: BTreeSet<u64> = obs
        .runs
        .iter()
        .filter(|r| supervise::needs_jobs(r, &obs.runs, model, first))
        .map(|r| r.id)
        .collect();
    for r in obs.runs.iter_mut() {
        r.jobs = if wanted.contains(&r.id) {
            forge.jobs(r.id)?
        } else {
            Vec::new()
        };
    }
    let situation = Situation {
        runs: &obs.runs,
        pr_heads: &obs.pr_heads,
        full: &obs.full,
        default_branch: &obs.default_branch,
        model,
        now: first,
        self_run,
    };
    let decided = supervise::decide(&situation);
    let mut acts = Vec::new();
    // what this pass has done so far: a run it cancelled is judged as cancelled by every later
    // act's re-check, so that no decision rests on a run this pass already ended
    let mut current = obs.runs.clone();
    let mut trail_broken = false;
    let mut exhausted_holds = Vec::new();
    // the pull requests whose head's re-run an act of this pass judged: recovery leaves them be
    let mut paired_heads = BTreeSet::new();
    // the jobs this pass may read for verdicts in cancelled runs, the acts' and recovery's
    let mut verdict_reads = VERDICT_READS;
    if apply {
        for d in &decided.decisions {
            let word = d.reason.word().to_string();
            if trail_broken {
                acts.push(Act {
                    run: d.run,
                    reason: word,
                    outcome: ActOutcome::Unrecorded,
                    detail: "not acted on: the trail refused an earlier write".into(),
                });
                continue;
            }
            // observe twice: the run and its pull request's head, read again just now
            let fresh = match forge.run(d.run) {
                Ok(r) => r,
                Err(e) => {
                    acts.push(Act {
                        run: d.run,
                        reason: word,
                        outcome: ActOutcome::Unobserved,
                        detail: format!("not cancelled: it could not be read again: {e}"),
                    });
                    continue;
                }
            };
            let mut heads = obs.pr_heads.clone();
            if let Some(n) = d.pull_request {
                match forge.pr_head(n) {
                    Ok(Some(h)) => {
                        heads.insert(n, h);
                    }
                    Ok(None) => {
                        heads.remove(&n);
                    }
                    Err(e) => {
                        acts.push(Act {
                            run: d.run,
                            reason: word,
                            outcome: ActOutcome::Unobserved,
                            detail: format!("not cancelled: #{n} could not be read again: {e}"),
                        });
                        continue;
                    }
                }
            }
            // the other runs of its commit are read again too: a duplicate rests on its older run
            // being in flight, so that a twin the group admitted a moment ago is not ended as
            // one, and a cancel at its pull request's head rests on what else that head has, so
            // that a run of it ended meanwhile turns the cancel into a paired one or none
            let at_head = d
                .pull_request
                .is_some_and(|n| heads.get(&n) == Some(&d.head_sha));
            let duplicate = matches!(d.reason, supervise::CancelReason::DuplicateRun { .. });
            let companions: Vec<u64> = if at_head || duplicate {
                current
                    .iter()
                    .filter(|c| {
                        c.id != d.run
                            && c.in_flight()
                            && c.workflow == d.workflow
                            && c.event == "pull_request"
                            && c.pull_request == d.pull_request
                            && c.head_sha == d.head_sha
                    })
                    .map(|c| c.id)
                    .collect()
            } else {
                Vec::new()
            };
            let mut unread = None;
            for id in companions {
                match forge.run(id) {
                    Ok(r) => {
                        if let Some(slot) = current.iter_mut().find(|c| c.id == r.id) {
                            *slot = r;
                        }
                    }
                    Err(e) => {
                        unread = Some(format!(
                            "not cancelled: run {id} of the same commit could not be read again: \
                             {e}"
                        ));
                        break;
                    }
                }
            }
            if let Some(detail) = unread {
                acts.push(Act {
                    run: d.run,
                    reason: word,
                    outcome: ActOutcome::Unobserved,
                    detail,
                });
                continue;
            }
            let again = Situation {
                runs: &current,
                pr_heads: &heads,
                now: now(),
                ..situation
            };
            let Some(held) = supervise::still_holds(d, Some(&fresh), &again) else {
                acts.push(Act {
                    run: d.run,
                    reason: word,
                    outcome: ActOutcome::Skipped,
                    detail: format!(
                        "not cancelled: read again, it is {} and the decision no longer holds",
                        fresh.status
                    ),
                });
                continue;
            };
            // the act is on the trail before it is taken, or it is not taken
            if let Err(e) = drain::record(
                root,
                event(IntegrationAction::RunCancelAttempted, &held, subject(&held)),
            ) {
                trail_broken = true;
                acts.push(Act {
                    run: d.run,
                    reason: word,
                    outcome: ActOutcome::Unrecorded,
                    detail: format!("not cancelled: the trail could not record it: {e}"),
                });
                continue;
            }
            // `ended`: the run reads completed after the act without a verdict, whatever the
            // forge answered the cancel, so that a paired re-run is asked of it
            let (action, outcome, detail, ended) = match forge.force_cancel(d.run) {
                Ok(()) => {
                    if let Some(r) = current.iter_mut().find(|r| r.id == d.run) {
                        r.status = "completed".into();
                        r.conclusion = Some("cancelled".into());
                    }
                    (
                        IntegrationAction::RunCancelled,
                        ActOutcome::Cancelled,
                        format!("force-cancelled {}", subject(&held)),
                        true,
                    )
                }
                Err(e) => match already_ended(forge, d.run, &e) {
                    // two lanes in one burst can decide the same cancel: the second finds it
                    // done, which is no failure; and a cancel `gh` called failed may have landed
                    Some((why, conclusion)) => {
                        let decided = matches!(
                            conclusion.as_deref(),
                            Some("success" | "failure" | "timed_out")
                        );
                        if let Some(r) = current.iter_mut().find(|r| r.id == d.run) {
                            r.status = "completed".into();
                            if conclusion.is_some() {
                                r.conclusion = conclusion;
                            }
                        }
                        (
                            IntegrationAction::RunAlreadyEnded,
                            ActOutcome::Skipped,
                            format!("not cancelled, {why}: {}", subject(&held)),
                            !decided,
                        )
                    }
                    // still in flight: the head keeps it, and a cancel that lands later is
                    // found by the next pass ([`recover`])
                    None => (
                        IntegrationAction::RunCancelFailed,
                        ActOutcome::Failed,
                        format!("could not cancel {}: {e}", subject(&held)),
                        false,
                    ),
                },
            };
            let mut detail = match drain::record(root, event(action, &held, detail.clone())) {
                Ok(_) => detail,
                Err(e) => {
                    trail_broken = true;
                    format!("{detail} (the trail could not record the outcome: {e})")
                }
            };
            // the pair of a cancel that ended its head's last run: asked whatever the forge
            // answered the cancel and the trail said of it, once the run reads ended without a
            // verdict, since it is what gives the head a run again; unless the head has a
            // verdict or a run on its way after all, or the run's attempts are spent
            let mut outcome = outcome;
            if ended && held.rerun {
                paired_heads.extend(held.pull_request);
                let branch = obs
                    .pr_branches
                    .get(&held.pull_request.unwrap_or_default())
                    .map(String::as_str);
                let yielded = held.pull_request.is_some_and(|n| obs.yielded.contains(&n));
                match paired_rerun_needed(forge, &held, &fresh, branch, yielded, &mut verdict_reads)
                {
                    Ok(Paired::Ask { yields }) => {
                        let bound = MAX_ATTEMPTS + yields;
                        let (asked, said) = ask_rerun(
                            root,
                            forge,
                            held.pull_request,
                            &held.head_sha,
                            d.run,
                            &d.workflow,
                            (fresh.run_attempt + 1, bound),
                            &mut current,
                            &mut trail_broken,
                        );
                        match asked {
                            Asked::Rerun => outcome = ActOutcome::CancelledAndRerun,
                            Asked::Decided => {}
                            Asked::Failed => outcome = ActOutcome::RerunFailed,
                        }
                        detail = format!("{detail}; {said}");
                    }
                    Ok(Paired::Not { why, exhausted }) => {
                        if exhausted {
                            exhausted_holds.push(Hold {
                                run: d.run,
                                workflow: d.workflow.clone(),
                                event: d.event.clone(),
                                reason: supervise::HoldReason::HeadExhausted {
                                    pull_request: held.pull_request.unwrap_or_default(),
                                    attempts: fresh.run_attempt,
                                },
                            });
                        }
                        detail = format!("{detail}; {why}");
                    }
                    Err(e) => {
                        outcome = ActOutcome::RerunFailed;
                        detail = format!(
                            "{detail}; not run again: #{}'s head's runs could not be read, so \
                             whether it has a verdict is not known, and the next pass judges it \
                             from the forge: {e}",
                            held.pull_request.unwrap_or_default()
                        );
                    }
                }
            }
            acts.push(Act {
                run: d.run,
                reason: word,
                outcome,
                detail,
            });
        }
    }
    // a head left without a run, by any pass of any lane or by a person, is asked for one again,
    // after this pass's acts; a head whose re-run an act judged is left to that act
    let mut held = decided.held;
    held.append(&mut exhausted_holds);
    let mut unread = Vec::new();
    let recovered = if apply {
        let acted = paired_heads;
        let at = now();
        recover(
            root,
            forge,
            &obs,
            model,
            at,
            &mut current,
            &acted,
            &mut trail_broken,
            &mut held,
            &mut unread,
            &mut verdict_reads,
        )
    } else {
        Vec::new()
    };
    // read last, so that an unreadable answer costs no act
    // over the runs of the stale bound only: an older success is stale whenever it ended
    let liveness = match early {
        Some(l) => l,
        None => {
            let at = now();
            let window = supervise::STALE_MINUTES + 1;
            supervise::liveness_of(
                forge
                    .newest_success(
                        &model.supervisor,
                        Successes {
                            lane: None,
                            since: Some(at.saturating_sub(window * 60)),
                        },
                    )
                    .as_ref()
                    .map(Option::as_deref)
                    .map_err(ToString::to_string),
                at,
                None,
                Some(window),
            )
        }
    };
    Ok(SuperviseReport {
        liveness,
        repository: obs.repository,
        observed_at: crate::peers::rfc3339(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(first),
        ),
        applied: apply,
        in_flight: obs.runs.iter().filter(|r| r.in_flight()).count(),
        decisions: decided.decisions,
        held,
        acts,
        recovered,
        unread,
        jobs_read: wanted.len(),
        skipped: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::supervise::{SupersededGroup, WorkflowBound};
    use serde_json::json;
    use std::cell::RefCell;

    const NOW: u64 = 1_793_000_000;

    fn at(minutes_ago: u64) -> String {
        crate::peers::rfc3339(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(NOW - minutes_ago * 60),
        )
    }

    fn model() -> SuperviseModel {
        let mut bounds = BTreeMap::new();
        bounds.insert(
            "validate".to_string(),
            WorkflowBound {
                queued_minutes: 240,
                running_minutes: 255,
                superseded: SupersededGroup::PerPullRequest,
            },
        );
        SuperviseModel {
            supervisor: "supervise".into(),
            bounds,
        }
    }

    fn run(id: u64, sha: &str, pr: u64, status: &str, minutes_ago: u64) -> RunObservation {
        RunObservation {
            id,
            workflow: "validate".into(),
            event: "pull_request".into(),
            head_branch: "fix/x".into(),
            head_sha: sha.into(),
            pull_request: Some(pr),
            status: status.into(),
            conclusion: None,
            created_at: at(minutes_ago),
            run_started_at: Some(at(minutes_ago)),
            run_attempt: 1,
            updated_at: None,
            // a run its group admitted at its creation has its first job; a pending one has none
            jobs: if status == "queued" {
                vec![JobObservation {
                    name: "plan".into(),
                    status: "queued".into(),
                    started_at: Some(at(minutes_ago)),
                    created_at: Some(at(minutes_ago)),
                    conclusion: None,
                }]
            } else {
                Vec::new()
            },
        }
    }

    /// A forge that is a value: what it answers, and what was asked of it.
    #[derive(Default)]
    struct Fake {
        obs: Option<RunsObservation>,
        /// What a second read of a run says; the first observation's when absent.
        reread: BTreeMap<u64, RunObservation>,
        /// What a second read of a pull request's head says.
        reread_heads: BTreeMap<u64, Option<String>>,
        /// How the forge refuses a cancel, when it does.
        refuse_cancel: Option<&'static str>,
        /// Whether a refused cancel landed all the same (`gh` timed out, or the forge answered
        /// 502 while it applied it).
        refused_but_landed: bool,
        /// What a read of a run says once its cancel was asked, landed or not.
        after_cancel: BTreeMap<u64, RunObservation>,
        /// The runs whose cancel was asked, in order.
        cancel_asked: RefCell<Vec<u64>>,
        /// What the forge lists for a commit, when it is asked; the observation's runs of it,
        /// read again, when absent.
        at_commit: Option<Result<Vec<RunObservation>, &'static str>>,
        cancelled: RefCell<Vec<u64>>,
        /// How the forge refuses a re-run, when it does.
        refuse_rerun: Option<&'static str>,
        /// What the listing of cancelled runs answers; nothing listed when absent.
        listed_cancelled: Option<Result<Vec<RunObservation>, &'static str>>,
        /// The commits whose runs were asked for, in order.
        commits_asked: RefCell<Vec<String>>,
        /// The runs asked to run again, in order.
        rerun: RefCell<Vec<u64>>,
        /// What the supervisor's newest success is; `None` reads as "an hour ago".
        success: Option<Result<Option<String>, String>>,
        /// The runs whose jobs were asked for, in order.
        jobs_asked: RefCell<Vec<u64>>,
        /// How often the newest success was asked for, and for which lane.
        success_asked: RefCell<Vec<Option<String>>>,
        /// What a read of a run's jobs answers, before anything else: a completed run's, which
        /// no listing carries.
        job_reads: BTreeMap<u64, Vec<JobObservation>>,
        /// How many yield notes the release path left for each run.
        yield_notes: BTreeMap<u64, u64>,
        /// The runs whose yields were asked for, in order.
        yields_asked: RefCell<Vec<u64>>,
    }

    impl RunForge for Fake {
        fn observe(&self) -> Result<RunsObservation, ForgeError> {
            self.obs
                .clone()
                .ok_or_else(|| ForgeError("no forge here".into()))
        }
        fn jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError> {
            self.jobs_asked.borrow_mut().push(id);
            if let Some(j) = self.job_reads.get(&id) {
                return Ok(j.clone());
            }
            Ok(self
                .obs
                .as_ref()
                .and_then(|o| o.runs.iter().find(|r| r.id == id))
                .map(|r| r.jobs.clone())
                .unwrap_or_default())
        }
        fn run(&self, id: u64) -> Result<RunObservation, ForgeError> {
            if let Some(r) = self
                .after_cancel
                .get(&id)
                .filter(|_| self.cancel_asked.borrow().contains(&id))
            {
                return Ok(r.clone());
            }
            if let Some(r) = self.reread.get(&id) {
                return Ok(r.clone());
            }
            self.obs
                .as_ref()
                .and_then(|o| o.runs.iter().find(|r| r.id == id).cloned())
                .ok_or_else(|| ForgeError(format!("no run {id}")))
        }
        fn pr_head(&self, n: u64) -> Result<Option<String>, ForgeError> {
            if let Some(h) = self.reread_heads.get(&n) {
                return Ok(h.clone());
            }
            Ok(self.obs.as_ref().and_then(|o| o.pr_heads.get(&n).cloned()))
        }
        fn force_cancel(&mut self, id: u64) -> Result<(), ForgeError> {
            self.cancel_asked.borrow_mut().push(id);
            if let Some(why) = self.refuse_cancel {
                if self.refused_but_landed {
                    self.cancelled.borrow_mut().push(id);
                }
                return Err(ForgeError(why.into()));
            }
            self.cancelled.borrow_mut().push(id);
            Ok(())
        }
        fn rerun(&mut self, id: u64) -> Result<Rerun, ForgeError> {
            let listed = self
                .at_commit
                .as_ref()
                .and_then(|c| c.as_ref().ok())
                .and_then(|c| c.iter().find(|r| r.id == id));
            if let Some(r) = self
                .after_cancel
                .get(&id)
                .or(self.reread.get(&id))
                .or(listed)
            {
                // as GhRunForge's: a cancelled run's jobs are read for its verdict
                let mut r = r.clone();
                if let Some(j) = self.job_reads.get(&id) {
                    r.jobs = j.clone();
                }
                if r.verdict_written() {
                    return Ok(Rerun::Decided);
                }
            } else if !self.cancelled.borrow().contains(&id) {
                return Err(ForgeError(format!("run {id} was never cancelled")));
            }
            if let Some(why) = self.refuse_rerun {
                return Err(ForgeError(why.into()));
            }
            self.rerun.borrow_mut().push(id);
            Ok(Rerun::Asked)
        }
        fn runs_at(&self, sha: &str) -> Result<Vec<RunObservation>, ForgeError> {
            self.commits_asked.borrow_mut().push(sha.to_string());
            if let Some(r) = &self.at_commit {
                return r.clone().map_err(|e| ForgeError(e.into()));
            }
            Ok(self
                .obs
                .iter()
                .flat_map(|o| o.runs.iter())
                .filter(|r| r.head_sha == sha)
                .map(|r| self.reread.get(&r.id).unwrap_or(r).clone())
                .collect())
        }
        fn cancelled_since(&self, _since: u64) -> Result<Vec<RunObservation>, ForgeError> {
            match &self.listed_cancelled {
                None => Ok(Vec::new()),
                Some(r) => r.clone().map_err(|e| ForgeError(e.into())),
            }
        }
        fn yields(&self, _pr: u64, run: u64) -> Result<u64, ForgeError> {
            self.yields_asked.borrow_mut().push(run);
            Ok(self.yield_notes.get(&run).copied().unwrap_or_default())
        }
        fn newest_success(
            &self,
            _workflow: &str,
            among: Successes<'_>,
        ) -> Result<Option<String>, ForgeError> {
            self.success_asked
                .borrow_mut()
                .push(among.lane.map(str::to_string));
            match &self.success {
                None => Ok(Some(at(60))),
                Some(r) => r.clone().map_err(ForgeError),
            }
        }
    }

    /// Train 10a's shape: run 100 of the old head in progress, run 101 of the new one
    /// pending behind it, and run 200 of another pull request's earlier head queued for five
    /// hours (at its head, with nothing else there, it would be held: a head keeps its run).
    fn train() -> Fake {
        let mut stuck = run(200, "b", 8, "queued", 300);
        stuck.head_branch = "fix/y".into();
        Fake {
            obs: Some(RunsObservation {
                repository: "o/r".into(),
                default_branch: "master".into(),
                runs: vec![
                    run(100, "old", 7, "in_progress", 90),
                    run(101, "new", 7, "pending", 60),
                    stuck,
                ],
                pr_heads: [(7, "new".to_string()), (8, "c".to_string())]
                    .into_iter()
                    .collect(),
                full: BTreeSet::new(),
                pr_branches: BTreeMap::new(),
                yielded: BTreeSet::new(),
            }),
            ..Fake::default()
        }
    }

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(d.path())
            .status()
            .unwrap()
            .success();
        assert!(ok);
        d
    }

    fn trail(root: &Path) -> Vec<IntegrationEvent> {
        drain::events(root)
    }

    #[test]
    fn a_dry_run_and_apply_decide_identically_and_only_apply_acts() {
        let d = repo();
        let m = model();
        let mut dry = train();
        let a = supervise(d.path(), &mut dry, &m, &|| NOW, None, false, None).unwrap();
        assert!(dry.cancelled.borrow().is_empty());
        assert!(a.acts.is_empty());
        assert!(trail(d.path()).is_empty(), "a dry run writes nothing");
        assert_eq!(a.exit_code(), 10);

        let mut wet = train();
        let b = supervise(d.path(), &mut wet, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(a.decisions, b.decisions);
        assert_eq!(a.held, b.held);
        let words: Vec<_> = b
            .decisions
            .iter()
            .map(|x| (x.run, x.reason.word()))
            .collect();
        assert_eq!(
            words,
            vec![
                (100, "superseded_blocks_successor"),
                (200, "queued_past_bound")
            ]
        );
        assert_eq!(*wet.cancelled.borrow(), vec![100, 200]);
        assert!(b.acts.iter().all(|x| x.outcome == ActOutcome::Cancelled));
        assert_eq!(b.exit_code(), 0);
        // each act is on the trail before it, with its reason, and its outcome after
        let t: Vec<_> = trail(d.path())
            .into_iter()
            .map(|e| (e.action, e.pr, e.detail))
            .collect();
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].0, IntegrationAction::RunCancelAttempted);
        assert_eq!(t[0].1, Some(7));
        assert!(t[0].2.contains("run 100 of validate"), "{}", t[0].2);
        assert!(t[0].2.contains("superseded_blocks_successor"), "{}", t[0].2);
        assert_eq!(t[1].0, IntegrationAction::RunCancelled);
        assert_eq!(t[2].0, IntegrationAction::RunCancelAttempted);
        assert!(t[2].2.contains("queued_past_bound"), "{}", t[2].2);
        assert_eq!(t[3].0, IntegrationAction::RunCancelled);
    }

    #[test]
    fn nothing_to_do_exits_zero_and_an_unreadable_forge_is_an_error() {
        let d = repo();
        let m = model();
        let mut quiet = Fake {
            obs: Some(RunsObservation {
                repository: "o/r".into(),
                default_branch: "master".into(),
                runs: vec![run(1, "a", 7, "in_progress", 5)],
                pr_heads: [(7, "a".to_string())].into_iter().collect(),
                full: BTreeSet::new(),
                pr_branches: BTreeMap::new(),
                yielded: BTreeSet::new(),
            }),
            ..Fake::default()
        };
        let r = supervise(d.path(), &mut quiet, &m, &|| NOW, None, false, None).unwrap();
        assert_eq!(r.exit_code(), 0);
        let r = supervise(d.path(), &mut quiet, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.exit_code(), 0);
        assert!(supervise(
            d.path(),
            &mut Fake::default(),
            &m,
            &|| NOW,
            None,
            true,
            None
        )
        .is_err());
    }

    #[test]
    fn the_act_is_rechecked_and_skipped_when_the_head_changed_again() {
        let d = repo();
        let m = model();
        let mut f = train();
        // read again, #7's head is the run's own again (the newer push was reverted)
        f.reread_heads.insert(7, Some("old".into()));
        // and run 200 started meanwhile: it is no longer stuck in the queue
        let mut started = f.obs.as_ref().unwrap().runs[2].clone();
        started.status = "in_progress".into();
        started.run_started_at = Some(at(1));
        started.jobs = vec![JobObservation {
            name: "plan".into(),
            status: "in_progress".into(),
            started_at: Some(at(1)),
            created_at: None,
            conclusion: None,
        }];
        f.reread.insert(200, started);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.decisions.len(), 2);
        assert!(f.cancelled.borrow().is_empty());
        assert!(
            r.acts.iter().all(|a| a.outcome == ActOutcome::Skipped),
            "{r:?}"
        );
        assert!(
            trail(d.path()).is_empty(),
            "a skipped act is never attempted"
        );
        assert_eq!(r.exit_code(), 0);
        // a run that completed meanwhile is skipped too
        let mut f = train();
        let mut done = f.obs.as_ref().unwrap().runs[0].clone();
        done.status = "completed".into();
        f.reread.insert(100, done);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.cancelled.borrow(), vec![200]);
        assert_eq!(r.acts[0].outcome, ActOutcome::Skipped);
        // a duplicate whose older run passed meanwhile is skipped: the group admits the twin
        // then, and a cancel would run its `ci` job over the pass
        let mut f = train();
        f.obs
            .as_mut()
            .unwrap()
            .runs
            .push(run(102, "new", 7, "pending", 59));
        let mut passed = f.obs.as_ref().unwrap().runs[1].clone();
        assert_eq!(passed.id, 101);
        passed.status = "completed".into();
        passed.conclusion = Some("success".into());
        f.reread.insert(101, passed);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.decisions[1].reason.word(), "duplicate_run");
        assert_eq!(r.acts[1].run, 102);
        assert_eq!(r.acts[1].outcome, ActOutcome::Skipped, "{r:?}");
        assert!(!f.cancelled.borrow().contains(&102));
    }

    #[test]
    fn a_trail_that_cannot_be_written_prevents_every_act() {
        let d = repo();
        let m = model();
        let mut f = train();
        drain::FAIL_WRITE.with(|n| n.set(1));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        drain::FAIL_WRITE.with(|n| n.set(0));
        assert!(
            f.cancelled.borrow().is_empty(),
            "nothing is cancelled unrecorded"
        );
        assert!(
            r.acts.iter().all(|a| a.outcome == ActOutcome::Unrecorded),
            "{r:?}"
        );
        assert_eq!(r.exit_code(), 12);
    }

    #[test]
    fn the_report_says_whether_the_supervisor_is_alive_and_never_exits_on_it() {
        let d = repo();
        let m = model();
        let mut f = train();
        f.success = Some(Ok(Some(at(31))));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, None).unwrap();
        assert_eq!(r.liveness.finding.as_deref(), Some("supervise_stale"));
        assert_eq!(
            r.exit_code(),
            10,
            "the decisions decide the exit, not liveness"
        );
        let mut f = train();
        f.success = Some(Ok(Some(at(9))));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.liveness.finding, None);
        assert_eq!(r.liveness.minutes, Some(9));
        assert_eq!(r.exit_code(), 0);
        let mut f = train();
        f.success = Some(Err("HTTP 502".into()));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.liveness.finding.as_deref(), Some("supervise_unobserved"));
        assert_eq!(
            *f.cancelled.borrow(),
            vec![100, 200],
            "an unread liveness costs no act"
        );
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["liveness"]["finding"], "supervise_unobserved");
    }

    #[test]
    fn jobs_are_read_only_for_the_runs_a_rule_could_act_on() {
        let d = repo();
        let m = model();
        let mut f = train();
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, None).unwrap();
        // 100 (in progress 90 min) and 101 (pending 60 min, no twin) are younger than every
        // bound; 200 has queued 300 minutes
        assert_eq!(*f.jobs_asked.borrow(), vec![200]);
        assert_eq!(r.jobs_read, 1);
        assert_eq!(r.in_flight, 3);
        // a twin of its branch and commit from one burst is read, and so is the older run of
        // the pair, whatever their age: whether a job of the twin exists, and whether the older
        // run will be ended before the next pass
        let mut f = train();
        let obs = f.obs.as_mut().unwrap();
        obs.runs.push(run(102, "new", 7, "pending", 59));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, None).unwrap();
        assert_eq!(*f.jobs_asked.borrow(), vec![101, 200, 102]);
        assert_eq!(r.decisions.len(), 3, "{r:?}");
        assert_eq!(r.decisions[1].reason.word(), "duplicate_run");
    }

    #[test]
    fn a_lane_that_covers_stands_aside_when_a_supervision_just_succeeded() {
        let d = repo();
        let m = model();
        let within = |minutes, lane: Option<&str>| StandAside {
            within_minutes: minutes,
            lane: lane.map(str::to_string),
        };
        // succeeded three minutes ago: nothing is observed, nothing acted on, exit 0
        let mut f = Fake {
            success: Some(Ok(Some(at(3)))),
            ..Fake::default()
        };
        let cover = within(8, None);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, Some(&cover)).unwrap();
        assert!(r.skipped.is_some(), "{r:?}");
        assert!(r.acts.is_empty() && r.decisions.is_empty());
        assert_eq!(r.liveness.minutes, Some(3));
        assert_eq!(r.exit_code(), 0);
        // nine minutes ago is outside eight: it supervises, and asks for the success once
        let mut f = train();
        f.success = Some(Ok(Some(at(9))));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, Some(&cover)).unwrap();
        assert!(r.skipped.is_none());
        assert_eq!(*f.cancelled.borrow(), vec![100, 200]);
        assert_eq!(*f.success_asked.borrow(), vec![None]);
        assert_eq!(r.liveness.minutes, Some(9));
        // none, or one that cannot be read, never stands it aside
        for success in [Ok(None), Err("HTTP 502".to_string())] {
            let mut f = train();
            f.success = Some(success);
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, Some(&cover)).unwrap();
            assert!(r.skipped.is_none());
            assert_eq!(*f.cancelled.borrow(), vec![100, 200]);
        }
        // a lane that covers for lundra asks for lundra's success only, since a job that
        // stood aside succeeded too: counting its own, it would stand aside behind itself for
        // ever once lundra stopped. Supervising, it reports the supervisor's liveness, not
        // lundra's, and asks for it again
        let by = within(8, Some("lundra"));
        let mut f = Fake {
            success: Some(Ok(Some(at(3)))),
            ..Fake::default()
        };
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, Some(&by)).unwrap();
        assert!(r.skipped.is_some(), "{r:?}");
        assert_eq!(*f.success_asked.borrow(), vec![Some("lundra".to_string())]);
        let mut f = train();
        f.success = Some(Ok(Some(at(9))));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, Some(&by)).unwrap();
        assert!(r.skipped.is_none());
        assert_eq!(*f.cancelled.borrow(), vec![100, 200]);
        assert_eq!(
            *f.success_asked.borrow(),
            vec![Some("lundra".to_string()), None]
        );
    }

    #[test]
    fn the_alive_check_holds_the_lane_it_names_to_account() {
        let d = repo();
        let m = model();
        let by = StandAside {
            within_minutes: supervise::STALE_MINUTES,
            lane: Some("lundra".into()),
        };
        // lundra's newest success is beyond the stale bound, whatever the hosted lane did: the
        // report's liveness is lundra's, read once, and it is stale
        let mut f = train();
        f.success = Some(Ok(None));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, Some(&by)).unwrap();
        assert!(r.skipped.is_none());
        assert_eq!(*f.success_asked.borrow(), vec![Some("lundra".to_string())]);
        assert_eq!(
            r.liveness.finding.as_deref(),
            Some(supervise::SUPERVISE_STALE)
        );
        assert_eq!(r.liveness.lane.as_deref(), Some("lundra"));
        let detail = r.liveness.detail.clone().unwrap();
        assert!(
            detail.contains("by lundra") && detail.contains("31 min"),
            "{detail}"
        );
        // within it: alive, by lundra
        let mut f = Fake {
            success: Some(Ok(Some(at(12)))),
            ..Fake::default()
        };
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, Some(&by)).unwrap();
        assert!(r.skipped.is_some());
        assert_eq!(r.liveness.finding, None);
        assert_eq!(r.liveness.lane.as_deref(), Some("lundra"));
        // without a lane every lane's success counts, over the runs of the stale bound
        let mut f = train();
        f.success = Some(Ok(None));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, None).unwrap();
        assert_eq!(r.liveness.lane, None);
        assert_eq!(
            r.liveness.finding.as_deref(),
            Some(supervise::SUPERVISE_STALE)
        );
    }

    #[test]
    fn a_success_is_counted_for_a_lane_only_when_its_job_is_that_lanes() {
        let jobs = [
            json!({"name": "supervise (hosted)", "conclusion": "success",
                   "completed_at": "2026-10-29T03:00:00Z"}),
            json!({"name": "supervise (lundra)", "conclusion": "success",
                   "completed_at": "2026-10-29T02:50:00Z"}),
            json!({"name": "supervise (lundra)", "conclusion": "failure",
                   "completed_at": "2026-10-29T02:55:00Z"}),
        ];
        assert_eq!(
            newest_success_of(&jobs, None).as_deref(),
            Some("2026-10-29T03:00:00Z")
        );
        assert_eq!(
            newest_success_of(&jobs, Some("lundra")).as_deref(),
            Some("2026-10-29T02:50:00Z")
        );
        assert_eq!(newest_success_of(&jobs, Some("arm")), None);
        assert_eq!(newest_success_of(&jobs[..1], Some("lundra")), None);
        // and only the runs updated in the window are read for it
        let page = json!({"workflow_runs": [
            {"id": 3, "updated_at": "2026-10-29T03:00:00Z"},
            {"id": 2, "updated_at": "2026-10-29T01:00:00Z"},
            {"id": 1, "updated_at": "not a time"},
        ]});
        assert_eq!(runs_updated_since(&page, None), vec![3, 2, 1]);
        let since = crate::peers::parse_rfc3339("2026-10-29T02:51:00Z");
        assert_eq!(runs_updated_since(&page, since), vec![3, 1]);
    }

    #[test]
    fn a_refused_cancel_is_recorded_and_exits_twelve() {
        let d = repo();
        let m = model();
        let mut f = train();
        f.refuse_cancel = Some("gh: Resource not accessible by integration (HTTP 403)");
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.acts.iter().all(|a| a.outcome == ActOutcome::Failed));
        assert_eq!(r.exit_code(), 12);
        let t = trail(d.path());
        assert_eq!(t[1].action, IntegrationAction::RunCancelFailed);
        assert!(t[1].class.is_some(), "a failed act names its class");
    }

    #[test]
    fn a_cancel_another_lane_already_took_is_skipped_not_failed() {
        // two lanes of one burst decided the same cancels; this one re-read them still in
        // flight, and the other lane's force-cancels landed before its own: GitHub answers 409
        let d = repo();
        let m = model();
        let mut f = train();
        f.refuse_cancel = Some("gh: Cannot cancel a workflow run that is completed. (HTTP 409)");
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(!r.acts.is_empty(), "{r:?}");
        assert!(
            r.acts.iter().all(|a| a.outcome == ActOutcome::Skipped),
            "{r:?}"
        );
        assert_eq!(
            r.exit_code(),
            0,
            "a cancel that is already done fails nothing"
        );
        let t = trail(d.path());
        assert_eq!(t[1].action, IntegrationAction::RunAlreadyEnded);
        assert!(t[1].class.is_none(), "it is no failure: {:?}", t[1]);
        assert!(t[1].detail.contains("HTTP 409"), "{}", t[1].detail);
        // whatever else the forge says, a run that reads completed after the refusal had ended
        let mut f = train();
        let other = ForgeError("gh: Server Error (HTTP 502)".into());
        assert_eq!(
            already_ended(&f, 100, &other),
            None,
            "still in flight: it stands"
        );
        let mut done = run(100, "old", 7, "completed", 90);
        done.conclusion = Some("cancelled".into());
        f.reread.insert(100, done);
        assert_eq!(
            already_ended(&f, 100, &other),
            Some((
                "it had already ended (completed, cancelled)".to_string(),
                Some("cancelled".to_string())
            ))
        );
        // and a 409 is the forge's own word for it, even when the run cannot be read again
        let unread = Fake::default();
        assert_eq!(
            already_ended(&unread, 1, &ForgeError("HTTP 403".into())),
            None
        );
        assert!(already_ended(&unread, 1, &ForgeError("x (HTTP 409)".into())).is_some());
    }

    /// #8's only run, at its head, with a job in progress for `minutes`, or queued that long.
    fn last_run_of_head(status: &str, minutes: u64) -> Fake {
        let mut r = run(300, "h", 8, status, minutes);
        r.head_branch = "fix/y".into();
        if status == "in_progress" {
            r.jobs = vec![JobObservation {
                name: "suite".into(),
                status: "in_progress".into(),
                started_at: Some(at(minutes)),
                created_at: Some(at(minutes)),
                conclusion: None,
            }];
        }
        Fake {
            obs: Some(RunsObservation {
                repository: "o/r".into(),
                default_branch: "master".into(),
                runs: vec![r],
                pr_heads: [(8, "h".to_string())].into_iter().collect(),
                full: BTreeSet::new(),
                pr_branches: BTreeMap::new(),
                yielded: BTreeSet::new(),
            }),
            ..Fake::default()
        }
    }

    #[test]
    fn the_last_run_of_a_head_past_its_bound_is_run_again_or_left_to_wait() {
        let d = repo();
        let m = model();
        // a job past its bound holds a runner: ended, and asked again once ended
        let mut f = last_run_of_head("in_progress", 300);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.decisions.len(), 1, "{r:?}");
        assert_eq!(r.decisions[0].reason.word(), "running_past_bound");
        assert!(r.decisions[0].rerun, "{r:?}");
        assert_eq!(*f.cancelled.borrow(), vec![300]);
        assert_eq!(*f.rerun.borrow(), vec![300]);
        assert_eq!(r.acts[0].outcome, ActOutcome::CancelledAndRerun, "{r:?}");
        assert_eq!(r.exit_code(), 0);
        let t: Vec<_> = trail(d.path()).into_iter().map(|e| e.action).collect();
        assert_eq!(
            t,
            vec![
                IntegrationAction::RunCancelAttempted,
                IntegrationAction::RunCancelled,
                IntegrationAction::RunRerunAttempted,
                IntegrationAction::RunRerunRequested
            ]
        );
        assert!(trail(d.path()).iter().all(|e| e.run == Some(300)));
        let first = &trail(d.path())[0].detail;
        assert!(first.contains("run again once ended"), "{first}");
        // a re-run the forge refuses is a failed act on the trail: the head has no run
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_rerun = Some("gh: Resource not accessible by integration (HTTP 403)");
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.acts[0].outcome, ActOutcome::RerunFailed, "{r:?}");
        assert_eq!(r.exit_code(), 12);
        assert!(r.acts[0].detail.contains("#8 has no run"), "{r:?}");
        assert!(
            r.recovered.is_empty(),
            "the act's own run waits for the next pass: {r:?}"
        );
        let t = trail(d.path());
        assert_eq!(t[3].action, IntegrationAction::RunRerunFailed);
        assert!(t[3].class.is_some(), "a failed act names its class");
        // a run that waits holds no runner: held, and nothing is asked of the forge
        let d = repo();
        let mut f = last_run_of_head("queued", 300);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.decisions.is_empty(), "{r:?}");
        assert_eq!(
            r.held[0].reason,
            crate::integration::supervise::HoldReason::LastRunOfHead {
                pull_request: 8,
                would_end_for: "queued_past_bound".into()
            }
        );
        assert!(f.cancelled.borrow().is_empty());
        assert!(trail(d.path()).is_empty());
        // with a newer run of the head waiting behind it, the queued one is ended as before
        let d = repo();
        let mut f = last_run_of_head("queued", 300);
        let mut behind = run(301, "h", 8, "pending", 10);
        behind.head_branch = "fix/y".into();
        f.obs.as_mut().unwrap().runs.push(behind);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.cancelled.borrow(), vec![300], "{r:?}");
        assert!(!r.decisions[0].rerun);
        assert!(f.rerun.borrow().is_empty());
        // and a plain cancel decided while another run of the head was there is paired at the
        // act when that run has ended meanwhile
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        let mut other = run(301, "h", 8, "pending", 10);
        other.head_branch = "fix/y".into();
        f.obs.as_mut().unwrap().runs.push(other.clone());
        let mut gone = other;
        gone.status = "completed".into();
        gone.conclusion = Some("cancelled".into());
        f.reread.insert(301, gone.clone());
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(!r.decisions[0].rerun, "decided with 301 waiting: {r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::CancelledAndRerun, "{r:?}");
        assert_eq!(*f.rerun.borrow(), vec![300]);
    }

    #[test]
    fn a_paired_cancel_gh_called_failed_is_run_again_once_it_reads_ended() {
        // the review of #2645 at e54560c6d31: the force-cancel reached the forge, but `gh`
        // answered an error (its 20 s ran out, or a 502 while the forge applied it). The run
        // reads completed and cancelled: the head has lost its last run, and it is run again
        let d = repo();
        let m = model();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_cancel = Some("gh: Server Error (HTTP 502)");
        f.refused_but_landed = true;
        let mut gone = f.obs.as_ref().unwrap().runs[0].clone();
        gone.status = "completed".into();
        gone.conclusion = Some("cancelled".into());
        f.after_cancel.insert(300, gone);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::CancelledAndRerun, "{r:?}");
        assert_eq!(r.exit_code(), 0);
        let t: Vec<_> = trail(d.path()).into_iter().map(|e| e.action).collect();
        assert_eq!(
            t,
            vec![
                IntegrationAction::RunCancelAttempted,
                IntegrationAction::RunAlreadyEnded,
                IntegrationAction::RunRerunAttempted,
                IntegrationAction::RunRerunRequested
            ]
        );
        // it reached a verdict of its own before the cancel did, passed, failed or timed out:
        // the head has it, and nothing is asked, since a re-run would write a newer `ci` over it
        for conclusion in ["success", "failure", "timed_out"] {
            let d = repo();
            let mut f = last_run_of_head("in_progress", 300);
            f.refuse_cancel =
                Some("gh: Cannot cancel a workflow run that is completed. (HTTP 409)");
            let mut ended = f.obs.as_ref().unwrap().runs[0].clone();
            ended.status = "completed".into();
            ended.conclusion = Some(conclusion.into());
            f.after_cancel.insert(300, ended);
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
            assert!(f.rerun.borrow().is_empty(), "{conclusion}: {r:?}");
            assert_eq!(
                r.acts[0].outcome,
                ActOutcome::Skipped,
                "{conclusion}: {r:?}"
            );
            assert_eq!(r.exit_code(), 0);
        }
        // still in flight after the refusal: the head keeps that run, and the act failed
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_cancel = Some("gh: Server Error (HTTP 502)");
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.acts[0].outcome, ActOutcome::Failed, "{r:?}");
        assert!(f.rerun.borrow().is_empty());
        assert_eq!(r.exit_code(), 12);
    }

    /// #8's head `h` with no run in flight: `runs` are every run the forge lists for the commit,
    /// on #8's branch `fix/y`, and nothing else is open or in flight.
    fn bare_head(runs: Vec<RunObservation>) -> Fake {
        Fake {
            obs: Some(RunsObservation {
                repository: "o/r".into(),
                default_branch: "master".into(),
                runs: Vec::new(),
                pr_heads: [(8, "h".to_string())].into_iter().collect(),
                full: BTreeSet::new(),
                pr_branches: [(8, "fix/y".to_string())].into_iter().collect(),
                yielded: BTreeSet::new(),
            }),
            at_commit: Some(Ok(runs)),
            ..Fake::default()
        }
    }

    /// A run of #8's head on its branch, completed with `conclusion` (or in flight with `status`
    /// when that is not `completed`), its attempt started and last updated `minutes_ago`.
    fn ended(id: u64, status: &str, conclusion: Option<&str>, minutes_ago: u64) -> RunObservation {
        let mut r = run(id, "h", 8, status, minutes_ago);
        r.head_branch = "fix/y".into();
        r.conclusion = conclusion.map(str::to_string);
        r.updated_at = Some(at(minutes_ago));
        r.jobs.clear();
        r
    }

    #[test]
    fn a_head_one_runner_left_bare_is_run_again_by_any_other_from_the_forge_alone() {
        // the review of #2645 at 68fa1c17b6c: lundra's three runners each keep a trail of their
        // own, and the hosted lane starts from a fresh checkout. Runner one cancels #8's last
        // run and its re-run is refused; runner two, whose trail has never heard of it, is the
        // one that takes the next pass
        let m = model();
        let one = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_rerun = Some("gh: Server Error (HTTP 502)");
        let r = supervise(one.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.acts[0].outcome, ActOutcome::RerunFailed, "{r:?}");
        assert_eq!(
            r.exit_code(),
            12,
            "the pass whose own ask was refused says so"
        );
        let on_one = trail(one.path()).len();
        let two = repo();
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        f.cancelled.borrow_mut().push(300);
        let r = supervise(two.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.decisions.is_empty());
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        assert_eq!(r.recovered.len(), 1, "{r:?}");
        assert_eq!(r.recovered[0].outcome, RecoveryOutcome::Rerun);
        assert_eq!(r.recovered[0].attempt, 2, "GitHub's attempt 1, re-run as 2");
        assert_eq!(r.recovered[0].pull_request, 8);
        assert_eq!(r.exit_code(), 0);
        // runner two's trail records the ask, and runner one's is untouched: neither decides
        let t: Vec<_> = trail(two.path()).into_iter().map(|e| e.action).collect();
        assert_eq!(
            t,
            vec![
                IntegrationAction::RunRerunAttempted,
                IntegrationAction::RunRerunRequested
            ]
        );
        assert_eq!(trail(one.path()).len(), on_one);
        // and a lane with a trail full of the run's story reads the same forge the same way:
        // the trail is a record, never the source
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        f.cancelled.borrow_mut().push(300);
        let r = supervise(one.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.recovered[0].outcome, RecoveryOutcome::Rerun, "{r:?}");
    }

    #[test]
    fn the_newest_run_needs_no_story_whoever_cancelled_it() {
        // finding 1 of the review of 68fa1c17b6c: #8 was opened with a label, run A (300) and
        // its twin B (301); B was cancelled by hand, or by another lane as a duplicate, so this
        // trail tells no story of it; A then ran past its bound, its paired cancel landed and
        // its re-run was refused. B, started later, is the head's newest run. The trail-read
        // recovery found no story of B and gave up with exit 0, for good
        let m = model();
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_rerun = Some("gh: Server Error (HTTP 502)");
        supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(
            trail(d.path()).iter().all(|e| e.run == Some(300)),
            "B is on no trail here"
        );
        let a = ended(300, "completed", Some("cancelled"), 2);
        let mut b = ended(301, "completed", Some("cancelled"), 1);
        b.created_at = at(299);
        let mut f = bare_head(vec![a, b]);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![301], "the newest is asked: {r:?}");
        assert_eq!(r.recovered[0].outcome, RecoveryOutcome::Rerun, "{r:?}");
        assert_eq!(r.exit_code(), 0);
        // and with no trail at all, a head a person cancelled by hand is recovered the same way
        let fresh = repo();
        let mut f = bare_head(vec![ended(310, "completed", Some("cancelled"), 5)]);
        let r = supervise(fresh.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![310], "{r:?}");
    }

    #[test]
    fn a_head_is_run_again_until_githubs_own_attempt_reaches_the_bound_then_held() {
        let m = model();
        let d = repo();
        // attempts one and two are asked again
        for attempt in 1..MAX_ATTEMPTS {
            let mut gone = ended(300, "completed", Some("cancelled"), 1);
            gone.run_attempt = attempt;
            let mut f = bare_head(vec![gone]);
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
            assert_eq!(*f.rerun.borrow(), vec![300], "attempt {attempt}: {r:?}");
            assert_eq!(r.recovered[0].attempt, attempt + 1);
        }
        // at the bound nothing is asked: held as head_exhausted, a warning naming #8, exit 0 on
        // this pass and on every later one that finds it, from any runner
        for root in [repo(), repo()] {
            for _ in 0..3 {
                let mut gone = ended(300, "completed", Some("cancelled"), 1);
                gone.run_attempt = MAX_ATTEMPTS;
                let mut f = bare_head(vec![gone]);
                let r = supervise(root.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
                assert!(f.rerun.borrow().is_empty(), "{r:?}");
                assert!(r.recovered.is_empty(), "{r:?}");
                assert_eq!(
                    r.held.last().map(|h| &h.reason),
                    Some(&crate::integration::supervise::HoldReason::HeadExhausted {
                        pull_request: 8,
                        attempts: MAX_ATTEMPTS
                    }),
                    "{r:?}"
                );
                assert_eq!(
                    r.exit_code(),
                    0,
                    "one stuck pull request is no stopped supervisor"
                );
                let mut out = Vec::new();
                crate::commands::prs::render_supervise(&r, &mut out).unwrap();
                let text = String::from_utf8(out).unwrap();
                assert!(
                    text.lines()
                        .any(|l| l.starts_with("::warning ") && l.contains("#8")),
                    "{text}"
                );
                assert!(
                    trail(root.path()).is_empty(),
                    "nothing asked, nothing recorded"
                );
            }
        }
        // a recovery re-run the forge refuses, on every pass and from every runner, is a warning
        // naming #8 and never a red lane (finding 4: a refusal that persists would fail every
        // pass); the next pass asks again at the same attempt, since the forge counted none
        for root in [repo(), repo()] {
            for _ in 0..3 {
                let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
                f.refuse_rerun = Some("gh: Server Error (HTTP 502)");
                let r = supervise(root.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
                assert_eq!(
                    r.recovered[0].outcome,
                    RecoveryOutcome::RerunFailed,
                    "{r:?}"
                );
                assert!(r.recovered[0].detail.contains("#8 has no run"), "{r:?}");
                assert_eq!(
                    r.exit_code(),
                    0,
                    "a refusal that persists is no stopped supervisor"
                );
                let mut out = Vec::new();
                crate::commands::prs::render_supervise(&r, &mut out).unwrap();
                let text = String::from_utf8(out).unwrap();
                assert!(
                    text.lines()
                        .any(|l| l.starts_with("::warning ") && l.contains("#8")),
                    "{text}"
                );
            }
        }
        let d = repo();
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        f.refuse_rerun = Some("gh: Server Error (HTTP 502)");
        supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        f.refuse_rerun = None;
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.recovered[0].outcome, RecoveryOutcome::Rerun, "{r:?}");
        assert_eq!(r.recovered[0].attempt, 2);
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn a_head_that_has_a_run_or_a_verdict_or_was_cancelled_long_ago_is_left_alone() {
        let m = model();
        let cancelled = ended(300, "completed", Some("cancelled"), 10);
        let window = RECOVERY_WINDOW_MINUTES;
        for (runs, why) in [
            (
                vec![ended(300, "completed", Some("success"), 10)],
                "it passed",
            ),
            (
                vec![
                    cancelled.clone(),
                    ended(301, "completed", Some("failure"), 5),
                ],
                "another run of the head has a verdict",
            ),
            (
                vec![cancelled.clone(), ended(301, "queued", None, 5)],
                "another run of the head is on its way",
            ),
            (
                vec![ended(300, "completed", Some("skipped"), 10)],
                "its newest run was skipped, not cancelled",
            ),
            (
                vec![ended(300, "completed", Some("cancelled"), window + 1)],
                "its newest run was cancelled before the window",
            ),
            (Vec::new(), "it has no run at all"),
        ] {
            let d = repo();
            let mut f = bare_head(runs);
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
            assert!(r.recovered.is_empty(), "{why}: {r:?}");
            assert!(f.rerun.borrow().is_empty(), "{why}");
            assert!(
                !r.held.iter().any(|h| matches!(
                    h.reason,
                    crate::integration::supervise::HoldReason::HeadExhausted { .. }
                )),
                "{why}"
            );
            assert_eq!(r.exit_code(), 0, "{why}");
        }
        // another branch's run of the same commit (a train opened at its member's tip) is not
        // the pull request's
        let d = repo();
        let mut other = cancelled.clone();
        other.head_branch = "integration/x".into();
        let mut f = bare_head(vec![other]);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.recovered.is_empty(), "{r:?}");
        // a head whose runs cannot be read is a warning, not a failed pass: the next pass reads
        let d = repo();
        let mut f = bare_head(Vec::new());
        f.at_commit = Some(Err("gh: Server Error (HTTP 502)"));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.unread.iter().any(|u| u.contains("#8")), "{r:?}");
        assert_eq!(r.exit_code(), 0);
        let mut out = Vec::new();
        crate::commands::prs::render_supervise(&r, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("::warning title=supervise::#8's head h"),
            "{text}"
        );
        // nor is a listing of cancelled runs that cannot be read: the rotating heads are read
        let d = repo();
        let mut f = bare_head(vec![cancelled.clone()]);
        f.listed_cancelled = Some(Err("gh: Server Error (HTTP 502)"));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.unread.len(), 1, "{r:?}");
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        assert_eq!(r.exit_code(), 0);
        // a dry run asks nothing and reads no commit
        let d = repo();
        let mut f = bare_head(vec![cancelled]);
        f.at_commit = Some(Err("not asked"));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, false, None).unwrap();
        assert!(r.recovered.is_empty() && r.unread.is_empty(), "{r:?}");
        assert!(f.commits_asked.borrow().is_empty());
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn one_pass_asks_a_bounded_number_of_re_runs_and_the_next_asks_the_rest() {
        let m = model();
        let d = repo();
        let prs: Vec<u64> = (1..=RECOVERY_RERUNS as u64 + 2).collect();
        let mut gone = Vec::new();
        for n in &prs {
            let mut r = ended(500 + n, "completed", Some("cancelled"), 1);
            r.pull_request = Some(*n);
            r.head_branch = format!("fix/{n}");
            r.head_sha = format!("h{n}");
            gone.push(r);
        }
        let mut f = Fake {
            obs: Some(RunsObservation {
                repository: "o/r".into(),
                default_branch: "master".into(),
                runs: Vec::new(),
                pr_heads: prs.iter().map(|n| (*n, format!("h{n}"))).collect(),
                full: BTreeSet::new(),
                pr_branches: prs.iter().map(|n| (*n, format!("fix/{n}"))).collect(),
                yielded: BTreeSet::new(),
            }),
            listed_cancelled: Some(Ok(gone.clone())),
            at_commit: Some(Ok(gone.clone())),
            ..Fake::default()
        };
        f.cancelled.borrow_mut().extend(gone.iter().map(|r| r.id));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        let count = |o| r.recovered.iter().filter(|x| x.outcome == o).count();
        assert_eq!(count(RecoveryOutcome::Rerun), RECOVERY_RERUNS, "{r:?}");
        assert_eq!(count(RecoveryOutcome::Deferred), 2, "{r:?}");
        assert_eq!(
            r.exit_code(),
            0,
            "a deferred head is asked by the next pass"
        );
    }

    #[test]
    fn the_heads_read_are_the_recently_cancelled_first_then_a_rotating_few() {
        let m = model();
        let n_prs = 30u64;
        let obs = RunsObservation {
            repository: "o/r".into(),
            default_branch: "master".into(),
            runs: vec![{
                // #3 has a run in flight: it is not read
                let mut r = run(903, "h3", 3, "queued", 5);
                r.head_branch = "fix/3".into();
                r
            }],
            pr_heads: (1..=n_prs).map(|n| (n, format!("h{n}"))).collect(),
            full: BTreeSet::new(),
            pr_branches: (1..=n_prs).map(|n| (n, format!("fix/{n}"))).collect(),
            yielded: BTreeSet::new(),
        };
        let cancel = |n: u64, minutes_ago: u64| {
            let mut r = run(700 + n, &format!("h{n}"), n, "completed", minutes_ago);
            r.head_branch = format!("fix/{n}");
            r.conclusion = Some("cancelled".into());
            r.updated_at = Some(at(minutes_ago));
            r
        };
        // #20 cancelled a minute ago, #10 an hour ago, #5 before the window
        let listed = vec![
            cancel(20, 1),
            cancel(10, 60),
            cancel(5, RECOVERY_WINDOW_MINUTES + 5),
        ];
        let skip: BTreeSet<u64> = [7].into_iter().collect();
        let read = heads_to_read(&obs, &obs.runs, &listed, &m, &skip, NOW);
        let numbers: Vec<u64> = read.iter().map(|(n, _)| *n).collect();
        assert_eq!(&numbers[..2], &[20, 10], "newest cancel first: {numbers:?}");
        assert_eq!(numbers.len(), 2 + ROTATING_READS, "{numbers:?}");
        // over enough ten-minute slots every other head is read, and #3 (in flight) and #7
        // (acted on this pass) never
        let mut seen = BTreeSet::new();
        for slot in 0..n_prs {
            for (n, _) in heads_to_read(&obs, &obs.runs, &listed, &m, &skip, NOW + slot * 600) {
                seen.insert(n);
            }
        }
        let want: BTreeSet<u64> = (1..=n_prs).filter(|n| *n != 3 && *n != 7).collect();
        assert_eq!(seen, want);
        // two runners at the same moment read the same heads
        assert_eq!(
            read,
            heads_to_read(&obs, &obs.runs, &listed, &m, &skip, NOW)
        );
    }

    #[test]
    fn a_hung_twin_is_never_run_again_over_its_older_twins_verdict() {
        // finding 3 of the review of 68fa1c17b6c, #2678's shape: twins A (299) and B (300) of
        // #8's head; A passed, the group admitted B, and B's job hung past its bound. The
        // observation lists only B, so B reads as the head's last run and its cancel is paired;
        // the commit's runs read whole say A has the verdict, so B is ended and not run again
        let m = model();
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        let b = f.obs.as_ref().unwrap().runs[0].clone();
        let mut a = ended(299, "completed", Some("success"), 301);
        a.created_at = at(302);
        f.at_commit = Some(Ok(vec![a, b]));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.decisions[0].rerun, "decided as the head's last: {r:?}");
        assert_eq!(*f.cancelled.borrow(), vec![300], "the runner is freed");
        assert!(
            f.rerun.borrow().is_empty(),
            "no newer ci over the pass: {r:?}"
        );
        assert_eq!(r.acts[0].outcome, ActOutcome::Cancelled, "{r:?}");
        assert!(r.acts[0].detail.contains("has its verdict"), "{r:?}");
        assert_eq!(r.exit_code(), 0);
        // a head whose runs cannot be read is not asked either: the act failed, and the next
        // pass judges the head from the forge
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.at_commit = Some(Err("gh: Server Error (HTTP 502)"));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::RerunFailed, "{r:?}");
        assert_eq!(r.exit_code(), 12);
        // and a run at GitHub's attempt bound is ended, not asked: the head is held exhausted
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.obs.as_mut().unwrap().runs[0].run_attempt = MAX_ATTEMPTS;
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.cancelled.borrow(), vec![300]);
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert!(
            r.held.iter().any(|h| h.reason
                == crate::integration::supervise::HoldReason::HeadExhausted {
                    pull_request: 8,
                    attempts: MAX_ATTEMPTS
                }),
            "{r:?}"
        );
        assert_eq!(r.exit_code(), 0);
    }

    /// A job of #8's run, completed with `conclusion`, started `minutes_ago`.
    fn job(name: &str, conclusion: &str, minutes_ago: u64) -> JobObservation {
        JobObservation {
            name: name.into(),
            status: "completed".into(),
            started_at: Some(at(minutes_ago)),
            created_at: Some(at(minutes_ago)),
            conclusion: Some(conclusion.into()),
        }
    }

    /// The jobs of a validate run that went red and cancelled itself (I2664): a gate failed, the
    /// rest were cancelled, and `ci`, `if: always()`, wrote the failure: 40906901071's shape.
    fn red_and_cancelled(minutes_ago: u64) -> Vec<JobObservation> {
        vec![
            job("plan", "success", minutes_ago),
            job("site-probe-shard (1)", "failure", minutes_ago),
            job("suite-shard (1)", "cancelled", minutes_ago),
            job("ci", "failure", minutes_ago),
        ]
    }

    #[test]
    fn a_red_run_that_cancelled_itself_is_its_heads_verdict_and_is_never_run_again() {
        // the review of #2645 at 859039b21de: at 18:49Z every head recovery would have re-run
        // was one of these (#2682 run 40906901071, #2520 40885641288, #2687 40868113225). The
        // run reads `cancelled`, its `ci` reads failure: a re-run would write pending over the
        // red, spend a suite, and could turn a real red green
        let m = model();
        for attempt in [1, MAX_ATTEMPTS] {
            let d = repo();
            let mut gone = ended(300, "completed", Some("cancelled"), 1);
            gone.run_attempt = attempt;
            let mut f = bare_head(vec![gone]);
            f.job_reads.insert(300, red_and_cancelled(1));
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
            assert!(f.rerun.borrow().is_empty(), "attempt {attempt}: {r:?}");
            assert!(r.recovered.is_empty(), "{r:?}");
            assert!(r.unread.is_empty(), "{r:?}");
            assert!(
                r.held.is_empty(),
                "a head with its verdict is not exhausted, at any attempt: {r:?}"
            );
            assert_eq!(*f.jobs_asked.borrow(), vec![300], "its jobs were read");
            assert_eq!(r.exit_code(), 0);
            assert!(trail(d.path()).is_empty());
        }
        // a cancel that let the always() jobs run writes `ci` too, red or green: a verdict
        for ci in ["failure", "success"] {
            let d = repo();
            let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
            f.job_reads
                .insert(300, vec![job("suite", "cancelled", 1), job("ci", ci, 1)]);
            let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
            assert!(f.rerun.borrow().is_empty(), "ci {ci}: {r:?}");
        }
        // the twin cancelled while it waited in its group has no job: the read goes past it to
        // the run beside it, A, which went red
        let d = repo();
        let a = ended(299, "completed", Some("cancelled"), 3);
        let b = ended(300, "completed", Some("cancelled"), 2);
        let mut f = bare_head(vec![a, b]);
        f.job_reads.insert(299, red_and_cancelled(3));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(*f.jobs_asked.borrow(), vec![300, 299], "newest first");
        // what a force-cancel leaves (the always() jobs skipped, `ci` cancelled) is no verdict,
        // and is asked again: the head has no `ci` until it runs
        let d = repo();
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        f.job_reads.insert(
            300,
            vec![
                job("plan", "success", 1),
                job("suite", "cancelled", 1),
                job("ci", "cancelled", 1),
            ],
        );
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        // however many twins with no job stand before the run that went red, the reads go past
        // them all: none of them overrides its `ci`
        let d = repo();
        let mut runs: Vec<RunObservation> = (1..=6)
            .map(|k| ended(300 + k, "completed", Some("cancelled"), k))
            .collect();
        runs.push(ended(300, "completed", Some("cancelled"), 40));
        let mut f = bare_head(runs);
        f.job_reads.insert(300, red_and_cancelled(40));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(
            *f.jobs_asked.borrow(),
            vec![301, 302, 303, 304, 305, 306, 300],
            "newest first, to the red one"
        );
        // and a pass reads no more than its budget: a head past it is said, and left to the
        // next pass, never asked unread
        let d = repo();
        let mut runs: Vec<RunObservation> = (1..=VERDICT_READS as u64)
            .map(|k| ended(300 + k, "completed", Some("cancelled"), k))
            .collect();
        runs.push(ended(300, "completed", Some("cancelled"), 40));
        let mut f = bare_head(runs);
        f.job_reads.insert(300, red_and_cancelled(40));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(f.jobs_asked.borrow().len(), VERDICT_READS, "bounded: {r:?}");
        assert!(
            r.unread
                .iter()
                .any(|u| u.contains("#8") && u.contains("next pass")),
            "{r:?}"
        );
        assert_eq!(r.exit_code(), 0);
        // and the paired re-run of a hung run is not asked over a twin that went red
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        let b = f.obs.as_ref().unwrap().runs[0].clone();
        let mut a = ended(299, "completed", Some("cancelled"), 301);
        a.created_at = at(302);
        f.at_commit = Some(Ok(vec![a, b]));
        f.job_reads.insert(299, red_and_cancelled(301));
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(r.decisions[0].rerun, "{r:?}");
        assert_eq!(*f.cancelled.borrow(), vec![300]);
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::Cancelled, "{r:?}");
        assert!(r.acts[0].detail.contains("in its jobs"), "{r:?}");
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn a_head_that_yielded_to_the_release_path_is_left_to_it_and_its_yields_are_not_counted() {
        // the review of #2645 at 859039b21de: a run that yielded is labelled ci:yielded and
        // cancels itself; `resume` runs it again once the release path is clear. Recovery that
        // re-ran it while the path was busy spent the head's attempts on yields, and a later
        // hang at attempt 4 was then held exhausted with the head left bare
        let m = model();
        let d = repo();
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        f.obs.as_mut().unwrap().yielded.insert(8);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert!(f.commits_asked.borrow().is_empty(), "not even read: {r:?}");
        assert!(r.recovered.is_empty() && r.held.is_empty() && r.unread.is_empty());
        assert_eq!(r.exit_code(), 0);
        // the paired re-run of a hung run of a yielded head is the release path's too: the run
        // is ended, nothing asked, nothing held
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.obs.as_mut().unwrap().yielded.insert(8);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.cancelled.borrow(), vec![300]);
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::Cancelled, "{r:?}");
        assert!(r.acts[0].detail.contains(YIELDED_LABEL), "{r:?}");
        assert!(r.held.is_empty(), "{r:?}");
        assert_eq!(r.exit_code(), 0);
        // once `resume` has run it again, the attempts it took are not the supervisor's: a head
        // at GitHub's attempt 3 that yielded once is asked once more, as attempt 4 of 4
        let d = repo();
        let mut gone = ended(300, "completed", Some("cancelled"), 1);
        gone.run_attempt = MAX_ATTEMPTS;
        let mut f = bare_head(vec![gone]);
        f.yield_notes.insert(300, 1);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        assert_eq!(r.recovered[0].attempt, MAX_ATTEMPTS + 1);
        assert!(r.recovered[0].detail.contains("of at most 4"), "{r:?}");
        assert_eq!(*f.yields_asked.borrow(), vec![300]);
        // and at attempt 5 with one yield, it is spent
        let mut gone = ended(300, "completed", Some("cancelled"), 1);
        gone.run_attempt = MAX_ATTEMPTS + 1;
        let mut f = bare_head(vec![gone]);
        f.yield_notes.insert(300, 1);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.rerun.borrow().is_empty(), "{r:?}");
        assert!(matches!(
            r.held.last().map(|h| &h.reason),
            Some(crate::integration::supervise::HoldReason::HeadExhausted {
                pull_request: 8,
                ..
            })
        ));
        // below the bound the notes are never read: they cost nothing on a healthy head
        let mut f = bare_head(vec![ended(300, "completed", Some("cancelled"), 1)]);
        supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert!(f.yields_asked.borrow().is_empty());
        // the act path's paired re-run counts the same way: attempt 3 that yielded twice
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.obs.as_mut().unwrap().runs[0].run_attempt = MAX_ATTEMPTS;
        f.yield_notes.insert(300, 2);
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(*f.rerun.borrow(), vec![300], "{r:?}");
        assert_eq!(r.acts[0].outcome, ActOutcome::CancelledAndRerun, "{r:?}");
        assert!(r.held.is_empty(), "{r:?}");
    }

    #[test]
    fn the_release_paths_label_and_note_are_the_ones_its_script_writes() {
        // the label and the note are the release path's words, read here: a rename there must
        // fail here, not leave recovery fighting the yield again
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let model = std::fs::read_to_string(root.join(".ai/repo/ci/release-path.yaml")).unwrap();
        assert!(
            model
                .lines()
                .any(|l| l.trim() == format!("label: {YIELDED_LABEL}")),
            "release-path.yaml names another label than {YIELDED_LABEL}"
        );
        let script = std::fs::read_to_string(root.join("scripts/ci/release-path")).unwrap();
        let note = format!("body={YIELD_NOTE}$2{YIELD_NOTE_AFTER}");
        assert!(
            script.contains(&note),
            "release-path no longer writes `{note}`"
        );
        assert!(is_yield_note(
            "Validate run 40825938755 of this pull request yielded to the release path: the \
             release run of v1.0.0, run 1 (queued).",
            40825938755
        ));
        assert!(!is_yield_note(
            "Validate run 408259387551 of this pull request yielded to the release path",
            40825938755
        ));
        assert!(!is_yield_note(
            "Validate run 40825938755 failed",
            40825938755
        ));
        assert!(within_attempts(MAX_ATTEMPTS - 1, 0));
        assert!(!within_attempts(MAX_ATTEMPTS, 0));
        assert!(within_attempts(MAX_ATTEMPTS + 1, 2));
    }

    #[test]
    fn the_forges_json_is_read_into_runs_and_jobs() {
        let v = json!({
            "id": 40733537887u64, "name": "validate", "path": ".github/workflows/validate.yml",
            "event": "pull_request", "head_branch": "integration/10a", "head_sha": "abc",
            "status": "pending", "created_at": "2026-10-28T13:00:00Z",
            "run_started_at": "2026-10-28T13:00:00Z",
            "head_repository": {"id": 1},
            "pull_requests": [{"number": 2631,
                "head": {"sha": "abc", "ref": "integration/10a", "repo": {"id": 1}}}]
        });
        let r = run_of(&v).unwrap();
        assert_eq!(r.workflow, "validate");
        assert_eq!(r.pull_request, Some(2631));
        assert_eq!(r.status, "pending");
        assert_eq!(
            r.run_attempt, 1,
            "a run the forge gives no attempt is its first"
        );
        // of every pull request the forge lists, the one whose head is the run's branch
        let mut both = v.clone();
        both["pull_requests"] = json!([
            {"number": 2600, "head": {"ref": "fix/x", "repo": {"id": 1}}},
            {"number": 2631, "head": {"ref": "integration/10a", "repo": {"id": 1}}}
        ]);
        both["run_attempt"] = json!(3);
        let b = run_of(&both).unwrap();
        assert_eq!(b.pull_request, Some(2631));
        assert_eq!(b.run_attempt, 3);
        // a fork's pull request of the same branch name is not this run's
        both["pull_requests"] = json!([
            {"number": 2632, "head": {"ref": "integration/10a", "repo": {"id": 9}}}
        ]);
        assert_eq!(run_of(&both).unwrap().pull_request, None);
        assert!(run_of(&json!({"id": 1})).is_none());
        assert_eq!(
            workflow_of("dynamic/pages/pages-build-deployment"),
            "pages-build-deployment"
        );
        let j = job_of(&json!({"name": "plan", "status": "queued", "started_at": null})).unwrap();
        assert!(!j.started());
        // the forge fills started_at for a queued job too: it is not started
        let j = job_of(&json!({"name": "rust (1)", "status": "queued",
            "started_at": "2026-10-28T22:24:29Z", "conclusion": null}))
        .unwrap();
        assert!(!j.started());
        let j = job_of(&json!({"name": "plan", "status": "completed",
            "started_at": "2026-10-28T22:24:09Z", "conclusion": "success"}))
        .unwrap();
        assert!(j.started());
        assert_eq!(j.conclusion.as_deref(), Some("success"));
        let lines = lines_of("{\"a\":1}\n\n{\"a\":2}\n", "x").unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines_of("{nope", "x").is_err());
        // a pull request's run the forge named no pull request for is given its branch's
        let mut runs = vec![RunObservation {
            pull_request: None,
            ..r.clone()
        }];
        let branches = [("integration/10a".to_string(), 2631u64)]
            .into_iter()
            .collect();
        attach_pull_requests(&mut runs, &branches);
        assert_eq!(runs[0].pull_request, Some(2631));
    }
}
