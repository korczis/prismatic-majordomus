//! The supervision of in-flight CI runs: which of them to cancel, and why (I3903).
//!
//! On 2026-10-28 two trains' new `validate` runs sat `pending` for hours behind their own
//! superseded runs. Validate's concurrency group is one per pull request and never cancels
//! in progress, on purpose: a started run reaches its verdict, and #576 was once cancelled
//! four times in a row without ever producing one (`.github/workflows/validate.yml`, the
//! comment above `concurrency:`). So the newer commit waits, and when the older run is of a
//! head nobody will merge any more, it waits for a verdict nobody reads.
//!
//! This module decides, from plain data, which runs end. It reads nothing and acts on
//! nothing: [`decide`] takes the in-flight runs, each open pull request's current head, the
//! bounds `.ai/repo/ci/supervise.yaml` declares and the moment, and returns a [`Decision`]
//! for each run to cancel with its typed [`CancelReason`], and a [`Hold`] for each run it
//! looked at and deliberately left alone. The forge side, which observes and acts, is
//! [`super::runs`]; a dry run and `--apply` call this same function, so they decide alike.
//!
//! Four reasons, and nothing else, end a run:
//!
//! - [`CancelReason::DuplicateRun`]: a `pull_request` run that waits in its group with no job
//!   created while an older run of the same pull request, branch and commit, created within
//!   [`TWIN_WINDOW_SECONDS`] of it, is in flight and will reach its verdict. A train opened
//!   with `--label train` is `opened` and `labeled` at once, and both events start validate:
//!   6 of the 8 trains of 2026-10-28 had such a twin, and train 10b's (40713048283) held its
//!   group for seven hours. Three limits keep the rule to that burst:
//!   - *no job created*. The forge creates a run's jobs when its group admits it, and from then
//!     on a cancel still runs its `if: always()` jobs: validate's required `ci` then writes a
//!     newer check that is not a success over the pass of the twin it duplicated (#2678 at
//!     3a45c8c7, 2026-10-29: run 40811621641, admitted when 40811610939 passed at 05:31:53Z, was
//!     cancelled at 05:58Z and its `ci` failed, which blocked the pull request). So a twin is
//!     never cancelled once admitted, and a judged run is never the older half of a pair: its
//!     twin has been admitted by then.
//!   - *the older run reaches its verdict*. When the older run is itself to be ended, now or
//!     within [`TWIN_HORIZON_MINUTES`], by a bound or for being superseded, or was cancelled
//!     already and runs only its `if: always()` jobs, the twin is held
//!     ([`HoldReason::EarlierRunEnds`]): cancelling both would leave the commit with no run.
//!   - *one burst*. A run created later than the window was asked for — a reopen tests the head
//!     against a master that moved, a label added hours later asks again — and so is a re-run
//!     (`run_attempt` above one, [`HoldReason::RerunRequested`]) and any second run of a pull
//!     request labelled [`FULL_LABEL`] ([`HoldReason::FullRunRequested`]).
//! - [`CancelReason::SupersededBlocksSuccessor`]: a `pull_request` run of a workflow whose
//!   concurrency group is one per pull request, whose head is no longer its pull request's
//!   head, while a newer run of the same pull request at its current head waits in the same
//!   group. Cancelling it is what lets the verdict that matters start; a newer run of the old
//!   commit (its twin) is no such verdict. With no successor waiting it is left alone
//!   ([`HoldReason::SupersededWithoutSuccessor`]): that is the concurrency rationale.
//! - [`CancelReason::QueuedPastBound`]: its attempt queued longer than its workflow's declared
//!   bound with no job of it started, measured from [`RunObservation::queued_since`]: its
//!   admission by its concurrency group, the creation of the attempt's first job. Neither its
//!   creation nor `run_started_at` is that: a twin that waited hours in its group behind its
//!   older run is admitted with both hours old, and a clock from them would end it at once (and
//!   its `if: always()` `ci` would write a failure). A run with no job of its attempt is pending
//!   in its group and is never ended for waiting. Only a pull request's run is ended this way,
//!   since a run that waits in a queue holds no runner and frees nothing for anyone but a
//!   newer run of its own pull request; and at its pull request's current head only while
//!   another run of that head is left to reach a verdict ([`HoldReason::LastRunOfHead`], "A
//!   head keeps its run" below): an
//!   `integration/*` top waits for a runner and its verdict is the one a landing needs
//!   ([`HoldReason::IntegrationTopWaits`]), and a run of the trunk, a tag, the schedule, a
//!   dispatch or a release is in a group of its own ([`HoldReason::ProtectedRunWaits`]).
//! - [`CancelReason::RunningPastBound`]: a job of it in progress longer than its workflow's
//!   declared bound, which is the workflow's largest job timeout plus thirty minutes: a job
//!   past it should have been ended by its own timeout and was not. It is each running job
//!   that is judged ([`RunObservation::longest_running_job`]), never the run: validate's jobs
//!   run in a chain of `needs` with the runner queue between the stages, and of 59 recent
//!   validate runs at least 8 ran past 255 minutes from first job to last while every job was
//!   within its own (40576851731 took 579 minutes and succeeded). A completed job is no job's
//!   time either, so the jobs a re-run carries over from an earlier attempt, listed with their
//!   first start, never count (38013111243's attempt 2, 40747313397's attempt 3). An
//!   `integration/*` top whose job runs past it is held ([`HoldReason::IntegrationTopRuns`]).
//!   The last run of a pull request's head ended this way is re-run once it has ended
//!   ([`Decision::rerun`]).
//!
//! What is never cancelled for being superseded is fixed here, not declared: a push to the
//! default branch, any other push (a tag push among them), the schedule, a dispatch, a
//! release, and every run of the pages or release workflows ([`Protection`]). The supervising
//! workflow's own runs are never cancelled for any reason.
//!
//! # A head keeps its run
//!
//! The four reasons judge one run each, and a run judged alone can be the last one its pull
//! request's head has. The review of #2645 at eed0b4f8ff4 found two ways to end it: a run at the
//! head queued past its bound with nothing behind it, and a twin cancelled as a duplicate while
//! its older run was young, whose older run a bound ended hours later. A force-cancel skips the
//! `if: always()` jobs, so validate's required `ci` is then never written and the pull request
//! waits for a check that will not come, with nothing asking for a run again. So [`decide`] holds
//! every pass to one invariant, and [`still_holds`] holds every act to it again over its fresh
//! reading:
//!
//! > an open pull request's current head that has a run on its way to a verdict
//! > ([`RunObservation::toward_verdict`]), or a verdict among the runs observed, still has one
//! > once the pass has acted.
//!
//! A cancel that would end the last such run ([`head_keeps_a_run`]) is settled by what the run
//! holds. A run that waits holds no runner, so ending it frees nothing: a `queued_past_bound` or
//! `duplicate_run` cancel is not made, and the run is held ([`HoldReason::LastRunOfHead`]). A run
//! with a job past its bound holds a runner that its own timeout did not free, and freeing it is
//! the bound's purpose: the `running_past_bound` cancel is made, paired with a re-run of the same
//! run once it has ended ([`Decision::rerun`]), as the release path's `resume` re-runs a run that
//! yielded; the re-run is on the trail with the cancel. A `superseded_blocks_successor` cancel
//! never meets the rule: its run is not at the head, and its successor is.
//!
//! That keeps the incidents I3903 was opened for fixed. The runs of 2026-10-28 that waited for
//! hours waited behind runs of heads nobody would merge (10a's, 10b's 40764355688) or behind a
//! same-commit twin whose older run was in flight (10b's 40713048283): neither cancel ends a
//! head's last run, and both are made as before. Holding a twin until its older run has a
//! verdict, and cancelling it only on a success, was weighed and refused: the group admits the
//! twin the moment its older run completes, an admitted twin is never cancelled (#2678 above),
//! so that rule would never cancel a twin at all and would bring back 10b's seven-hour twin.
//! Cancelling a twin early is safe instead because a later bound can no longer end its older
//! run without leaving the head a run.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where the bounds are declared.
pub const MODEL: &str = ".ai/repo/ci/supervise.yaml";

/// The workflows whose runs are never cancelled for being superseded: publication and the
/// release are each the only reading of what they produce.
pub const PROTECTED_WORKFLOWS: &[&str] = &["pages", "release"];

/// The statuses of a run that has not completed, as the forge words them.
pub const IN_FLIGHT: &[&str] = &["queued", "in_progress", "waiting", "pending", "requested"];

/// The label with which a pull request asks for every gate, and so for a second full run of
/// a head that already has one: its same-head twin is held, not cancelled.
pub const FULL_LABEL: &str = "ci:full";

/// The branches of the landing seat's combined tops: never ended for waiting in the queue.
pub const INTEGRATION_PREFIX: &str = "integration/";

/// How close in time two runs of one commit must have been created for the newer one to be the
/// older one's duplicate: the `opened` and `labeled` runs of a train are one second apart. A
/// run created later than this was asked for.
pub const TWIN_WINDOW_SECONDS: u64 = 120;

/// Minutes after which the newest successful supervision is reported as `supervise_stale`.
pub const STALE_MINUTES: u64 = 30;

/// How far ahead the older run of a twin is judged before the twin is cancelled: the longest a
/// live supervisor goes between two passes ([`STALE_MINUTES`]). An older run that a bound would
/// end by then keeps its twin, which takes its place, and no re-run is needed. A live
/// supervisor judges a twin within minutes of its creation, when its older run is far from
/// either bound, so this rarely holds one; what keeps the head a run when the bound comes later
/// is the invariant of [`decide`] (a head keeps its run), not this horizon.
pub const TWIN_HORIZON_MINUTES: u64 = STALE_MINUTES;

/// The finding of a supervisor whose newest success is older than [`STALE_MINUTES`].
pub const SUPERVISE_STALE: &str = "supervise_stale";

/// The finding of a supervisor whose newest success could not be read.
pub const SUPERVISE_UNOBSERVED: &str = "supervise_unobserved";

