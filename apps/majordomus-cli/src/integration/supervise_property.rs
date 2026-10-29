//! The invariant of every supervision, as a property (I3903; the review of #2645 at
//! eed0b4f8ff4): once a pass has acted, an open pull request's current head that had a run on
//! its way to a verdict, or a verdict, still has one.
//!
//! [`heads_left_without_a_run`] states it over one decision, without the rules
//! [`super::supervise::decide`] reaches it by, and the replays of `supervise_replay.rs` and the
//! unit tests of `supervise.rs` assert it over every decision they take, at every tick they
//! replay. Here it is held over generated worlds: a few pull requests whose validate runs share
//! one concurrency group each, as GitHub's does (one run admitted, one pending, a newer pending
//! run replacing the one before it), with heads that move, `opened` and `labeled` twins, labels
//! added later, re-runs a person asks for, `ci:full`, runner queues that pass the queue bound,
//! jobs that run past the running bound or hang, and a supervisor that is sometimes down for
//! hours. The supervisor in them is the command's own [`super::runs::supervise`] with `--apply`,
//! over a forge that is the world, so its observation, its job reads, its re-checks, its cancels
//! and its re-runs are the ones the workflow runs. After every pass the invariant is asserted
//! over the world's whole truth, completed runs included, and [`super::supervise::decide`] over
//! the same moment is asserted to keep it too.
//!
//! The forge of the world also half-fails, as GitHub's does: a force-cancel that lands while
//! `gh` reports an error, at once or a minute later (the run still reads in flight when it is
//! read again), and a re-run that is refused. People act on it too: a person cancels a head's
//! run by hand now and then, and in one world of four a person cancels every run of one pull
//! request a few minutes after its group admits it, so that its head reaches GitHub's attempt
//! bound and stays there. The supervisor is several runners, each with a trail of its own (a
//! pass lands on any of them, as lundra's three runners and the hosted lane's fresh checkout
//! do), and in some minutes a second lane applies right after the first, on the same heads.
//!
//! Then a head can be left with no run, and the properties are these. A head that is not
//! exhausted gets its run back within [`super::runs::MAX_ATTEMPTS`] passes, from whichever runner
//! takes them: each world's forge refuses one run's re-run at most one time fewer than that in a
//! row, so a supervisor that judges the forge alone, and asks once a pass, always recovers; the
//! pass whose own act bared the head says so with exit 12, and a later pass that still finds it
//! bare (its recovery re-run refused) with a warning naming it. A head whose newest run was
//! cancelled at GitHub's attempt bound is held as `head_exhausted`, with a warning, on every pass
//! while it is within the window, and never makes a pass exit 12: a pass is red only when the
//! forge refused one of its own acts during it. And no re-run is ever asked over a verdict of its head,
//! nor of a run at the attempt bound: the forge of the world counts every one that is.
//!
//! The generator is a xorshift seeded per world, so a failure names the seed that reproduces it.

use std::collections::{BTreeMap, BTreeSet};

use super::forge::ForgeError;
use super::runs::{
    self, ActOutcome, RecoveryOutcome, Rerun, RunForge, RunsObservation, Successes, MAX_ATTEMPTS,
    RECOVERY_WINDOW_MINUTES,
};
use super::supervise::{
    self, HoldReason, JobObservation, RunObservation, Situation, SupersededGroup, SuperviseModel,
    Supervision, WorkflowBound, IN_FLIGHT,
};

/// Whether `r` decides its head, or will: completed with a verdict, or in flight with no job of
/// its attempt cancelled. Stated here on its own, not through the rules it checks.
fn covers(r: &RunObservation) -> bool {
    if r.status == "completed" {
        return matches!(
            r.conclusion.as_deref(),
            Some("success" | "failure" | "timed_out")
        );
    }
    IN_FLIGHT.contains(&r.status.as_str())
        && !r
            .jobs
            .iter()
            .any(|j| j.conclusion.as_deref() == Some("cancelled") && !r.carried(j))
}

/// The open pull requests' heads that `out` leaves with no run: each head that has, among
/// `s.runs`, a run of a workflow that decides it or will ([`covers`]), and has none once every
/// decision is acted on. A cancel paired with a re-run puts its run back in flight; any other
/// cancel ends it.
pub(super) fn heads_left_without_a_run(s: &Situation<'_>, out: &Supervision) -> Vec<String> {
    let acts: BTreeMap<u64, bool> = out.decisions.iter().map(|d| (d.run, d.rerun)).collect();
    let mut left = Vec::new();
    for (n, head) in s.pr_heads {
        let at_head: Vec<&RunObservation> = s
            .runs
            .iter()
            .filter(|r| {
                r.event == "pull_request" && r.pull_request == Some(*n) && r.head_sha == *head
            })
            .collect();
        let workflows: BTreeSet<&str> = at_head.iter().map(|r| r.workflow.as_str()).collect();
        for w in workflows {
            let of: Vec<&&RunObservation> = at_head.iter().filter(|r| r.workflow == w).collect();
            let before = of.iter().any(|r| covers(r));
            let after = of.iter().any(|r| match acts.get(&r.id) {
                Some(rerun) => *rerun,
                None => covers(r),
            });
            if before && !after {
                left.push(format!("#{n} at {head} ({w})"));
            }
        }
    }
    left
}

