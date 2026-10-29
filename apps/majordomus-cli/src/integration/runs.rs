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
//! taken: the forge re-runs only a run that has ended, and accepts a force-cancel before it has,
//! so [`RunForge::rerun`] reads the run until it reads completed ([`RERUN_READS`] reads,
//! [`RERUN_WAIT_SECONDS`] apart) and then asks once. Its outcome is on the trail after the
//! cancel's (`run_rerun_requested` or `run_rerun_failed`), and a re-run that could not be asked
//! is a failed act (exit 12): the head has no run until somebody re-runs it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::drain::{self, IntegrationAction, IntegrationEvent};
use super::forge::{gh_within, ForgeError};
use super::supervise::{
    self, Decision, Hold, JobObservation, Liveness, RunObservation, Situation, SuperviseModel,
    FULL_LABEL, IN_FLIGHT,
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
    /// it. Err when it did not end in time or the forge refused.
    fn rerun(&mut self, id: u64) -> Result<(), ForgeError>;
    /// When the newest successful job of `workflow` ended, RFC 3339; `None` when there is none
    /// among its recent runs, or none of them `among` admits.
    fn newest_success(
        &self,
        workflow: &str,
        among: Successes<'_>,
    ) -> Result<Option<String>, ForgeError>;
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
        let mut full = BTreeSet::new();
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
                branches.insert(b, n);
            }
            let labelled = v
                .get("labels")
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(|l| l.as_str() == Some(FULL_LABEL)));
            if labelled {
                full.insert(n);
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

    fn rerun(&mut self, id: u64) -> Result<(), ForgeError> {
        // a force-cancel is accepted before the run has ended, and the forge re-runs only a run
        // that has: read it until it reads completed, then ask once, never again
        let endpoint = format!("repos/{}/actions/runs/{id}", self.repository);
        let mut ended = false;
        for read in 0..RERUN_READS {
            if read > 0 {
                std::thread::sleep(std::time::Duration::from_secs(RERUN_WAIT_SECONDS));
            }
            if self.one(&endpoint)?.get("status").and_then(Value::as_str) == Some("completed") {
                ended = true;
                break;
            }
        }
        if !ended {
            return Err(ForgeError(format!(
                "run {id} still read in flight {RERUN_READS} times after its cancel; its re-run \
                 was not asked"
            )));
        }
        let again = format!("{endpoint}/rerun");
        let (ok, _, err) = call(
            self.root,
            self.deadline,
            &["api", "-X", "POST", &again],
            false,
        )?;
        if ok {
            Ok(())
        } else {
            Err(ForgeError(format!(
                "gh api -X POST {again} failed: {}",
                err.trim()
            )))
        }
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
    /// decisions a dry run found; 12 when an act could not be taken.
    pub fn exit_code(&self) -> u8 {
        if !self.applied {
            return if self.decisions.is_empty() { 0 } else { 10 };
        }
        if self.acts.iter().all(|a| {
            matches!(
                a.outcome,
                ActOutcome::Cancelled | ActOutcome::CancelledAndRerun | ActOutcome::Skipped
            )
        }) {
            0
        } else {
            12
        }
    }
}

fn subject(d: &Decision) -> String {
    format!(
        "run {} of {} ({} on {}): {}: {}{}",
        d.run,
        d.workflow,
        d.event,
        d.head_branch,
        d.reason.word(),
        d.reason.explain(),
        if d.rerun {
            "; the last run of its pull request's head, so it is run again once ended"
        } else {
            ""
        }
    )
}

fn event(action: IntegrationAction, d: &Decision, detail: String) -> IntegrationEvent {
    IntegrationEvent {
        pr: d.pull_request,
        head_sha: Some(d.head_sha.clone()),
        detail,
        ..IntegrationEvent::of(action)
    }
}

/// Why a cancel the forge refused found nothing left to do, or `None` when the refusal stands:
/// the run read again has completed, or the forge answered 409, which it gives for a run that can
/// no longer be cancelled because it completed. A second supervision that decided the same cancel
/// in the same minute (two lanes in one burst) meets exactly this.
fn already_ended(forge: &dyn RunForge, id: u64, refusal: &ForgeError) -> Option<String> {
    match forge.run(id) {
        Ok(r) if !r.in_flight() => Some(format!(
            "it had already ended ({}{})",
            r.status,
            r.conclusion
                .as_deref()
                .map(|c| format!(", {c}"))
                .unwrap_or_default()
        )),
        _ if refusal.0.contains("HTTP 409") => Some(format!(
            "the forge answered that it can no longer be cancelled: {}",
            refusal.0.trim()
        )),
        _ => None,
    }
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
    if apply {
        let mut trail_broken = false;
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
            let (action, outcome, detail) = match forge.force_cancel(d.run) {
                Ok(()) => {
                    if let Some(r) = current.iter_mut().find(|r| r.id == d.run) {
                        r.status = "completed".into();
                        r.conclusion = Some("cancelled".into());
                    }
                    (
                        IntegrationAction::RunCancelled,
                        ActOutcome::Cancelled,
                        format!("force-cancelled {}", subject(&held)),
                    )
                }
                Err(e) => match already_ended(forge, d.run, &e) {
                    // two lanes in one burst can decide the same cancel: the second finds it
                    // done, which is no failure
                    Some(why) => {
                        if let Some(r) = current.iter_mut().find(|r| r.id == d.run) {
                            r.status = "completed".into();
                        }
                        (
                            IntegrationAction::RunAlreadyEnded,
                            ActOutcome::Skipped,
                            format!("not cancelled, {why}: {}", subject(&held)),
                        )
                    }
                    None => (
                        IntegrationAction::RunCancelFailed,
                        ActOutcome::Failed,
                        format!("could not cancel {}: {e}", subject(&held)),
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
            // the pair of a cancel that ended its head's last run: asked whatever the trail
            // said of the cancel, since it is what gives the head a run again
            let mut outcome = outcome;
            if outcome == ActOutcome::Cancelled && held.rerun {
                let (action, then, said) = match forge.rerun(d.run) {
                    Ok(()) => {
                        if let Some(r) = current.iter_mut().find(|r| r.id == d.run) {
                            r.status = "requested".into();
                            r.conclusion = None;
                            r.run_attempt += 1;
                            r.jobs.clear();
                        }
                        (
                            IntegrationAction::RunRerunRequested,
                            ActOutcome::CancelledAndRerun,
                            format!("re-requested run {} of {}", d.run, d.workflow),
                        )
                    }
                    Err(e) => (
                        IntegrationAction::RunRerunFailed,
                        ActOutcome::RerunFailed,
                        format!(
                            "could not re-run run {} of {} after its cancel, and #{} has no run \
                             of its head: {e}",
                            d.run,
                            d.workflow,
                            d.pull_request.unwrap_or_default()
                        ),
                    ),
                };
                outcome = then;
                detail = match drain::record(root, event(action, &held, said.clone())) {
                    Ok(_) => format!("{detail}; {said}"),
                    Err(e) => {
                        trail_broken = true;
                        format!("{detail}; {said} (the trail could not record it: {e})")
                    }
                };
            }
            acts.push(Act {
                run: d.run,
                reason: word,
                outcome,
                detail,
            });
        }
    }
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
        held: decided.held,
        acts,
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
        cancelled: RefCell<Vec<u64>>,
        /// How the forge refuses a re-run, when it does.
        refuse_rerun: Option<&'static str>,
        /// The runs asked to run again, in order.
        rerun: RefCell<Vec<u64>>,
        /// What the supervisor's newest success is; `None` reads as "an hour ago".
        success: Option<Result<Option<String>, String>>,
        /// The runs whose jobs were asked for, in order.
        jobs_asked: RefCell<Vec<u64>>,
        /// How often the newest success was asked for, and for which lane.
        success_asked: RefCell<Vec<Option<String>>>,
    }

    impl RunForge for Fake {
        fn observe(&self) -> Result<RunsObservation, ForgeError> {
            self.obs
                .clone()
                .ok_or_else(|| ForgeError("no forge here".into()))
        }
        fn jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError> {
            self.jobs_asked.borrow_mut().push(id);
            Ok(self
                .obs
                .as_ref()
                .and_then(|o| o.runs.iter().find(|r| r.id == id))
                .map(|r| r.jobs.clone())
                .unwrap_or_default())
        }
        fn run(&self, id: u64) -> Result<RunObservation, ForgeError> {
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
            if let Some(why) = self.refuse_cancel {
                return Err(ForgeError(why.into()));
            }
            self.cancelled.borrow_mut().push(id);
            Ok(())
        }
        fn rerun(&mut self, id: u64) -> Result<(), ForgeError> {
            if !self.cancelled.borrow().contains(&id) {
                return Err(ForgeError(format!("run {id} was never cancelled")));
            }
            if let Some(why) = self.refuse_rerun {
                return Err(ForgeError(why.into()));
            }
            self.rerun.borrow_mut().push(id);
            Ok(())
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
            already_ended(&f, 100, &other).as_deref(),
            Some("it had already ended (completed, cancelled)")
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
                IntegrationAction::RunRerunRequested
            ]
        );
        let first = &trail(d.path())[0].detail;
        assert!(first.contains("run again once ended"), "{first}");
        // a re-run the forge refuses is a failed act on the trail: the head has no run
        let d = repo();
        let mut f = last_run_of_head("in_progress", 300);
        f.refuse_rerun = Some("gh: Resource not accessible by integration (HTTP 403)");
        let r = supervise(d.path(), &mut f, &m, &|| NOW, None, true, None).unwrap();
        assert_eq!(r.acts[0].outcome, ActOutcome::RerunFailed, "{r:?}");
        assert_eq!(r.exit_code(), 12);
        let t = trail(d.path());
        assert_eq!(t[2].action, IntegrationAction::RunRerunFailed);
        assert!(t[2].class.is_some(), "a failed act names its class");
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