/// One job of a run, as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobObservation {
    /// The job's name.
    pub name: String,
    /// `queued`, `in_progress`, `completed`, `waiting`, ...
    pub status: String,
    /// When a runner took it, RFC 3339. The forge also fills it for a job still `queued`
    /// (with its creation), so it says when only once [`JobObservation::started`] says whether.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When the forge created it, RFC 3339: when its run's concurrency group admitted the run,
    /// for the first job of an attempt ([`RunObservation::queued_since`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// `success`, `failure`, `skipped`, ... once completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
}

impl JobObservation {
    /// Whether a runner ever took this job: in progress, or completed without being skipped.
    /// A `queued` job is not started whatever its `started_at` says.
    pub fn started(&self) -> bool {
        match self.status.as_str() {
            "in_progress" => true,
            "completed" => self.conclusion.as_deref() != Some("skipped"),
            _ => false,
        }
    }
}

/// One workflow run that has not completed, as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunObservation {
    /// The run's id.
    pub id: u64,
    /// The workflow's file name without `.yml` (`validate`), the key of its bound.
    pub workflow: String,
    /// The event that started it: `pull_request`, `push`, `schedule`, `workflow_dispatch`, ...
    pub event: String,
    /// The branch (or, for a tag push, the tag) it ran on.
    pub head_branch: String,
    /// The commit it ran on.
    pub head_sha: String,
    /// The pull request it belongs to, when the forge named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// One of [`IN_FLIGHT`], or `completed` when it has ended since.
    pub status: String,
    /// `success`, `failure`, `cancelled`, ... once completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// When it was created, RFC 3339.
    pub created_at: String,
    /// When its current attempt started, RFC 3339; the forge sets it at creation and again on
    /// a re-run, so it is not when a job of it started to run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_started_at: Option<String>,
    /// Which attempt it is: 1, or more once re-run. A re-run keeps its id and `created_at`.
    #[serde(default = "first_attempt")]
    pub run_attempt: u64,
    /// Its jobs.
    #[serde(default)]
    pub jobs: Vec<JobObservation>,
}

fn first_attempt() -> u64 {
    1
}

impl RunObservation {
    /// Whether it has not completed.
    pub fn in_flight(&self) -> bool {
        IN_FLIGHT.contains(&self.status.as_str())
    }

    /// Whether it waits rather than runs: in flight and not `in_progress`.
    pub fn waiting(&self) -> bool {
        self.in_flight() && self.status != "in_progress"
    }

    /// Whether `job` is one an earlier attempt left behind: completed, and started before this
    /// attempt did. On a re-run of the failed jobs the forge lists the jobs it reuses with their
    /// first start.
    pub fn carried(&self, job: &JobObservation) -> bool {
        let attempt = self.run_started_at.as_deref().and_then(seconds);
        let started = job.started_at.as_deref().and_then(seconds);
        job.status == "completed" && matches!((started, attempt), (Some(j), Some(a)) if j < a)
    }

    /// Whether any job of its current attempt was taken by a runner.
    pub fn any_job_started(&self) -> bool {
        self.jobs.iter().any(|j| j.started() && !self.carried(j))
    }

    /// Whether a cancel has reached its current attempt: the run, or a job of the attempt,
    /// concluded `cancelled`. Such a run can still be listed in flight while its `if: always()`
    /// jobs run, with a verdict nobody reads.
    pub fn cancelled_already(&self) -> bool {
        self.conclusion.as_deref() == Some("cancelled")
            || self
                .jobs
                .iter()
                .any(|j| j.conclusion.as_deref() == Some("cancelled") && !self.carried(j))
    }

    /// Whether it is on its way to a verdict: in flight, and no cancel has reached it.
    pub fn toward_verdict(&self) -> bool {
        self.in_flight() && !self.cancelled_already()
    }

    /// Whether it has completed with a verdict: `success`, `failure` or `timed_out`. A
    /// cancelled or skipped run decided nothing. The supervisor observes only the runs in
    /// flight, so it sees one only where it is handed it.
    pub fn verdict_written(&self) -> bool {
        self.status == "completed"
            && matches!(
                self.conclusion.as_deref(),
                Some("success" | "failure" | "timed_out")
            )
    }

    /// Since when its current attempt has waited for a runner, in seconds since the epoch: when
    /// its concurrency group admitted it, which is when the forge created the first job of the
    /// attempt (its `created_at`, or for a job that names none its `started_at`, which the forge
    /// fills with its creation while it is queued), and never before the attempt's
    /// `run_started_at` or the run's creation. Neither of those is the admission: a first attempt
    /// has `run_started_at` at its creation however long it then waited in its group, and
    /// 40811621641, created at 03:20:59Z, was admitted at 05:31:53Z. `Ok(None)` while no job of
    /// the attempt exists: the run is `pending` in its group, waiting behind a run of its own
    /// pull request rather than for a runner, and no queue bound is measured. Err names a time
    /// that cannot be read.
    pub fn queued_since(&self) -> Result<Option<u64>, String> {
        let created = seconds(&self.created_at)
            .ok_or_else(|| format!("its creation `{}` is not a time", self.created_at))?;
        let attempt = self.run_started_at.as_deref().and_then(seconds);
        let mut admitted: Option<u64> = None;
        for j in self.jobs.iter().filter(|j| !self.carried(j)) {
            let stamp = j.created_at.as_deref().or(j.started_at.as_deref());
            let Some(stamp) = stamp else {
                return Err(format!("job `{}` names no creation", j.name));
            };
            let t = seconds(stamp).ok_or_else(|| {
                format!(
                    "job `{}` was created at `{stamp}`, which is not a time",
                    j.name
                )
            })?;
            admitted = Some(admitted.map_or(t, |a| a.min(t)));
        }
        Ok(admitted.map(|a| a.max(created).max(attempt.unwrap_or(created))))
    }

    /// The job of it that has been in progress longest at `now`, with its minutes;
    /// `Ok(None)` when no job is in progress. Only a running job is held to a bound: a
    /// completed one, a carried one among them, has ended, and a queued one has not begun.
    /// Err names a running job whose start is not a time.
    pub fn longest_running_job(&self, now: u64) -> Result<Option<(&JobObservation, u64)>, String> {
        let mut longest: Option<(&JobObservation, u64)> = None;
        for j in self.jobs.iter().filter(|j| j.status == "in_progress") {
            let stamp = j.started_at.as_deref().unwrap_or("");
            let t = seconds(stamp).ok_or_else(|| {
                format!("job `{}` started at `{stamp}`, which is not a time", j.name)
            })?;
            let minutes = now.saturating_sub(t) / 60;
            if longest.is_none_or(|(_, m)| minutes > m) {
                longest = Some((j, minutes));
            }
        }
        Ok(longest)
    }
}

/// How a workflow's concurrency group is keyed, as far as superseding is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SupersededGroup {
    /// Nothing is ever cancelled for being superseded.
    #[default]
    Never,
    /// One group per pull request for its `pull_request` runs (validate's
    /// `validate-<number>`): a newer run of the pull request waits behind the older one.
    PerPullRequest,
}

/// What one workflow's runs are held to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowBound {
    /// Minutes a pull request's run may stay queued with no job of its attempt started.
    pub queued_minutes: u64,
    /// Minutes one job may stay in progress: the workflow's largest job timeout plus thirty.
    pub running_minutes: u64,
    /// How its concurrency group is keyed.
    #[serde(default)]
    pub superseded: SupersededGroup,
}

/// The model `.ai/repo/ci/supervise.yaml` declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SuperviseModel {
    /// The supervising workflow, whose runs are never cancelled.
    pub supervisor: String,
    /// The bound of each workflow, by its file name without `.yml`. A workflow named nowhere
    /// is judged by nothing.
    pub bounds: BTreeMap<String, WorkflowBound>,
}

fn whole(v: Option<&serde_json::Value>, what: &str) -> Result<u64, String> {
    v.and_then(serde_json::Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("`{what}` is not a whole number of minutes above zero"))
}

/// The model declared in `text`, the content of [`MODEL`].
pub fn model_of(text: &str) -> Result<SuperviseModel, String> {
    let map = crate::metadata::yaml::parse_mapping(text)?;
    if map.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("it does not declare `version: 1`".into());
    }
    let supervisor = map
        .get("supervisor")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "it names no `supervisor` workflow".to_string())?
        .to_string();
    let declared = map
        .get("bounds")
        .and_then(serde_json::Value::as_object)
        .filter(|m| !m.is_empty())
        .ok_or_else(|| "it declares no `bounds`".to_string())?;
    let mut bounds = BTreeMap::new();
    for (workflow, b) in declared {
        if *workflow == supervisor {
            return Err(format!(
                "`bounds.{workflow}` bounds the supervisor, whose runs are never cancelled"
            ));
        }
        let superseded = match b.get("superseded").and_then(serde_json::Value::as_str) {
            None => SupersededGroup::Never,
            Some("never") => SupersededGroup::Never,
            Some("per_pull_request") => SupersededGroup::PerPullRequest,
            Some(other) => {
                return Err(format!(
                    "`bounds.{workflow}.superseded` is `{other}`, neither `never` nor `per_pull_request`"
                ))
            }
        };
        if superseded == SupersededGroup::PerPullRequest
            && PROTECTED_WORKFLOWS.contains(&workflow.as_str())
        {
            return Err(format!(
                "`bounds.{workflow}.superseded`: a run of {workflow} is never cancelled for being superseded"
            ));
        }
        bounds.insert(
            workflow.clone(),
            WorkflowBound {
                queued_minutes: whole(
                    b.get("queued_minutes"),
                    &format!("bounds.{workflow}.queued_minutes"),
                )?,
                running_minutes: whole(
                    b.get("running_minutes"),
                    &format!("bounds.{workflow}.running_minutes"),
                )?,
                superseded,
            },
        );
    }
    Ok(SuperviseModel { supervisor, bounds })
}

/// The model declared in the repository at `root`.
pub fn read_model(root: &std::path::Path) -> Result<SuperviseModel, String> {
    std::fs::read_to_string(root.join(MODEL))
        .map_err(|e| format!("{MODEL} cannot be read: {e}"))
        .and_then(|t| model_of(&t).map_err(|e| format!("{MODEL}: {e}")))
}