// ---------------------------------------------------------------- the generator

/// xorshift64*: small, deterministic, and enough to draw interleavings from.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn draw(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Whether an event of `per_mille` chance happens.
    fn chance(&mut self, per_mille: u64) -> bool {
        self.draw() % 1000 < per_mille
    }
    /// A whole number from `lo` to `hi`, both included.
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.draw() % (hi - lo + 1)
    }
}

/// Never, as a number of seconds.
const NEVER: u64 = u64::MAX;

/// The start of every world: 2026-10-29T00:00:00Z.
const T0: u64 = 1_793_232_000;

fn stamp(secs: u64) -> String {
    crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// One validate run of the world, its latest attempt with one job.
#[derive(Debug, Clone)]
struct SimRun {
    id: u64,
    pr: u64,
    sha: String,
    created: u64,
    attempt: u64,
    attempt_started: u64,
    /// `pending`, `queued`, `in_progress` or `completed`.
    status: &'static str,
    conclusion: Option<&'static str>,
    /// When the group admitted the attempt: when its job was created.
    admitted: Option<u64>,
    /// When a runner took its job.
    started: Option<u64>,
    /// How long after its admission a runner takes its job ([`NEVER`]: none ever does).
    waits: u64,
    /// How long its job runs once taken ([`NEVER`]: it hangs).
    takes: u64,
    /// Whether it passes.
    passes: bool,
    /// When it completed.
    ended: Option<u64>,
}

impl SimRun {
    fn jobs(&self) -> Vec<JobObservation> {
        let Some(created) = self.admitted else {
            return Vec::new();
        };
        let (status, conclusion) = match (self.status, self.started) {
            ("completed", _) => ("completed", self.conclusion),
            (_, Some(_)) => ("in_progress", None),
            _ => ("queued", None),
        };
        vec![JobObservation {
            name: "suite".into(),
            status: status.into(),
            // the forge fills a queued job's start with its creation
            started_at: Some(stamp(self.started.unwrap_or(created))),
            created_at: Some(stamp(created)),
            conclusion: conclusion.map(str::to_string),
        }]
    }

    fn observed(&self, with_jobs: bool) -> RunObservation {
        RunObservation {
            id: self.id,
            workflow: "validate".into(),
            event: "pull_request".into(),
            head_branch: format!("fix/p{}", self.pr),
            head_sha: self.sha.clone(),
            pull_request: Some(self.pr),
            status: self.status.into(),
            conclusion: self.conclusion.map(str::to_string),
            created_at: stamp(self.created),
            run_started_at: Some(stamp(self.attempt_started)),
            run_attempt: self.attempt,
            updated_at: Some(stamp(self.ended.unwrap_or(self.attempt_started))),
            jobs: if with_jobs { self.jobs() } else { Vec::new() },
        }
    }
}

/// The world: the runs, each pull request's head and label, and the clock.
struct World {
    rng: Rng,
    /// What people do by hand, drawn apart from `rng`, so that adding a person to the world
    /// changes none of its other interleavings.
    people: Rng,
    now: u64,
    next_id: u64,
    runs: Vec<SimRun>,
    heads: BTreeMap<u64, String>,
    pushes: BTreeMap<u64, u64>,
    full: BTreeSet<u64>,
    /// Whether a run was requested this minute: the supervisor's `workflow_run` trigger.
    requested: bool,
    /// Cancels the forge refused that land in the next minute all the same.
    late_cancels: Vec<u64>,
    /// How many times in a row each run's re-run was refused.
    refused_reruns: BTreeMap<u64, u32>,
    /// How often each half-failure, and each person's cancel, was injected.
    injected: BTreeMap<&'static str, u64>,
    /// The pull request whose every admitted run a person cancels, in one world of four.
    canceller: Option<u64>,
    /// What the forge was asked that it must never be: a re-run over a verdict of its head, or
    /// of a run at the attempt bound.
    violations: Vec<String>,
}

impl World {
    fn new(seed: u64, prs: u64) -> Self {
        let mut w = World {
            rng: Rng::new(seed),
            people: Rng::new(seed ^ 0x5EED_CA9C_E11E_D000),
            now: T0,
            next_id: 1000,
            runs: Vec::new(),
            heads: BTreeMap::new(),
            pushes: BTreeMap::new(),
            full: BTreeSet::new(),
            requested: false,
            late_cancels: Vec::new(),
            refused_reruns: BTreeMap::new(),
            injected: BTreeMap::new(),
            canceller: None,
            violations: Vec::new(),
        };
        if w.people.chance(250) {
            w.canceller = Some(w.people.range(1, prs));
        }
        for pr in 1..=prs {
            w.heads.insert(pr, format!("p{pr}-h0"));
            w.pushes.insert(pr, 0);
            // opened, half of them with a label: `opened` and `labeled`, one second apart
            w.create(pr);
            if w.rng.chance(500) {
                w.now += 1;
                w.create(pr);
            }
            w.now = T0;
        }
        w
    }

    /// What one attempt will do, drawn as it is asked for.
    fn behaviour(&mut self) -> (u64, u64, bool) {
        let waits = match self.rng.draw() % 100 {
            0..=64 => self.rng.range(0, 30 * 60),
            65..=84 => self.rng.range(30 * 60, 200 * 60),
            85..=97 => self.rng.range(230 * 60, 320 * 60),
            _ => NEVER,
        };
        let takes = match self.rng.draw() % 100 {
            0..=69 => self.rng.range(10 * 60, 150 * 60),
            70..=89 => self.rng.range(200 * 60, 290 * 60),
            _ => NEVER,
        };
        (waits, takes, self.rng.chance(800))
    }

    /// A new run of `pr`'s head, now, entering its group.
    fn create(&mut self, pr: u64) {
        let (waits, takes, passes) = self.behaviour();
        let id = self.next_id;
        self.next_id += 1 + self.rng.range(0, 5);
        self.runs.push(SimRun {
            id,
            pr,
            sha: self.heads[&pr].clone(),
            created: self.now,
            attempt: 1,
            attempt_started: self.now,
            status: "pending",
            conclusion: None,
            admitted: None,
            started: None,
            waits,
            takes,
            passes,
            ended: None,
        });
        self.enter(id);
    }

    /// Run `id` enters its pull request's group: a run pending there is replaced by it, as
    /// GitHub cancels the pending run a newer one replaces, and the group admits it when free.
    fn enter(&mut self, id: u64) {
        let pr = self.run(id).pr;
        let now = self.now;
        for r in self.runs.iter_mut() {
            if r.pr == pr && r.id != id && r.status == "pending" {
                r.status = "completed";
                r.conclusion = Some("cancelled");
                r.ended = Some(now);
            }
        }
        self.requested = true;
        self.admit(pr);
    }

    fn admit(&mut self, pr: u64) {
        let busy = self
            .runs
            .iter()
            .any(|r| r.pr == pr && matches!(r.status, "queued" | "in_progress"));
        if busy {
            return;
        }
        let now = self.now;
        if let Some(r) = self
            .runs
            .iter_mut()
            .find(|r| r.pr == pr && r.status == "pending")
        {
            r.status = "queued";
            r.admitted = Some(now);
        }
    }

    fn run(&self, id: u64) -> &SimRun {
        self.runs
            .iter()
            .find(|r| r.id == id)
            .expect("a run of the world")
    }

    /// One minute: runners take and finish jobs, groups admit, and people act.
    fn minute(&mut self) {
        self.now += 60;
        self.requested = false;
        let now = self.now;
        // a cancel `gh` reported failed, landing now
        for id in std::mem::take(&mut self.late_cancels) {
            let r = self.runs.iter_mut().find(|r| r.id == id).unwrap();
            if r.status != "completed" {
                r.status = "completed";
                r.conclusion = Some("cancelled");
                r.ended = Some(now);
                let pr = r.pr;
                self.admit(pr);
            }
        }
        for r in self.runs.iter_mut() {
            match r.status {
                "queued" if r.waits != NEVER && now >= r.admitted.unwrap() + r.waits => {
                    r.status = "in_progress";
                    r.started = Some(r.admitted.unwrap() + r.waits);
                }
                "in_progress" if r.takes != NEVER && now >= r.started.unwrap() + r.takes => {
                    r.status = "completed";
                    r.conclusion = Some(if r.passes { "success" } else { "failure" });
                    r.ended = Some(now);
                }
                _ => {}
            }
        }
        // a person who cancels every run of one pull request a few minutes after its admission
        if let Some(pr) = self.canceller {
            let doomed: Vec<u64> = self
                .runs
                .iter()
                .filter(|r| r.pr == pr && matches!(r.status, "queued" | "in_progress"))
                .filter(|r| r.admitted.is_some_and(|a| now >= a + 5 * 60))
                .map(|r| r.id)
                .collect();
            for id in doomed {
                self.cancel_by_hand(id, "hand_cancel");
            }
        }
        let prs: Vec<u64> = self.heads.keys().copied().collect();
        for pr in prs {
            self.admit(pr);
            if self.rng.chance(4) {
                // a push: the head moves, and its synchronize run is asked for
                let k = self.pushes[&pr] + 1;
                self.pushes.insert(pr, k);
                self.heads.insert(pr, format!("p{pr}-h{k}"));
                // a push that starts no run: `[skip ci]`, or made with the workflow token
                if !self.rng.chance(150) {
                    self.create(pr);
                }
            } else if self.rng.chance(1) {
                // a label added later, or a reopen: a second run of the head, asked for
                self.create(pr);
            } else if self.rng.chance(2) {
                // a reopen with a label: two runs of the head, one second apart
                self.create(pr);
                self.now += 1;
                self.create(pr);
                self.now = now;
            } else if self.rng.chance(2) {
                // a person re-runs the head's newest run once it has completed
                let head = self.heads[&pr].clone();
                let newest = self
                    .runs
                    .iter()
                    .filter(|r| r.pr == pr && r.sha == head)
                    .max_by_key(|r| r.created)
                    .map(|r| (r.id, r.status));
                if let Some((id, "completed")) = newest {
                    self.rerun(id);
                }
            } else if self.rng.chance(1) && !self.full.remove(&pr) {
                // ci:full, added; or, when it was there, removed
                self.full.insert(pr);
            } else if self.people.chance(2) {
                // a person cancels the head's newest run the group admitted, by hand
                let head = self.heads[&pr].clone();
                let newest = self
                    .runs
                    .iter()
                    .filter(|r| r.pr == pr && r.sha == head)
                    .filter(|r| matches!(r.status, "queued" | "in_progress"))
                    .max_by_key(|r| (r.attempt_started, r.id))
                    .map(|r| r.id);
                if let Some(id) = newest {
                    self.cancel_by_hand(id, "person_cancel");
                }
            }
        }
    }

    /// A person's cancel of run `id`: it ends at once, and its group admits the next.
    fn cancel_by_hand(&mut self, id: u64, what: &'static str) {
        let now = self.now;
        let r = self.runs.iter_mut().find(|r| r.id == id).unwrap();
        r.status = "completed";
        r.conclusion = Some("cancelled");
        r.ended = Some(now);
        let pr = r.pr;
        *self.injected.entry(what).or_default() += 1;
        self.admit(pr);
    }

    /// A new attempt of a completed run: every job again, entering its group.
    fn rerun(&mut self, id: u64) {
        let (waits, takes, passes) = self.behaviour();
        let now = self.now;
        let r = self.runs.iter_mut().find(|r| r.id == id).unwrap();
        r.attempt += 1;
        r.attempt_started = now;
        r.status = "pending";
        r.conclusion = None;
        r.admitted = None;
        r.started = None;
        r.waits = waits;
        r.takes = takes;
        r.passes = passes;
        r.ended = None;
        self.enter(id);
    }

    /// Whether `pr`'s head is decided, or will be: by the world's whole truth.
    fn head_covered(&self, pr: u64) -> bool {
        let head = &self.heads[&pr];
        self.runs
            .iter()
            .filter(|r| r.pr == pr && r.sha == *head)
            .any(|r| covers(&r.observed(true)))
    }

    /// The attempt of `pr`'s head's newest run, when that run ended `cancelled` at GitHub's
    /// attempt bound or past it and within the window, and the head has no run that covers it:
    /// the head is exhausted, and held.
    fn head_exhausted(&self, pr: u64) -> Option<u64> {
        let head = &self.heads[&pr];
        let newest = self
            .runs
            .iter()
            .filter(|r| r.pr == pr && r.sha == *head)
            .max_by_key(|r| (r.attempt_started, r.id))?;
        let recent = newest
            .ended
            .is_some_and(|e| e + RECOVERY_WINDOW_MINUTES * 60 >= self.now);
        (!self.head_covered(pr)
            && newest.conclusion == Some("cancelled")
            && newest.attempt >= MAX_ATTEMPTS
            && recent)
            .then_some(newest.attempt)
    }

    /// Whether `pr`'s head's newest run ended `cancelled` at GitHub's attempt bound or past it,
    /// within the window or not: a head that was exhausted, and stays so after the window, when it
    /// is no longer held either.
    fn head_spent(&self, pr: u64) -> bool {
        let head = &self.heads[&pr];
        self.runs
            .iter()
            .filter(|r| r.pr == pr && r.sha == *head)
            .max_by_key(|r| (r.attempt_started, r.id))
            .is_some_and(|r| r.conclusion == Some("cancelled") && r.attempt >= MAX_ATTEMPTS)
    }

    /// Whether `pr`'s head's newest run ended within the window: a bare head older than it is
    /// nobody's transient, and is not recovered.
    fn head_recent(&self, pr: u64) -> bool {
        let head = &self.heads[&pr];
        self.runs
            .iter()
            .filter(|r| r.pr == pr && r.sha == *head)
            .max_by_key(|r| (r.attempt_started, r.id))
            .and_then(|r| r.ended)
            .is_none_or(|e| e + RECOVERY_WINDOW_MINUTES * 60 >= self.now)
    }

    fn situation_runs(&self) -> Vec<RunObservation> {
        self.runs.iter().map(|r| r.observed(true)).collect()
    }
}

/// The forge, as the world: what it lists, and the two acts on it.
struct SimForge<'w> {
    world: &'w mut World,
}