/// Why a run is cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CancelReason {
    /// A pull request's first-attempt run that waits with no job created while an older run of
    /// the same pull request, branch and commit, created within [`TWIN_WINDOW_SECONDS`] of it,
    /// is in flight and is not itself to be ended, and the pull request is not labelled
    /// [`FULL_LABEL`].
    DuplicateRun {
        /// The pull request.
        pull_request: u64,
        /// The older run of the same commit.
        earlier: u64,
        /// The older run's status.
        earlier_state: String,
    },
    /// A pull request's run of a head that is no longer the pull request's, while a newer run
    /// of the same pull request at its current head waits behind it in the same concurrency
    /// group.
    SupersededBlocksSuccessor {
        /// The pull request.
        pull_request: u64,
        /// Its head now.
        current_head: String,
        /// The newer run that waits.
        successor: u64,
    },
    /// A pull request's run whose attempt queued past its workflow's bound with no job of it
    /// started.
    QueuedPastBound {
        /// Minutes since its group admitted its attempt ([`RunObservation::queued_since`]).
        minutes: u64,
        /// The declared bound.
        bound: u64,
    },
    /// A job of it in progress past its workflow's bound.
    RunningPastBound {
        /// The job.
        job: String,
        /// Minutes since that job started.
        minutes: u64,
        /// The declared bound.
        bound: u64,
    },
}

impl CancelReason {
    /// The wire word of the kind.
    pub fn word(&self) -> &'static str {
        match self {
            CancelReason::DuplicateRun { .. } => "duplicate_run",
            CancelReason::SupersededBlocksSuccessor { .. } => "superseded_blocks_successor",
            CancelReason::QueuedPastBound { .. } => "queued_past_bound",
            CancelReason::RunningPastBound { .. } => "running_past_bound",
        }
    }

    /// The reason, for a person.
    pub fn explain(&self) -> String {
        match self {
            CancelReason::DuplicateRun {
                pull_request,
                earlier,
                earlier_state,
            } => format!(
                "run {earlier} of #{pull_request}'s same commit is {earlier_state}; this one only \
                 waits behind it (label {FULL_LABEL} to ask for a second run)"
            ),
            CancelReason::SupersededBlocksSuccessor {
                pull_request,
                current_head,
                successor,
            } => format!(
                "#{pull_request} is at {} now, and run {successor} of it waits behind this one",
                &current_head[..current_head.len().min(10)]
            ),
            CancelReason::QueuedPastBound { minutes, bound } => {
                format!("queued {minutes} min with no job started; the bound is {bound} min")
            }
            CancelReason::RunningPastBound {
                job,
                minutes,
                bound,
            } => {
                format!("job `{job}` has run {minutes} min; the bound is {bound} min")
            }
        }
    }
}

/// Why a run that would be superseded is not cancelled for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Protection {
    /// A push to the default branch: the trunk's verdict.
    DefaultBranchPush,
    /// A push to anything else, a tag among them.
    OtherPush,
    /// The schedule.
    Schedule,
    /// A dispatch, which is how the release pipeline asks for its record's verdict.
    WorkflowDispatch,
    /// A release.
    Release,
    /// Another event that is not a pull request's.
    OtherEvent,
    /// A run of the pages or release workflows.
    ProtectedWorkflow,
}

/// Why a run was looked at and left alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HoldReason {
    /// Its head moved, and no newer run of its pull request waits: a started run reaches its
    /// verdict (the concurrency rationale).
    SupersededWithoutSuccessor {
        /// The pull request.
        pull_request: u64,
    },
    /// Its head moved and a newer run waits, but it is never cancelled for that.
    Protected {
        /// Why.
        protection: Protection,
    },
    /// A run of the supervising workflow.
    Supervisor,
    /// A same-commit twin of a pull request labelled [`FULL_LABEL`]: the second run was
    /// asked for.
    FullRunRequested {
        /// The pull request.
        pull_request: u64,
        /// The older run of the same commit.
        earlier: u64,
    },
    /// The twin of an older run of its commit that is itself to be ended, now or within
    /// [`TWIN_HORIZON_MINUTES`], or that was cancelled already and runs only its `if: always()`
    /// jobs: cancelling both would leave the commit with no run, so this one waits to take the
    /// older one's place.
    EarlierRunEnds {
        /// The pull request.
        pull_request: u64,
        /// The older run of the same commit.
        earlier: u64,
        /// Why the older run ends, as a [`CancelReason::word`], or `cancelled`.
        ends_for: String,
    },
    /// A re-run (`run_attempt` above one) of a commit with an older run in flight: a person
    /// asked for it again.
    RerunRequested {
        /// The pull request.
        pull_request: u64,
        /// The older run of the same commit.
        earlier: u64,
        /// Its attempt.
        attempt: u64,
    },
    /// An `integration/*` top queued past its bound: it waits for a runner and is never
    /// cancelled for that.
    IntegrationTopWaits {
        /// Minutes since its attempt began to wait.
        minutes: u64,
        /// The declared bound.
        bound: u64,
    },
    /// An `integration/*` top with a job in progress past its bound: its verdict is the one a
    /// landing needs, and it is never cancelled for that.
    IntegrationTopRuns {
        /// The job.
        job: String,
        /// Minutes since that job started.
        minutes: u64,
        /// The declared bound.
        bound: u64,
    },
    /// A run of the trunk, a tag, the schedule, a dispatch or a release queued past its bound:
    /// its concurrency group is its own, so ending it frees nothing, and its verdict is the
    /// only one of what it runs.
    ProtectedRunWaits {
        /// Why it is protected.
        protection: Protection,
        /// Minutes since its attempt began to wait.
        minutes: u64,
        /// The declared bound.
        bound: u64,
    },
    /// The last run of its pull request's current head on its way to a verdict, which a rule
    /// would end while it waits: as a duplicate, or queued past its bound. Ending it would
    /// leave the head with no run and nothing to ask for one (a force-cancel skips the
    /// `if: always()` `ci`, so the required check would stay expected), while a run that waits
    /// holds no runner and ending it frees nothing: it waits on.
    LastRunOfHead {
        /// The pull request.
        pull_request: u64,
        /// What it would have been ended for, as a [`CancelReason::word`].
        would_end_for: String,
    },
    /// A time it would be judged by could not be read.
    Unjudgeable {
        /// Which.
        detail: String,
    },
}

/// One run to cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    /// The run.
    pub run: u64,
    /// Its workflow.
    pub workflow: String,
    /// Its event.
    pub event: String,
    /// Its branch.
    pub head_branch: String,
    /// Its commit.
    pub head_sha: String,
    /// Its pull request, when one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// Its status when decided.
    pub status: String,
    /// Why.
    pub reason: CancelReason,
    /// Whether the cancel is paired with a re-run of the same run once it has ended: it is the
    /// last run of its pull request's current head on its way to a verdict, ended because a
    /// job of it runs past its bound, so its runner is freed and the head is tested again
    /// rather than left with no run (the release path's `resume` re-runs a run that yielded
    /// the same way). Only a `running_past_bound` cancel is ever paired.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rerun: bool,
}

/// One run left alone, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hold {
    /// The run.
    pub run: u64,
    /// Its workflow.
    pub workflow: String,
    /// Its event.
    pub event: String,
    /// Why.
    pub reason: HoldReason,
}

/// Everything decided over one observation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Supervision {
    /// The runs to cancel, in run order.
    pub decisions: Vec<Decision>,
    /// The runs looked at and left alone, in run order. A run within its bounds and at its
    /// pull request's head is neither.
    pub held: Vec<Hold>,
}

/// What one decision is taken over.
#[derive(Debug, Clone, Copy)]
pub struct Situation<'a> {
    /// Every in-flight run observed.
    pub runs: &'a [RunObservation],
    /// Each open pull request's head now.
    pub pr_heads: &'a BTreeMap<u64, String>,
    /// The open pull requests labelled [`FULL_LABEL`].
    pub full: &'a BTreeSet<u64>,
    /// The repository's default branch.
    pub default_branch: &'a str,
    /// The bounds.
    pub model: &'a SuperviseModel,
    /// Now, in seconds since the epoch.
    pub now: u64,
    /// The supervising run's own id, when known.
    pub self_run: Option<u64>,
}

/// What [`judge`] says of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Cancel it.
    Cancel(CancelReason),
    /// Leave it, for this reason.
    Hold(HoldReason),
    /// Nothing to say: within its bounds, at its head.
    Fine,
}

/// Why a superseded run is not cancelled for it; `None` when it may be.
pub fn protection(run: &RunObservation, default_branch: &str) -> Option<Protection> {
    if PROTECTED_WORKFLOWS.contains(&run.workflow.as_str()) {
        return Some(Protection::ProtectedWorkflow);
    }
    match run.event.as_str() {
        "pull_request" => None,
        "push" if run.head_branch == default_branch => Some(Protection::DefaultBranchPush),
        "push" => Some(Protection::OtherPush),
        "schedule" => Some(Protection::Schedule),
        "workflow_dispatch" => Some(Protection::WorkflowDispatch),
        "release" => Some(Protection::Release),
        _ => Some(Protection::OtherEvent),
    }
}

fn seconds(stamp: &str) -> Option<u64> {
    crate::peers::parse_rfc3339(stamp)
}

/// Whether `a` was created after `b`: by time, then by id (ids grow).
fn newer(a: &RunObservation, b: &RunObservation) -> bool {
    match (seconds(&a.created_at), seconds(&b.created_at)) {
        (Some(x), Some(y)) if x != y => x > y,
        _ => a.id > b.id,
    }
}

/// The newer run of pull request `n` at its current head `head` that waits in `run`'s
/// concurrency group. A newer run of another commit is no verdict that matters: a same-commit
/// twin of a superseded run would only test that head again from scratch.
fn successor<'a>(
    run: &RunObservation,
    n: u64,
    head: &str,
    s: &Situation<'a>,
) -> Option<&'a RunObservation> {
    s.runs
        .iter()
        .filter(|o| {
            o.id != run.id
                && o.workflow == run.workflow
                && o.event == "pull_request"
                && o.pull_request == Some(n)
                && o.head_sha == head
                && o.waiting()
                && newer(o, run)
        })
        .max_by_key(|o| o.id)
}