impl RunForge for SimForge<'_> {
    fn observe(&self) -> Result<RunsObservation, ForgeError> {
        Ok(RunsObservation {
            repository: "o/r".into(),
            default_branch: "master".into(),
            runs: self
                .world
                .runs
                .iter()
                .filter(|r| r.status != "completed")
                .map(|r| r.observed(false))
                .collect(),
            pr_heads: self.world.heads.clone(),
            full: self.world.full.clone(),
            pr_branches: self
                .world
                .heads
                .keys()
                .map(|n| (*n, format!("fix/p{n}")))
                .collect(),
        })
    }
    fn jobs(&self, id: u64) -> Result<Vec<JobObservation>, ForgeError> {
        Ok(self.world.run(id).jobs())
    }
    fn run(&self, id: u64) -> Result<RunObservation, ForgeError> {
        Ok(self.world.run(id).observed(true))
    }
    fn pr_head(&self, number: u64) -> Result<Option<String>, ForgeError> {
        Ok(self.world.heads.get(&number).cloned())
    }
    fn force_cancel(&mut self, id: u64) -> Result<(), ForgeError> {
        let pr = self.world.run(id).pr;
        if self.world.run(id).status == "completed" {
            return Err(ForgeError(
                "gh: Cannot cancel a workflow run that is completed. (HTTP 409)".into(),
            ));
        }
        let half = self.world.rng.draw() % 100;
        if half < 12 {
            // it lands a minute later, while `gh` gave up: read again now, it is in flight
            self.world.late_cancels.push(id);
            *self.world.injected.entry("late_cancel").or_default() += 1;
            return Err(ForgeError("gh: Server Error (HTTP 502)".into()));
        }
        // at once, its `if: always()` jobs skipped; the group admits the run behind it
        let now = self.world.now;
        let r = self.world.runs.iter_mut().find(|r| r.id == id).unwrap();
        r.status = "completed";
        r.conclusion = Some("cancelled");
        r.ended = Some(now);
        self.world.admit(pr);
        if half < 24 {
            // it landed, and `gh` reported an error all the same
            *self
                .world
                .injected
                .entry("landed_cancel_error")
                .or_default() += 1;
            return Err(ForgeError(
                "gh api -X POST .../force-cancel timed out after 20 s".into(),
            ));
        }
        Ok(())
    }
    fn rerun(&mut self, id: u64) -> Result<Rerun, ForgeError> {
        let r = self.world.run(id).clone();
        if r.status != "completed" {
            return Err(ForgeError(format!("run {id} has not ended")));
        }
        // the forge's own check, as GhRunForge's: a run with a verdict is not asked
        if matches!(r.conclusion, Some("success" | "failure" | "timed_out")) {
            return Ok(Rerun::Decided);
        }
        // what the supervisor must check before it asks: the other runs of the head
        if let Some(v) = self.world.runs.iter().find(|o| {
            o.id != id
                && o.pr == r.pr
                && o.sha == r.sha
                && matches!(o.conclusion, Some("success" | "failure" | "timed_out"))
        }) {
            self.world.violations.push(format!(
                "run {id} of #{} at {} re-run over run {}'s verdict {:?}",
                r.pr, r.sha, v.id, v.conclusion
            ));
        }
        if r.attempt >= MAX_ATTEMPTS {
            self.world.violations.push(format!(
                "run {id} of #{} re-run at attempt {}, past {MAX_ATTEMPTS}",
                r.pr, r.attempt
            ));
        }
        let refused = self.world.refused_reruns.get(&id).copied().unwrap_or(0);
        if u64::from(refused) + 1 < MAX_ATTEMPTS && self.world.rng.chance(300) {
            self.world.refused_reruns.insert(id, refused + 1);
            *self.world.injected.entry("rerun_refused").or_default() += 1;
            return Err(ForgeError("gh: Server Error (HTTP 502)".into()));
        }
        self.world.refused_reruns.remove(&id);
        self.world.rerun(id);
        Ok(Rerun::Asked)
    }
    fn runs_at(&self, sha: &str) -> Result<Vec<RunObservation>, ForgeError> {
        Ok(self
            .world
            .runs
            .iter()
            .filter(|r| r.sha == sha)
            .map(|r| r.observed(false))
            .collect())
    }
    fn cancelled_since(&self, created_since: u64) -> Result<Vec<RunObservation>, ForgeError> {
        // newest first: the world creates its runs in order of time, and never reorders them
        Ok(self
            .world
            .runs
            .iter()
            .rev()
            .filter(|r| r.conclusion == Some("cancelled") && r.created >= created_since)
            .map(|r| r.observed(false))
            .collect())
    }
    fn newest_success(
        &self,
        _workflow: &str,
        _among: Successes<'_>,
    ) -> Result<Option<String>, ForgeError> {
        Ok(Some(stamp(self.world.now)))
    }
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

/// What the supervisors of every world did, so that a generator that stopped reaching a rule
/// fails rather than passes by having nothing to judge.
#[derive(Debug, Default)]
struct Seen {
    passes: u64,
    words: BTreeMap<String, u64>,
}

impl Seen {
    fn saw(&mut self, word: &str) {
        *self.words.entry(word.to_string()).or_default() += 1;
    }
    fn count(&self, word: &str) -> u64 {
        self.words.get(word).copied().unwrap_or_default()
    }
}

/// How many worlds, how many pull requests each, and how long each lives.
const WORLDS: u64 = 200;
const PRS: u64 = 3;
const HOURS: u64 = 14;

/// How many runners take the supervisor's passes, each with a checkout and a trail of its own:
/// lundra's three. The hosted lane's fresh checkout is one more that remembers nothing.
const RUNNERS: usize = 3;

/// A repository of its own for each runner of each world: the trail is each runner's own, and
/// two worlds share run and pull request numbers.
fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let ok = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success();
    assert!(ok);
    root
}

/// Whether `report` names #`pr` anywhere a person reads it: an act, a recovery, a hold of its
/// head, or what could not be read.
fn names(report: &runs::SuperviseReport, pr: u64) -> bool {
    let n = format!("#{pr}");
    report
        .acts
        .iter()
        .map(|a| &a.detail)
        .chain(report.recovered.iter().map(|r| &r.detail))
        .chain(report.unread.iter())
        .any(|d| d.contains(&n))
        || report.held.iter().any(|h| {
            matches!(h.reason, HoldReason::HeadExhausted { pull_request, .. } if pull_request == pr)
        })
}