/// Whether `a` and `b` were created within [`TWIN_WINDOW_SECONDS`] of each other: one burst.
fn one_burst(a: &RunObservation, b: &RunObservation) -> bool {
    match (seconds(&a.created_at), seconds(&b.created_at)) {
        (Some(x), Some(y)) => x.abs_diff(y) <= TWIN_WINDOW_SECONDS,
        _ => false,
    }
}

/// Whether `o` is a `pull_request` run of `run`'s workflow, branch and commit, other than it,
/// in flight and of the same burst ([`one_burst`]). A run of another branch is in another
/// concurrency group and blocks nothing, whatever pull request the forge attributed it to.
fn same_burst(o: &RunObservation, run: &RunObservation) -> bool {
    o.id != run.id
        && o.workflow == run.workflow
        && o.event == "pull_request"
        && o.head_branch == run.head_branch
        && o.head_sha == run.head_sha
        && o.in_flight()
        && one_burst(o, run)
}

/// The older run of pull request `n` that `run` duplicates: in flight and of the same burst
/// ([`same_burst`]); the oldest when there are several. A judged run is never one: by the time a
/// run is judged its twin has been admitted, and an admitted twin is never cancelled.
fn earlier_twin<'a>(run: &RunObservation, n: u64, s: &Situation<'a>) -> Option<&'a RunObservation> {
    s.runs
        .iter()
        .filter(|o| o.pull_request == Some(n) && same_burst(o, run) && newer(run, o))
        .min_by_key(|o| o.id)
}

/// The word a twin's older run ends for when it has been cancelled already and is still in
/// flight: its `if: always()` jobs run on, so it is listed in flight with a verdict nobody reads.
const ENDS_CANCELLED: &str = "cancelled";

/// Why `earlier` ends without its verdict, as a [`CancelReason::word`] when it is to be ended,
/// now or within [`TWIN_HORIZON_MINUTES`], or [`ENDS_CANCELLED`] when a job of its attempt was
/// cancelled already; `None` when it reaches its verdict as far as this supervisor can tell. Its
/// own twins are older, so this ends.
fn ends_for(earlier: &RunObservation, s: &Situation<'_>) -> Option<String> {
    // cancelled, by an earlier pass or by hand: its `ci` still waits for a runner (one waited
    // eleven minutes on 2026-10-29), and the twin is the commit's only run from then on
    if earlier.cancelled_already() {
        return Some(ENDS_CANCELLED.into());
    }
    if let Verdict::Cancel(reason) = judge(earlier, s) {
        return Some(reason.word().into());
    }
    let later = Situation {
        now: s.now.saturating_add(TWIN_HORIZON_MINUTES * 60),
        ..*s
    };
    match judge(earlier, &later) {
        Verdict::Cancel(reason) => Some(reason.word().into()),
        _ => None,
    }
}

/// The verdict on one run.
pub fn judge(run: &RunObservation, s: &Situation<'_>) -> Verdict {
    if !run.in_flight() {
        return Verdict::Fine;
    }
    if run.workflow == s.model.supervisor || s.self_run == Some(run.id) {
        return Verdict::Hold(HoldReason::Supervisor);
    }
    let Some(bound) = s.model.bounds.get(&run.workflow) else {
        return Verdict::Fine;
    };
    // (d) a twin of a commit whose run is in flight: it would only wait in the group. Only
    // while it is `pending` there with no job: an admitted run (`queued` from then on, its jobs
    // created) runs its `if: always()` jobs on a cancel, and validate's `ci` would then write a
    // failure over its twin's pass
    if bound.superseded == SupersededGroup::PerPullRequest
        && run.status == "pending"
        && run.jobs.is_empty()
        && protection(run, s.default_branch).is_none()
    {
        if let Some(n) = run.pull_request {
            if let Some(earlier) = earlier_twin(run, n, s) {
                if run.run_attempt > 1 {
                    return Verdict::Hold(HoldReason::RerunRequested {
                        pull_request: n,
                        earlier: earlier.id,
                        attempt: run.run_attempt,
                    });
                }
                if s.full.contains(&n) {
                    return Verdict::Hold(HoldReason::FullRunRequested {
                        pull_request: n,
                        earlier: earlier.id,
                    });
                }
                return match ends_for(earlier, s) {
                    // it takes the older run's place, and its wait is the older run's time:
                    // ended for waiting, it would leave the commit with no run either
                    Some(ends_for) => Verdict::Hold(HoldReason::EarlierRunEnds {
                        pull_request: n,
                        earlier: earlier.id,
                        ends_for,
                    }),
                    None => Verdict::Cancel(CancelReason::DuplicateRun {
                        pull_request: n,
                        earlier: earlier.id,
                        earlier_state: earlier.status.clone(),
                    }),
                };
            }
        }
    }
    superseded_or_bounded(run, bound, s)
}

/// The verdict of rules (a), (b) and (c): superseded, then the two bounds.
fn superseded_or_bounded(
    run: &RunObservation,
    bound: &WorkflowBound,
    s: &Situation<'_>,
) -> Verdict {
    // (a) superseded, and something waits behind it
    if bound.superseded == SupersededGroup::PerPullRequest {
        if let Some(n) = run.pull_request {
            if let Some(head) = s.pr_heads.get(&n).filter(|h| **h != run.head_sha) {
                match successor(run, n, head, s) {
                    Some(next) => match protection(run, s.default_branch) {
                        Some(protection) => {
                            return Verdict::Hold(HoldReason::Protected { protection })
                        }
                        None => {
                            return Verdict::Cancel(CancelReason::SupersededBlocksSuccessor {
                                pull_request: n,
                                current_head: head.clone(),
                                successor: next.id,
                            })
                        }
                    },
                    // a run that holds nothing up reaches its verdict; only the bounds below
                    // can end it
                    None if protection(run, s.default_branch).is_none() => {
                        if let v @ Verdict::Cancel(_) = bounded(run, bound, s) {
                            return v;
                        }
                        return Verdict::Hold(HoldReason::SupersededWithoutSuccessor {
                            pull_request: n,
                        });
                    }
                    None => {}
                }
            }
        }
    }
    bounded(run, bound, s)
}

/// The verdict of the two bounds alone.
fn bounded(run: &RunObservation, bound: &WorkflowBound, s: &Situation<'_>) -> Verdict {
    let top = run.head_branch.starts_with(INTEGRATION_PREFIX);
    // (c) a job running past its bound: each running job on its own clock, never the run's,
    // whose stages wait for runners between them and whose re-run carries finished jobs
    match run.longest_running_job(s.now) {
        Err(detail) => return Verdict::Hold(HoldReason::Unjudgeable { detail }),
        Ok(Some((job, minutes))) if minutes > bound.running_minutes => {
            let job = job.name.clone();
            if top {
                return Verdict::Hold(HoldReason::IntegrationTopRuns {
                    job,
                    minutes,
                    bound: bound.running_minutes,
                });
            }
            return Verdict::Cancel(CancelReason::RunningPastBound {
                job,
                minutes,
                bound: bound.running_minutes,
            });
        }
        Ok(_) => {}
    }
    if run.any_job_started() {
        return Verdict::Fine;
    }
    // (b) its attempt queued past its bound, nothing of it started, from its admission: a run
    // with no job is pending in its group, and its wait there is its older run's time
    let since = match run.queued_since() {
        Err(detail) => return Verdict::Hold(HoldReason::Unjudgeable { detail }),
        Ok(None) => return Verdict::Fine,
        Ok(Some(since)) => since,
    };
    let minutes = s.now.saturating_sub(since) / 60;
    if minutes <= bound.queued_minutes {
        return Verdict::Fine;
    }
    if top {
        return Verdict::Hold(HoldReason::IntegrationTopWaits {
            minutes,
            bound: bound.queued_minutes,
        });
    }
    if let Some(protection) = protection(run, s.default_branch) {
        return Verdict::Hold(HoldReason::ProtectedRunWaits {
            protection,
            minutes,
            bound: bound.queued_minutes,
        });
    }
    Verdict::Cancel(CancelReason::QueuedPastBound {
        minutes,
        bound: bound.queued_minutes,
    })
}

/// Whether a rule could act on `run` only once its jobs are read, among `runs` at `now`. A
/// run's jobs cost a request each, out of the one `GITHUB_TOKEN` budget every workflow of the
/// repository spends, so they are read only where they can change the verdict:
///
/// - a bounded run older than the smaller of its two bounds: every job of it started after it
///   was created, and its attempt waits from its creation at the earliest, so a younger run
///   can be past neither;
/// - a pull request's run of a workflow with a per-pull-request group that has another run of
///   its branch and commit in flight from the same burst: the duplicate rule asks whether a job
///   of the newer one exists, and judges the older one ahead ([`TWIN_HORIZON_MINUTES`]), when
///   it could be past a bound however young it is now.
///
/// Every other run is judged without its jobs, and the verdict is the same. A creation that
/// is not a time reads them, and the run is then held as unjudgeable.
pub fn needs_jobs(
    run: &RunObservation,
    runs: &[RunObservation],
    model: &SuperviseModel,
    now: u64,
) -> bool {
    if !run.in_flight() || run.workflow == model.supervisor {
        return false;
    }
    let Some(bound) = model.bounds.get(&run.workflow) else {
        return false;
    };
    let Some(created) = seconds(&run.created_at) else {
        return true;
    };
    if now.saturating_sub(created) / 60 > bound.queued_minutes.min(bound.running_minutes) {
        return true;
    }
    bound.superseded == SupersededGroup::PerPullRequest
        && run.event == "pull_request"
        && runs.iter().any(|o| same_burst(o, run))
}