/// Whether a pass's report reads as failed for a reason of its own: an act of it that did not go
/// through. Only these may make it exit 12; what recovery finds is a pull request's state.
fn failed_itself(report: &runs::SuperviseReport) -> bool {
    report.acts.iter().any(|a| {
        !matches!(
            a.outcome,
            ActOutcome::Cancelled | ActOutcome::CancelledAndRerun | ActOutcome::Skipped
        )
    }) || report
        .recovered
        .iter()
        .any(|r| r.outcome == RecoveryOutcome::Unrecorded)
}

/// Whether `report` warns of #`pr`'s head in a line the workflow raises as an annotation.
fn warns(report: &runs::SuperviseReport, pr: u64) -> bool {
    let mut out = Vec::new();
    crate::commands::prs::render_supervise(report, &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .any(|l| l.starts_with("::warning ") && l.contains(&format!("#{pr}")))
}

/// One world of `seed`, every supervision of it asserted.
fn live(seed: u64, m: &SuperviseModel, seen: &mut Seen) {
    let runners: Vec<tempfile::TempDir> = (0..RUNNERS).map(|_| repository()).collect();
    let mut w = World::new(seed, PRS);
    // each pull request's head that had a run, and for how many passes in a row it has had
    // none since
    let mut covered: BTreeMap<u64, String> = BTreeMap::new();
    let mut bare: BTreeMap<u64, u32> = BTreeMap::new();
    // how many passes in a row each pull request's head has been held exhausted
    let mut exhausted: BTreeMap<u64, u32> = BTreeMap::new();
    // which runner ended each run, so that a recovery by another is seen
    let mut ended_by: BTreeMap<u64, usize> = BTreeMap::new();
    // how many half-failures the forge had injected when the previous pass began: a head is
    // bared only by one, in that pass or in this one, or by a person between them
    let injected = |w: &World| w.injected.values().sum::<u64>();
    let mut injected_before_last = 0;
    // which runner takes a pass, and whether a second lane follows it, drawn apart from the
    // world: the world's own interleavings are the same however many runners take its passes
    let mut lanes_rng = Rng::new(seed ^ 0x1A4E_5000_0000_0001);
    // a supervisor that is down for a stretch, in two worlds of five, never as long as the
    // recovery window: a longer outage is the queue's `supervise_stale`, not recovery's to undo
    let (down_from, down_to) = if w.rng.chance(400) {
        // most of them from the opening burst on, while its twins wait
        let from = if w.rng.chance(650) {
            T0
        } else {
            T0 + w.rng.range(0, 8 * 3600)
        };
        // an outage from the opening burst lasts at least four hours, so that its twins meet
        // the supervisor again near their bounds (`earlier_run_ends`)
        let shortest = if from == T0 { 4 * 3600 } else { 60 * 60 };
        (from, from + w.rng.range(shortest, 5 * 3600))
    } else {
        (0, 0)
    };
    const { assert!(5 * 60 < RECOVERY_WINDOW_MINUTES) };
    for minute in 0..HOURS * 60 {
        if minute > 0 {
            w.minute();
        }
        let tick = minute % 10 == 0 || w.requested;
        if !tick || (down_from..down_to).contains(&w.now) {
            continue;
        }
        let injected_at_start = injected(&w);
        let before: BTreeMap<u64, bool> = w
            .heads
            .keys()
            .map(|pr| (*pr, w.head_covered(*pr)))
            .collect();
        // the decision alone, over the world's whole truth, keeps every head
        let all = w.situation_runs();
        let situation = Situation {
            runs: &all,
            pr_heads: &w.heads,
            full: &w.full,
            default_branch: "master",
            model: m,
            now: w.now,
            self_run: None,
        };
        let decided = supervise::decide(&situation);
        let left = heads_left_without_a_run(&situation, &decided);
        assert!(
            left.is_empty(),
            "seed {seed} at {}: decide leaves {left:?}: {decided:?}\n{:#?}",
            stamp(w.now),
            w.runs
        );
        // the command's own apply, over the forge that is the world, on whichever runner takes
        // the job; and in some minutes the other lane applies right after it, on the same heads
        let now = w.now;
        let snapshot = w.runs.clone();
        let first = usize::try_from(lanes_rng.range(0, RUNNERS as u64 - 1)).unwrap();
        let mut lanes = vec![first];
        if lanes_rng.chance(150) {
            lanes.push((first + 1) % RUNNERS);
            seen.saw("second_lane");
        }
        let mut reports = Vec::new();
        for lane in lanes {
            let before_pass = injected(&w);
            let report = {
                let mut forge = SimForge { world: &mut w };
                runs::supervise(
                    runners[lane].path(),
                    &mut forge,
                    m,
                    &|| now,
                    None,
                    true,
                    None,
                )
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"))
            };
            seen.passes += 1;
            // a pass is red only for an ask of its own the forge refused, and the forge of the
            // world refuses only by a half-failure injected during that pass: a held head, an
            // exhausted one among them, never turns it red
            if report.exit_code() == 12 {
                assert!(
                    failed_itself(&report) && injected(&w) > before_pass,
                    "seed {seed} at {}: a red pass with nothing of its own refused\n{report:#?}",
                    stamp(now)
                );
                seen.saw("red_pass");
            } else {
                assert_eq!(report.exit_code(), 0, "seed {seed}: {report:#?}");
            }
            for d in &report.decisions {
                seen.saw(d.reason.word());
                if d.rerun {
                    seen.saw("rerun");
                }
            }
            for h in &report.held {
                seen.saw(
                    serde_json::to_value(&h.reason).unwrap()["kind"]
                        .as_str()
                        .unwrap(),
                );
            }
            for a in &report.acts {
                assert!(
                    matches!(
                        a.outcome,
                        ActOutcome::Cancelled
                            | ActOutcome::CancelledAndRerun
                            | ActOutcome::Skipped
                            | ActOutcome::Failed
                            | ActOutcome::RerunFailed
                    ),
                    "seed {seed} at {}: an act the world cannot explain: {a:?}",
                    stamp(now)
                );
                match a.outcome {
                    ActOutcome::CancelledAndRerun => seen.saw("cancelled_and_rerun"),
                    ActOutcome::RerunFailed => seen.saw("act_rerun_failed"),
                    ActOutcome::Failed => seen.saw("act_failed"),
                    _ => {}
                }
                if matches!(a.outcome, ActOutcome::Cancelled | ActOutcome::RerunFailed) {
                    ended_by.insert(a.run, lane);
                }
                if a.outcome == ActOutcome::CancelledAndRerun && a.detail.contains("not cancelled")
                {
                    seen.saw("rerun_after_a_refused_cancel");
                }
                if a.detail.contains("has its verdict") {
                    seen.saw("verdict_kept");
                }
            }
            for r in &report.recovered {
                assert!(
                    matches!(
                        r.outcome,
                        RecoveryOutcome::Rerun | RecoveryOutcome::RerunFailed
                    ),
                    "seed {seed} at {}: a recovery the world cannot explain: {r:?}\n{report:#?}",
                    stamp(now)
                );
                seen.saw(match r.outcome {
                    RecoveryOutcome::Rerun => "recovered",
                    _ => "recovery_refused",
                });
                if r.outcome == RecoveryOutcome::Rerun
                    && ended_by.get(&r.run).is_some_and(|l| *l != lane)
                {
                    seen.saw("recovered_by_another_runner");
                }
            }
            reports.push(report);
        }
        assert!(
            w.violations.is_empty(),
            "seed {seed} at {}: {:?}\nbefore: {snapshot:#?}\nafter: {:#?}\n{reports:#?}",
            stamp(now),
            w.violations,
            w.runs
        );
        let last = reports.last().unwrap();
        // a head the world could not bare stays covered through the pass; one it bared gets its
        // run back within MAX_ATTEMPTS passes from whichever runners take them, and a pass that
        // leaves it bare says so; one whose newest run reached the attempt bound is held
        for (pr, had) in before {
            if had {
                covered.insert(pr, w.heads[&pr].clone());
            }
            let head = w.heads[&pr].clone();
            if let Some(attempts) = w.head_exhausted(pr) {
                bare.remove(&pr);
                assert!(
                    last.held.iter().any(|h| h.reason
                        == HoldReason::HeadExhausted {
                            pull_request: pr,
                            attempts
                        })
                        && warns(last, pr),
                    "seed {seed} at {}: #{pr}'s head {head} is exhausted and not held\n{last:#?}",
                    stamp(now)
                );
                let n = exhausted.entry(pr).or_default();
                *n += 1;
                seen.saw(if *n > 1 {
                    "exhausted_persists"
                } else {
                    "exhausted_found"
                });
                continue;
            }
            exhausted.remove(&pr);
            if covered.get(&pr) != Some(&head) || w.head_covered(pr) {
                bare.remove(&pr);
                continue;
            }
            // an exhausted head, held while it was within the window, is left alone past it
            if w.head_spent(pr) && !w.head_recent(pr) {
                bare.remove(&pr);
                seen.saw("exhausted_aged_out");
                continue;
            }
            assert!(
                w.head_recent(pr),
                "seed {seed} at {}: #{pr}'s head {head} was left bare past the window",
                stamp(now)
            );
            let passes = bare.entry(pr).or_default();
            *passes += 1;
            assert!(
                *passes > 1 || injected(&w) > injected_before_last,
                "seed {seed} at {}: #{pr}'s head {head} lost its run with nothing half-failing\n\
                 reports: {reports:#?}\nbefore: {snapshot:#?}\nafter: {:#?}",
                stamp(now),
                w.runs
            );
            assert!(
                u64::from(*passes) < MAX_ATTEMPTS,
                "seed {seed} at {}: #{pr}'s head {head} has had no run for {passes} passes\n\
                 reports: {reports:#?}\nbefore: {snapshot:#?}\nafter: {:#?}",
                stamp(now),
                w.runs
            );
            // said by the pass whose own act bared it, red; by a later pass, a warning
            assert!(
                reports
                    .iter()
                    .any(|r| names(r, pr) && (r.exit_code() == 12 || warns(r, pr))),
                "seed {seed} at {}: #{pr}'s head {head} has no run and no pass says so\n\
                 {reports:#?}",
                stamp(now)
            );
            if *passes > 1 {
                seen.saw("bare_head_warned");
            }
            seen.saw("bare_for_a_pass");
        }
        injected_before_last = injected_at_start;
    }
    for (what, n) in &w.injected {
        *seen.words.entry((*what).to_string()).or_default() += n;
    }
}

#[test]
fn no_supervision_leaves_a_pull_requests_head_without_a_run() {
    let m = model();
    let mut seen = Seen::default();
    for seed in 0..WORLDS {
        live(seed, &m, &mut seen);
    }
    // the worlds reach every rule, and every way the invariant is kept
    for word in [
        "duplicate_run",
        "superseded_blocks_successor",
        "queued_past_bound",
        "running_past_bound",
        "rerun",
        "cancelled_and_rerun",
        "last_run_of_head",
        "earlier_run_ends",
        "superseded_without_successor",
        // and every half-failure, every person's cancel, and the way back from each
        "late_cancel",
        "landed_cancel_error",
        "rerun_refused",
        "person_cancel",
        "hand_cancel",
        "act_failed",
        "act_rerun_failed",
        "rerun_after_a_refused_cancel",
        "recovered",
        "recovery_refused",
        "recovered_by_another_runner",
        "second_lane",
        "red_pass",
        "bare_for_a_pass",
        "bare_head_warned",
        // a head at the attempt bound, held on the pass that finds it and on every later one
        "head_exhausted",
        "exhausted_found",
        "exhausted_persists",
    ] {
        assert!(seen.count(word) > 0, "no world reached {word}: {seen:?}");
    }
    assert!(seen.passes > WORLDS * HOURS * 3, "{seen:?}");
}

#[test]
fn the_checker_finds_a_head_left_without_a_run() {
    // the property is only as good as its checker: a cancel of a head's only run, unpaired, is
    // named; paired with a re-run, or with another run left, it is not
    let mut w = World::new(7, 1);
    w.runs.truncate(1);
    let runs = w.situation_runs();
    let m = model();
    let empty = BTreeSet::new();
    let s = Situation {
        runs: &runs,
        pr_heads: &w.heads,
        full: &empty,
        default_branch: "master",
        model: &m,
        now: w.now,
        self_run: None,
    };
    let only = runs[0].clone();
    let cancel = |rerun| Supervision {
        decisions: vec![supervise::Decision {
            run: only.id,
            workflow: only.workflow.clone(),
            event: only.event.clone(),
            head_branch: only.head_branch.clone(),
            head_sha: only.head_sha.clone(),
            pull_request: only.pull_request,
            status: only.status.clone(),
            reason: supervise::CancelReason::QueuedPastBound {
                minutes: 241,
                bound: 240,
            },
            rerun,
        }],
        held: Vec::new(),
    };
    assert_eq!(
        heads_left_without_a_run(&s, &cancel(false)),
        vec!["#1 at p1-h0 (validate)".to_string()]
    );
    assert!(heads_left_without_a_run(&s, &cancel(true)).is_empty());
    let mut two = runs.clone();
    let mut other = only.clone();
    other.id += 1;
    two.push(other);
    let s2 = Situation { runs: &two, ..s };
    assert!(heads_left_without_a_run(&s2, &cancel(false)).is_empty());
}