/// Whether ending `run` leaves its pull request's current head a run: always, unless `run` is a
/// pull request's run at that head on its way to a verdict
/// ([`RunObservation::toward_verdict`]); then another run of the same workflow, pull request
/// and head must be on its way to one, or have written one, and not be among `ended`, the runs
/// this pass ends before it. A run already cancelled decides nothing, so ending it takes
/// nothing from the head. The supervisor observes only the runs in flight, so a verdict the
/// head already has is seen only where it is handed in: the rule errs towards keeping a run.
pub fn head_keeps_a_run(run: &RunObservation, s: &Situation<'_>, ended: &BTreeSet<u64>) -> bool {
    let Some(n) = run.pull_request.filter(|_| run.event == "pull_request") else {
        return true;
    };
    if s.pr_heads.get(&n) != Some(&run.head_sha) || !run.toward_verdict() {
        return true;
    }
    s.runs.iter().any(|o| {
        o.id != run.id
            && !ended.contains(&o.id)
            && o.workflow == run.workflow
            && o.event == "pull_request"
            && o.pull_request == Some(n)
            && o.head_sha == run.head_sha
            && (o.toward_verdict() || o.verdict_written())
    })
}

/// [`judge`]'s verdict on `run`, held to the invariant of every pass: a cancel that would leave
/// its pull request's head with no run ([`head_keeps_a_run`], over the runs `ended` before it)
/// is a hold when the run waits ([`HoldReason::LastRunOfHead`]), and is paired with a re-run
/// when a job of it runs past its bound (the `bool`). See the module's "A head keeps its run".
fn settle(run: &RunObservation, s: &Situation<'_>, ended: &BTreeSet<u64>) -> (Verdict, bool) {
    match judge(run, s) {
        Verdict::Cancel(reason) if !head_keeps_a_run(run, s, ended) => match reason {
            // its runner is held by a job its own timeout did not end: freed, and asked again
            CancelReason::RunningPastBound { .. } => (Verdict::Cancel(reason), true),
            // it waits, holding no runner: ending it frees nothing and leaves the head bare
            other => (
                Verdict::Hold(HoldReason::LastRunOfHead {
                    pull_request: run.pull_request.unwrap_or_default(),
                    would_end_for: other.word().into(),
                }),
                false,
            ),
        },
        v => (v, false),
    }
}

/// Every decision over one observation, in run order. Each cancel is settled against the ones
/// before it ([`settle`]), so no pass ends the last run of an open pull request's head without
/// asking for it again.
pub fn decide(s: &Situation<'_>) -> Supervision {
    // in run order, each run once: the map is the order
    let runs: BTreeMap<u64, &RunObservation> = s.runs.iter().map(|r| (r.id, r)).collect();
    let mut out = Supervision::default();
    let mut ended = BTreeSet::new();
    for run in runs.into_values() {
        match settle(run, s, &ended) {
            (Verdict::Cancel(reason), rerun) => {
                ended.insert(run.id);
                out.decisions.push(Decision {
                    run: run.id,
                    workflow: run.workflow.clone(),
                    event: run.event.clone(),
                    head_branch: run.head_branch.clone(),
                    head_sha: run.head_sha.clone(),
                    pull_request: run.pull_request,
                    status: run.status.clone(),
                    reason,
                    rerun,
                })
            }
            (Verdict::Hold(reason), _) => out.held.push(Hold {
                run: run.id,
                workflow: run.workflow.clone(),
                event: run.event.clone(),
                reason,
            }),
            (Verdict::Fine, _) => {}
        }
    }
    out
}

/// Whether `decision` still holds over a fresh reading of its run (`fresh`, `None` when the
/// forge no longer lists it) and of its pull request's head, the other runs as this pass has
/// left them (a run it cancelled reads cancelled): the decision as it stands now, its reason of
/// the same kind and whether it is paired with a re-run settled again ([`settle`]), or `None`
/// and the act is skipped. A head whose other run ended meanwhile turns a plain cancel into a
/// paired one, or into no act.
pub fn still_holds(
    decision: &Decision,
    fresh: Option<&RunObservation>,
    s: &Situation<'_>,
) -> Option<Decision> {
    let mut fresh = fresh?.clone();
    // a run the forge names no pull request for keeps the one it was decided under
    if fresh.pull_request.is_none() && fresh.event == "pull_request" {
        fresh.pull_request = decision.pull_request;
    }
    let fresh = &fresh;
    let mut runs: Vec<RunObservation> = s
        .runs
        .iter()
        .filter(|r| r.id != fresh.id)
        .cloned()
        .collect();
    runs.push(fresh.clone());
    let again = Situation { runs: &runs, ..*s };
    match settle(fresh, &again, &BTreeSet::new()) {
        (Verdict::Cancel(reason), rerun)
            if std::mem::discriminant(&reason) == std::mem::discriminant(&decision.reason) =>
        {
            Some(Decision {
                reason,
                rerun,
                status: fresh.status.clone(),
                ..decision.clone()
            })
        }
        _ => None,
    }
}

/// Whether the supervisor itself is alive: its newest success, against [`STALE_MINUTES`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Liveness {
    /// When the newest successful supervision ended, RFC 3339; `None` when none was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_success_at: Option<String>,
    /// Its age in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u64>,
    /// [`STALE_MINUTES`].
    pub bound_minutes: u64,
    /// [`SUPERVISE_STALE`] or [`SUPERVISE_UNOBSERVED`]; `None` while it is alive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    /// What the finding means, for a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The lane whose supervision this is (`lundra`), when only that lane's job was counted;
    /// `None` when any lane's was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
}

/// The liveness of a supervisor whose newest success ended at `newest_success` (`Ok(None)`:
/// none found; `Err`: it could not be read), at `now`. Older than [`STALE_MINUTES`], none, or
/// unread is a finding: a supervisor that stopped is the failure this exists to name. Of one
/// `lane`'s supervision (`None`: any lane's), read over the runs of the last `window` minutes
/// (`None`: every run read). A window of at least [`STALE_MINUTES`] that holds no success is a
/// stale supervisor, as surely as no success at all.
pub fn liveness_of(
    newest_success: Result<Option<&str>, String>,
    now: u64,
    lane: Option<&str>,
    window: Option<u64>,
) -> Liveness {
    let by = lane.map(|l| format!(" by {l}")).unwrap_or_default();
    let mut l = Liveness {
        bound_minutes: STALE_MINUTES,
        lane: lane.map(str::to_string),
        ..Liveness::default()
    };
    match newest_success {
        Err(e) => {
            l.finding = Some(SUPERVISE_UNOBSERVED.into());
            l.detail = Some(format!(
                "the newest successful supervision{by} could not be read: {e}"
            ));
        }
        Ok(None) => {
            l.finding = Some(SUPERVISE_STALE.into());
            l.detail = Some(match window {
                Some(w) => format!("no supervision{by} succeeded in the last {w} min"),
                None => format!("no successful supervision{by} was found"),
            });
        }
        Ok(Some(at)) => {
            l.newest_success_at = Some(at.to_string());
            match seconds(at) {
                None => {
                    l.finding = Some(SUPERVISE_UNOBSERVED.into());
                    l.detail = Some(format!("its newest success `{at}` is not a time"));
                }
                Some(t) => {
                    let minutes = now.saturating_sub(t) / 60;
                    l.minutes = Some(minutes);
                    if minutes > STALE_MINUTES {
                        l.finding = Some(SUPERVISE_STALE.into());
                        l.detail = Some(format!(
                            "the newest successful supervision{by} ended {minutes} min ago; the \
                             bound is {STALE_MINUTES} min{}",
                            if lane.is_some() {
                                ""
                            } else {
                                ": nothing retires a stuck or duplicate run"
                            }
                        ));
                    }
                }
            }
        }
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_793_000_000; // 2026-10-26T...Z

    fn at(minutes_ago: u64) -> String {
        crate::peers::rfc3339(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(NOW - minutes_ago * 60),
        )
    }

    fn model() -> SuperviseModel {
        let mut bounds = BTreeMap::new();
        for (w, q, r, s) in [
            ("validate", 240, 255, SupersededGroup::PerPullRequest),
            ("pages", 240, 110, SupersededGroup::Never),
            ("release", 240, 75, SupersededGroup::Never),
        ] {
            bounds.insert(
                w.to_string(),
                WorkflowBound {
                    queued_minutes: q,
                    running_minutes: r,
                    superseded: s,
                },
            );
        }
        SuperviseModel {
            supervisor: "supervise".into(),
            bounds,
        }
    }

    fn run(id: u64, sha: &str, pr: Option<u64>, status: &str, minutes_ago: u64) -> RunObservation {
        RunObservation {
            id,
            workflow: "validate".into(),
            event: "pull_request".into(),
            head_branch: "fix/x".into(),
            head_sha: sha.into(),
            pull_request: pr,
            status: status.into(),
            conclusion: None,
            created_at: at(minutes_ago),
            run_started_at: Some(at(minutes_ago)),
            run_attempt: 1,
            jobs: if status == "in_progress" {
                vec![JobObservation {
                    name: "rust".into(),
                    status: "in_progress".into(),
                    started_at: Some(at(minutes_ago)),
                    created_at: None,
                    conclusion: None,
                }]
            } else if status == "queued" {
                // admitted by its group at its creation: its first job waits for a runner
                vec![JobObservation {
                    name: "plan".into(),
                    status: "queued".into(),
                    started_at: Some(at(minutes_ago)),
                    created_at: Some(at(minutes_ago)),
                    conclusion: None,
                }]
            } else {
                // pending in its group, or not yet read: no job
                Vec::new()
            },
        }
    }

    fn heads(pairs: &[(u64, &str)]) -> BTreeMap<u64, String> {
        pairs.iter().map(|(n, s)| (*n, s.to_string())).collect()
    }

    /// [`decide`], with the invariant of every pass asserted over what it decided: no open pull
    /// request's head that had a run is left without one (supervise_property.rs).
    fn decided(s: &Situation<'_>) -> Supervision {
        let out = decide(s);
        let left = crate::integration::supervise_property::heads_left_without_a_run(s, &out);
        assert!(
            left.is_empty(),
            "heads left without a run: {left:?}: {out:?}"
        );
        out
    }

    fn decide_over(runs: &[RunObservation], h: &BTreeMap<u64, String>) -> Supervision {
        let m = model();
        decided(&Situation {
            runs,
            pr_heads: h,
            full: &BTreeSet::new(),
            default_branch: "master",
            model: &m,
            now: NOW,
            self_run: None,
        })
    }

    /// The 10a/10b shape: the old head's run in progress, the new head's waits behind it.
    fn blocked_train() -> Vec<RunObservation> {
        vec![
            run(100, "old", Some(7), "in_progress", 90),
            run(101, "new", Some(7), "pending", 60),
        ]
    }

    #[test]
    fn a_superseded_run_that_blocks_its_successor_is_cancelled() {
        let s = decide_over(&blocked_train(), &heads(&[(7, "new")]));
        assert_eq!(s.decisions.len(), 1, "{s:?}");
        let d = &s.decisions[0];
        assert_eq!(d.run, 100);
        assert_eq!(
            d.reason,
            CancelReason::SupersededBlocksSuccessor {
                pull_request: 7,
                current_head: "new".into(),
                successor: 101
            }
        );
        assert_eq!(d.reason.word(), "superseded_blocks_successor");
        // the successor is at the head and within its bound: nothing to say of it
        assert!(s.held.is_empty(), "{:?}", s.held);
    }

    #[test]
    fn a_superseded_run_is_not_cancelled_for_a_twin_of_its_own_commit() {
        // train #7 opened with --label train: run 100 of S1 in progress, its twin 101 of S1
        // pending; the head moved to S2 and no run of S2 is in the group yet (a [skip ci] push,
        // a push by the workflow token, or a pass before the synchronize run was created)
        let mut runs = vec![
            run(100, "S1", Some(7), "in_progress", 90),
            run(101, "S1", Some(7), "pending", 90),
        ];
        let s = decide_over(&runs, &heads(&[(7, "S2")]));
        // the twin would only test S1 again from scratch: 100 reaches its verdict
        assert!(s.decisions.iter().all(|d| d.run != 100), "{s:?}");
        assert_eq!(
            s.held
                .iter()
                .find(|h| h.run == 100)
                .map(|h| h.reason.clone()),
            Some(HoldReason::SupersededWithoutSuccessor { pull_request: 7 }),
            "{s:?}"
        );
        // once a run of S2 waits, that is the verdict that matters
        runs.push(run(102, "S2", Some(7), "pending", 1));
        let s = decide_over(&runs, &heads(&[(7, "S2")]));
        assert_eq!(
            s.decisions
                .iter()
                .find(|d| d.run == 100)
                .map(|d| d.reason.clone()),
            Some(CancelReason::SupersededBlocksSuccessor {
                pull_request: 7,
                current_head: "S2".into(),
                successor: 102
            }),
            "{s:?}"
        );
    }

    #[test]
    fn a_superseded_run_with_no_waiting_successor_is_left_alone() {
        let runs = vec![run(100, "old", Some(7), "in_progress", 90)];
        let s = decide_over(&runs, &heads(&[(7, "new")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held,
            vec![Hold {
                run: 100,
                workflow: "validate".into(),
                event: "pull_request".into(),
                reason: HoldReason::SupersededWithoutSuccessor { pull_request: 7 }
            }]
        );
        // a successor that already runs is not waiting behind it either
        let mut runs = blocked_train();
        runs[1].status = "in_progress".into();
        let s = decide_over(&runs, &heads(&[(7, "new")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        // nor is a run of another pull request
        let mut runs = blocked_train();
        runs[1].pull_request = Some(8);
        assert!(decide_over(&runs, &heads(&[(7, "new"), (8, "new")]))
            .decisions
            .is_empty());
    }

    #[test]
    fn a_run_at_its_pull_requests_head_is_never_superseded() {
        let s = decide_over(&blocked_train(), &heads(&[(7, "old")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        // and a pull request that is not open (no head read) decides nothing
        assert!(decide_over(&blocked_train(), &heads(&[]))
            .decisions
            .is_empty());
    }

    #[test]
    fn a_workflow_without_a_per_pull_request_group_is_never_superseded() {
        let mut runs = blocked_train();
        for r in &mut runs {
            r.workflow = "pr-meta".into();
        }
        let mut m = model();
        m.bounds.insert(
            "pr-meta".into(),
            WorkflowBound {
                queued_minutes: 240,
                running_minutes: 255,
                superseded: SupersededGroup::Never,
            },
        );
        let h = heads(&[(7, "new")]);
        let s = decided(&Situation {
            runs: &runs,
            pr_heads: &h,
            full: &BTreeSet::new(),
            default_branch: "master",
            model: &m,
            now: NOW,
            self_run: None,
        });
        assert!(s.decisions.is_empty(), "{s:?}");
    }

    #[test]
    fn every_protected_event_and_workflow_is_held_rather_than_superseded() {
        let cases: &[(&str, &str, &str, Protection)] = &[
            ("validate", "push", "master", Protection::DefaultBranchPush),
            ("validate", "push", "v1.0.0", Protection::OtherPush),
            ("validate", "schedule", "master", Protection::Schedule),
            (
                "validate",
                "workflow_dispatch",
                "release/v1",
                Protection::WorkflowDispatch,
            ),
            ("validate", "release", "v1.0.0", Protection::Release),
            ("validate", "merge_group", "x", Protection::OtherEvent),
        ];
        for (workflow, event, branch, protection) in cases {
            let mut runs = blocked_train();
            runs[0].workflow = workflow.to_string();
            runs[0].event = event.to_string();
            runs[0].head_branch = branch.to_string();
            let s = decide_over(&runs, &heads(&[(7, "new")]));
            assert!(s.decisions.is_empty(), "{event} on {branch}: {s:?}");
            assert_eq!(
                s.held[0].reason,
                HoldReason::Protected {
                    protection: protection.clone()
                },
                "{event} on {branch}"
            );
        }
        // the pages and release workflows, even on a pull request
        for workflow in PROTECTED_WORKFLOWS {
            let mut runs = blocked_train();
            for r in &mut runs {
                r.workflow = workflow.to_string();
            }
            let mut m = model();
            // a model that tried to declare it is refused (model_of), so force it here
            m.bounds.get_mut(*workflow).unwrap().superseded = SupersededGroup::PerPullRequest;
            let h = heads(&[(7, "new")]);
            let s = decided(&Situation {
                runs: &runs,
                pr_heads: &h,
                full: &BTreeSet::new(),
                default_branch: "master",
                model: &m,
                now: NOW,
                self_run: None,
            });
            assert!(s.decisions.is_empty(), "{workflow}: {s:?}");
            assert_eq!(
                s.held[0].reason,
                HoldReason::Protected {
                    protection: Protection::ProtectedWorkflow
                }
            );
        }
    }

    #[test]
    fn a_run_queued_past_its_bound_with_no_job_started_is_cancelled() {
        // its pull request's head moved, and nothing waits behind it: the bound ends it (at the
        // head and alone, it would be held: a_head_keeps_its_last_run)
        let moved = heads(&[(7, "b")]);
        let runs = vec![run(100, "a", Some(7), "queued", 241)];
        let s = decide_over(&runs, &moved);
        assert_eq!(
            s.decisions[0].reason,
            CancelReason::QueuedPastBound {
                minutes: 241,
                bound: 240
            }
        );
        assert!(!s.decisions[0].rerun);
        // at the bound it is not past it
        let runs = vec![run(100, "a", Some(7), "queued", 240)];
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        // a job that started means it is not stuck in the queue: it is running, from then
        let mut runs = vec![run(100, "a", Some(7), "queued", 500)];
        runs[0].jobs.push(JobObservation {
            name: "plan".into(),
            status: "completed".into(),
            started_at: Some(at(10)),
            created_at: None,
            conclusion: Some("success".into()),
        });
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        // a queued or skipped job is not started, whatever its started_at says
        let mut runs = vec![run(100, "a", Some(7), "queued", 241)];
        runs[0].jobs = vec![
            JobObservation {
                name: "rust".into(),
                status: "queued".into(),
                started_at: Some(at(241)),
                created_at: None,
                conclusion: None,
            },
            JobObservation {
                name: "macos".into(),
                status: "completed".into(),
                started_at: Some(at(241)),
                created_at: None,
                conclusion: Some("skipped".into()),
            },
        ];
        assert_eq!(
            decide_over(&runs, &moved).decisions[0].reason.word(),
            "queued_past_bound"
        );
        // an integration/* top waits for a runner and is never ended for it
        let mut runs = vec![run(100, "a", Some(7), "queued", 600)];
        runs[0].head_branch = "integration/train-10b-20261028".into();
        let s = decide_over(&runs, &heads(&[(7, "a")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held[0].reason,
            HoldReason::IntegrationTopWaits {
                minutes: 600,
                bound: 240
            }
        );
        // a run of the trunk waits in a group of its own and holds no runner: held
        let mut runs = vec![run(100, "a", None, "queued", 300)];
        runs[0].event = "push".into();
        runs[0].head_branch = "master".into();
        let s = decide_over(&runs, &heads(&[]));
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held[0].reason,
            HoldReason::ProtectedRunWaits {
                protection: Protection::DefaultBranchPush,
                minutes: 300,
                bound: 240
            }
        );
        // a re-run is held to the bound from its attempt's start, not its creation
        let mut runs = vec![run(100, "a", Some(7), "queued", 360)];
        runs[0].run_attempt = 2;
        runs[0].run_started_at = Some(at(2));
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        runs[0].run_started_at = Some(at(241));
        assert_eq!(
            decide_over(&runs, &moved).decisions[0].reason,
            CancelReason::QueuedPastBound {
                minutes: 241,
                bound: 240
            }
        );
        // a job an earlier attempt left behind is not a start of this one
        runs[0].jobs.insert(
            0,
            JobObservation {
                name: "plan".into(),
                status: "completed".into(),
                started_at: Some(at(300)),
                created_at: None,
                conclusion: Some("success".into()),
            },
        );
        assert!(runs[0].carried(&runs[0].jobs[0]));
        assert!(!runs[0].any_job_started());
        assert_eq!(
            decide_over(&runs, &moved).decisions[0].reason.word(),
            "queued_past_bound"
        );
    }

    #[test]
    fn a_run_in_progress_past_its_bound_is_cancelled() {
        let runs = vec![run(100, "a", Some(7), "in_progress", 256)];
        let s = decide_over(&runs, &heads(&[(7, "a")]));
        assert_eq!(
            s.decisions[0].reason,
            CancelReason::RunningPastBound {
                job: "rust".into(),
                minutes: 256,
                bound: 255
            }
        );
        // the last run of its head: its runner is freed, and it is run again once ended
        assert!(s.decisions[0].rerun, "{s:?}");
        let runs = vec![run(100, "a", Some(7), "in_progress", 255)];
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        // measured from the running job's start, not the run's creation nor its first job: a
        // run that waited in its group, or between its stages, ran no job in the meantime
        let mut runs = vec![run(100, "a", Some(7), "in_progress", 600)];
        runs[0].jobs[0].started_at = Some(at(10));
        runs[0].jobs.push(JobObservation {
            name: "plan".into(),
            status: "completed".into(),
            started_at: Some(at(590)),
            created_at: None,
            conclusion: Some("success".into()),
        });
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        // the longest running job is the one named
        runs[0].jobs.push(JobObservation {
            name: "suite".into(),
            status: "in_progress".into(),
            started_at: Some(at(300)),
            created_at: None,
            conclusion: None,
        });
        assert_eq!(
            decide_over(&runs, &heads(&[(7, "a")])).decisions[0].reason,
            CancelReason::RunningPastBound {
                job: "suite".into(),
                minutes: 300,
                bound: 255
            }
        );
        // a run with no job in progress is never running past its bound, however old
        let mut runs = vec![run(100, "a", Some(7), "in_progress", 600)];
        runs[0].jobs[0].status = "completed".into();
        runs[0].jobs[0].conclusion = Some("success".into());
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        let mut runs = vec![run(100, "a", Some(7), "in_progress", 239)];
        runs[0].jobs.clear();
        assert!(decide_over(&runs, &heads(&[(7, "a")])).decisions.is_empty());
        // an integration/* top's job past its bound is held, never ended
        let mut runs = vec![run(100, "a", Some(7), "in_progress", 300)];
        runs[0].head_branch = "integration/train-10b-20261028".into();
        let s = decide_over(&runs, &heads(&[(7, "a")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held[0].reason,
            HoldReason::IntegrationTopRuns {
                job: "rust".into(),
                minutes: 300,
                bound: 255
            }
        );
        // a running job whose start is not a time holds rather than cancels
        let mut runs = vec![run(100, "a", Some(7), "in_progress", 600)];
        runs[0].jobs[0].started_at = None;
        assert!(matches!(
            decide_over(&runs, &heads(&[(7, "a")])).held[0].reason,
            HoldReason::Unjudgeable { .. }
        ));
        // a superseded run with nothing waiting still ends at its bound
        let runs = vec![run(100, "old", Some(7), "in_progress", 300)];
        assert_eq!(
            decide_over(&runs, &heads(&[(7, "new")])).decisions[0]
                .reason
                .word(),
            "running_past_bound"
        );
    }

    #[test]
    fn a_head_keeps_its_last_run() {
        let at_head = heads(&[(7, "a")]);
        // queued past its bound at its head, with nothing else there: held, since a run that
        // waits holds no runner and ending it would leave the head with no run
        let alone = vec![run(100, "a", Some(7), "queued", 241)];
        let s = decide_over(&alone, &at_head);
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held,
            vec![Hold {
                run: 100,
                workflow: "validate".into(),
                event: "pull_request".into(),
                reason: HoldReason::LastRunOfHead {
                    pull_request: 7,
                    would_end_for: "queued_past_bound".into()
                }
            }]
        );
        // a newer run of the head waiting behind it takes its place: ended as before
        let mut behind = alone.clone();
        behind.push(run(101, "a", Some(7), "pending", 30));
        let s = decide_over(&behind, &at_head);
        assert_eq!(s.decisions.len(), 1, "{s:?}");
        assert_eq!(s.decisions[0].run, 100);
        assert!(!s.decisions[0].rerun);
        // and so a verdict the head already has, where it is handed in
        let mut judged = alone.clone();
        let mut passed = run(90, "a", Some(7), "completed", 300);
        passed.conclusion = Some("success".into());
        judged.push(passed.clone());
        assert_eq!(decide_over(&judged, &at_head).decisions.len(), 1);
        // a cancelled run is no verdict, nor is one a cancel reached that still runs its ci
        let mut gone = passed.clone();
        gone.conclusion = Some("cancelled".into());
        let mut ending = run(101, "a", Some(7), "in_progress", 30);
        ending.jobs[0].status = "completed".into();
        ending.jobs[0].conclusion = Some("cancelled".into());
        assert!(ending.in_flight() && !ending.toward_verdict());
        let s = decide_over(&[alone[0].clone(), gone, ending], &at_head);
        assert!(s.decisions.is_empty(), "{s:?}");
        // a run of another pull request, branch or workflow at the same commit is not this
        // head's run
        let mut other = run(101, "a", Some(8), "pending", 30);
        other.head_branch = "fix/y".into();
        let s = decide_over(&[alone[0].clone(), other], &heads(&[(7, "a"), (8, "a")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        // a job past its bound holds a runner: the last run is ended and run again once ended,
        // and with another run of the head left it is ended alone
        let hung = vec![run(100, "a", Some(7), "in_progress", 300)];
        let s = decide_over(&hung, &at_head);
        assert_eq!(s.decisions[0].reason.word(), "running_past_bound");
        assert!(s.decisions[0].rerun);
        let mut hung_behind = hung.clone();
        hung_behind.push(run(101, "a", Some(7), "pending", 30));
        let s = decide_over(&hung_behind, &at_head);
        assert_eq!(s.decisions.len(), 1, "{s:?}");
        assert!(!s.decisions[0].rerun, "{s:?}");
        // two runs of one head both past a bound in one pass: the first ends alone, and the
        // second, then the last, is held or paired
        let mut both = vec![
            run(100, "a", Some(7), "queued", 300),
            run(101, "a", Some(7), "queued", 241),
        ];
        both[1].created_at = at(250);
        let s = decide_over(&both, &at_head);
        assert_eq!(
            s.decisions.iter().map(|d| d.run).collect::<Vec<_>>(),
            vec![100],
            "{s:?}"
        );
        assert!(matches!(s.held[0].reason, HoldReason::LastRunOfHead { .. }));
        // a superseded run is never its head's: ended by its bound with no re-run
        let s = decide_over(&hung, &heads(&[(7, "b")]));
        assert_eq!(s.decisions[0].reason.word(), "running_past_bound");
        assert!(!s.decisions[0].rerun);
        // nor is a run of a pull request that is not open
        let s = decide_over(&alone, &heads(&[]));
        assert_eq!(s.decisions[0].reason.word(), "queued_past_bound");
    }

    #[test]
    fn jobs_are_read_only_where_a_rule_could_act() {
        let m = model();
        let young = run(100, "a", Some(7), "in_progress", 60);
        assert!(
            !needs_jobs(&young, &[], &m, NOW),
            "younger than every bound"
        );
        // validate's smaller bound is 240 minutes
        assert!(!needs_jobs(
            &run(100, "a", Some(7), "queued", 240),
            &[],
            &m,
            NOW
        ));
        assert!(needs_jobs(
            &run(100, "a", Some(7), "queued", 241),
            &[],
            &m,
            NOW
        ));
        // release's running bound, 75, is its smaller one
        let mut tag = run(100, "a", None, "in_progress", 76);
        tag.workflow = "release".into();
        assert!(needs_jobs(&tag, &[], &m, NOW));
        // two runs of one branch and commit from one burst: does a job of the newer exist, and
        // will the older one be ended before the next pass?
        let first = run(100, "a", Some(7), "in_progress", 30);
        let twin = run(101, "a", Some(7), "pending", 29);
        assert!(needs_jobs(&twin, &[first.clone(), twin.clone()], &m, NOW));
        assert!(needs_jobs(&first, &[first.clone(), twin.clone()], &m, NOW));
        // a second run asked for later is no twin, and costs no read
        let later = run(101, "a", Some(7), "pending", 25);
        assert!(!needs_jobs(
            &later,
            &[first.clone(), later.clone()],
            &m,
            NOW
        ));
        assert!(!needs_jobs(
            &first,
            &[first.clone(), later.clone()],
            &m,
            NOW
        ));
        let mut elsewhere = twin.clone();
        elsewhere.head_branch = "integration/10a".into();
        assert!(!needs_jobs(
            &elsewhere,
            &[first.clone(), elsewhere.clone()],
            &m,
            NOW
        ));
        // never the supervisor's, an unbounded workflow's or a completed run's
        let mut own = run(100, "a", None, "queued", 10_000);
        own.workflow = "supervise".into();
        assert!(!needs_jobs(&own, &[], &m, NOW));
        own.workflow = "pages-build-deployment".into();
        assert!(!needs_jobs(&own, &[], &m, NOW));
        assert!(!needs_jobs(
            &run(100, "a", Some(7), "completed", 10_000),
            &[],
            &m,
            NOW
        ));
        // a creation that is not a time is read, and held as unjudgeable
        let mut odd = run(100, "a", Some(7), "queued", 0);
        odd.created_at = "yesterday".into();
        assert!(needs_jobs(&odd, &[], &m, NOW));
        // and a run judged without its jobs is judged alike: young, it has nothing to say
        let mut bare = young.clone();
        bare.jobs.clear();
        let h = heads(&[(7, "a")]);
        assert_eq!(
            decide_over(std::slice::from_ref(&young), &h),
            decide_over(&[bare], &h)
        );
    }

    #[test]
    fn the_supervisor_is_never_cancelled_and_an_unbounded_workflow_is_not_judged() {
        let mut runs = vec![run(100, "a", None, "in_progress", 10_000)];
        runs[0].workflow = "supervise".into();
        let s = decide_over(&runs, &heads(&[]));
        assert!(s.decisions.is_empty());
        assert_eq!(s.held[0].reason, HoldReason::Supervisor);
        // its own id, whatever the workflow is called
        let runs = vec![run(100, "a", None, "in_progress", 10_000)];
        let m = model();
        let h = heads(&[]);
        let s = decided(&Situation {
            runs: &runs,
            pr_heads: &h,
            full: &BTreeSet::new(),
            default_branch: "master",
            model: &m,
            now: NOW,
            self_run: Some(100),
        });
        assert!(s.decisions.is_empty());
        let mut runs = vec![run(100, "a", None, "in_progress", 10_000)];
        runs[0].workflow = "pages-build-deployment".into();
        let s = decide_over(&runs, &heads(&[]));
        assert!(s.decisions.is_empty() && s.held.is_empty());
    }

    #[test]
    fn a_time_that_cannot_be_read_holds_rather_than_cancels() {
        let mut runs = vec![run(100, "a", None, "queued", 0)];
        runs[0].created_at = "yesterday".into();
        let s = decide_over(&runs, &heads(&[]));
        assert!(s.decisions.is_empty());
        assert!(matches!(s.held[0].reason, HoldReason::Unjudgeable { .. }));
        // and so is an admission that cannot be read: a job created at no time, or never
        let mut runs = vec![run(100, "a", Some(7), "queued", 300)];
        runs[0].jobs[0].created_at = Some("soon".into());
        let s = decide_over(&runs, &heads(&[(7, "a")]));
        assert!(s.decisions.is_empty(), "{s:?}");
        assert_eq!(
            s.held[0].reason,
            HoldReason::Unjudgeable {
                detail: "job `plan` was created at `soon`, which is not a time".into()
            }
        );
        runs[0].jobs[0].created_at = None;
        runs[0].jobs[0].started_at = None;
        assert_eq!(
            runs[0].queued_since(),
            Err("job `plan` names no creation".into())
        );
        // a job's creation stands in for nothing when it has one: it is the admission
        runs[0].jobs[0].created_at = Some(at(30));
        runs[0].jobs[0].started_at = Some(at(300));
        assert_eq!(runs[0].queued_since(), Ok(Some(NOW - 30 * 60)));
    }

    #[test]
    fn a_decision_still_holds_only_when_the_fresh_reading_says_the_same() {
        let runs = blocked_train();
        let m = model();
        let h = heads(&[(7, "new")]);
        let s = Situation {
            runs: &runs,
            pr_heads: &h,
            full: &BTreeSet::new(),
            default_branch: "master",
            model: &m,
            now: NOW,
            self_run: None,
        };
        let d = decided(&s).decisions.remove(0);
        assert!(still_holds(&d, Some(&runs[0]), &s).is_some());
        // it completed meanwhile
        let mut done = runs[0].clone();
        done.status = "completed".into();
        assert!(still_holds(&d, Some(&done), &s).is_none());
        // the forge no longer lists it
        assert!(still_holds(&d, None, &s).is_none());
        // the head changed again, back to this run's
        let back = heads(&[(7, "old")]);
        let s2 = Situation {
            pr_heads: &back,
            ..s
        };
        assert!(still_holds(&d, Some(&runs[0]), &s2).is_none());
    }

    #[test]
    fn a_twin_is_cancelled_only_while_it_has_no_job_and_its_older_run_will_reach_a_verdict() {
        let m = model();
        let h = heads(&[(7, "a")]);
        let over = |runs: &[RunObservation], now: u64| {
            decided(&Situation {
                runs,
                pr_heads: &h,
                full: &BTreeSet::new(),
                default_branch: "master",
                model: &m,
                now,
                self_run: None,
            })
        };
        // the burst: the older run in progress, its twin a minute younger waiting with no job
        let first = run(100, "a", Some(7), "in_progress", 30);
        let twin = run(101, "a", Some(7), "pending", 29);
        let s = over(&[first.clone(), twin.clone()], NOW);
        assert_eq!(s.decisions.len(), 1, "{s:?}");
        assert_eq!(s.decisions[0].run, 101);
        assert_eq!(s.decisions[0].reason.word(), "duplicate_run");
        // admitted, its jobs exist: a cancel would run its always() jobs, never
        let mut admitted = twin.clone();
        admitted.status = "queued".into();
        admitted.jobs = vec![JobObservation {
            name: "plan".into(),
            status: "queued".into(),
            started_at: Some(at(0)),
            created_at: None,
            conclusion: None,
        }];
        let s = over(&[first.clone(), admitted], NOW);
        assert!(s.decisions.is_empty(), "{s:?}");
        // and a passed older run is never the older half: its twin has been admitted
        let mut passed = first.clone();
        passed.status = "completed".into();
        passed.conclusion = Some("success".into());
        assert!(over(&[passed, twin.clone()], NOW).decisions.is_empty());
        // the older run's job is past its bound: it is ended, and its twin takes its place
        let late = NOW + 250 * 60; // rust started 30 minutes before NOW: 280 > 255
        let s = over(&[first.clone(), twin.clone()], late);
        assert_eq!(
            s.decisions.iter().map(|d| d.run).collect::<Vec<_>>(),
            vec![100],
            "{s:?}"
        );
        assert_eq!(s.decisions[0].reason.word(), "running_past_bound");
        assert_eq!(
            s.held[0].reason,
            HoldReason::EarlierRunEnds {
                pull_request: 7,
                earlier: 100,
                ends_for: "running_past_bound".into()
            }
        );
        // and so when the bound falls before the next pass: 255 - 30 + 1 minutes after its
        // job started, the older run is not past it, and would be within the horizon
        let soon = NOW + (255 - 30 - TWIN_HORIZON_MINUTES + 1) * 60;
        let s = over(&[first.clone(), twin.clone()], soon);
        assert!(s.decisions.is_empty(), "{s:?}");
        assert!(matches!(
            s.held[0].reason,
            HoldReason::EarlierRunEnds { earlier: 100, .. }
        ));
        // a cancel of the older run taken in this pass is seen by the twin's own act
        let d = over(&[first.clone(), twin.clone()], NOW)
            .decisions
            .remove(0);
        let mut ended = first.clone();
        ended.status = "completed".into();
        ended.conclusion = Some("cancelled".into());
        let after = [ended, twin.clone()];
        let s = Situation {
            runs: &after,
            pr_heads: &h,
            full: &BTreeSet::new(),
            default_branch: "master",
            model: &m,
            now: NOW,
            self_run: None,
        };
        assert!(still_holds(&d, Some(&twin), &s).is_none());
    }

    #[test]
    fn the_model_is_read_and_refuses_what_it_must_not_declare() {
        let ok = "version: 1\nsupervisor: supervise\nbounds:\n  validate:\n    queued_minutes: 240\n    running_minutes: 255\n    superseded: per_pull_request\n  pages:\n    queued_minutes: 240\n    running_minutes: 110\n";
        let m = model_of(ok).unwrap();
        assert_eq!(m.supervisor, "supervise");
        assert_eq!(
            m.bounds["validate"].superseded,
            SupersededGroup::PerPullRequest
        );
        assert_eq!(m.bounds["pages"].superseded, SupersededGroup::Never);
        assert_eq!(m.bounds["pages"].running_minutes, 110);
        for (bad, says) in [
            (ok.replace("version: 1", "version: 2"), "version"),
            (ok.replace("supervisor: supervise\n", ""), "supervisor"),
            (
                ok.replace("running_minutes: 110", "running_minutes: 0"),
                "pages.running_minutes",
            ),
            (ok.replace("per_pull_request", "per_commit"), "per_commit"),
            (
                ok.replace(
                    "running_minutes: 110\n",
                    "running_minutes: 110\n    superseded: per_pull_request\n",
                ),
                "never cancelled for being superseded",
            ),
            (
                ok.replace("  pages:", "  supervise:"),
                "bounds the supervisor",
            ),
        ] {
            let e = model_of(&bad).unwrap_err();
            assert!(e.contains(says), "{says}: {e}");
        }
    }

    /// Every workflow's job timeouts, as its file declares them: the `timeout-minutes` of each
    /// job, at the job's own indentation.
    fn job_timeouts(text: &str) -> Vec<u64> {
        let mut in_jobs = false;
        let mut out = Vec::new();
        for line in text.lines() {
            if line.starts_with("jobs:") {
                in_jobs = true;
                continue;
            }
            if in_jobs
                && !line.starts_with(' ')
                && !line.trim().is_empty()
                && !line.starts_with('#')
            {
                in_jobs = false;
            }
            if in_jobs {
                if let Some(n) = line.strip_prefix("    timeout-minutes:") {
                    out.push(n.trim().parse().unwrap());
                }
            }
        }
        out
    }

    #[test]
    fn the_declared_bounds_hold_to_every_workflows_real_timeouts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let m = read_model(&root).unwrap();
        let dir = root.join(".github/workflows");
        let mut seen = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("yml") {
                continue;
            }
            let name = path.file_stem().unwrap().to_str().unwrap().to_string();
            seen.push(name.clone());
            if name == m.supervisor {
                assert!(!m.bounds.contains_key(&name));
                continue;
            }
            let b = m
                .bounds
                .get(&name)
                .unwrap_or_else(|| panic!("{name}.yml has no bound in {MODEL}"));
            let largest = job_timeouts(&std::fs::read_to_string(&path).unwrap())
                .into_iter()
                .max()
                .unwrap_or_else(|| panic!("{name}.yml has a job without a timeout"));
            assert_eq!(
                b.running_minutes,
                largest + 30,
                "{name}: running_minutes is its largest job timeout ({largest}) plus 30"
            );
            assert!(b.queued_minutes >= 240, "{name}: queued_minutes below 240");
        }
        assert!(seen.contains(&m.supervisor), "no {}.yml", m.supervisor);
        for w in m.bounds.keys() {
            assert!(
                seen.contains(w),
                "{MODEL} bounds {w}, which is not a workflow"
            );
        }
        // superseding is validate's, whose group is one per pull request
        assert_eq!(
            m.bounds["validate"].superseded,
            SupersededGroup::PerPullRequest
        );
        let validate = std::fs::read_to_string(dir.join("validate.yml")).unwrap();
        assert!(validate.contains(
            "group: validate-${{ github.event_name == 'pull_request' && github.event.pull_request.number || github.run_id }}"
        ));
    }
}
