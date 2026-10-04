//! The executor: one merge at a time, each against a master observed a moment before.
//!
//! # The invariant, structurally
//!
//! [`step`] takes no plan. It observes the forge, builds the queue, picks the first ready
//! pull request, observes the forge *again*, rebuilds the queue, and merges only if the
//! second decision names the same master and head as the first and still says `ready`.
//! [`drain`] is a loop over `step` and nothing else, so no candidate list survives a merge:
//! there is no variable for it to survive in.
//!
//! # What it will not do
//!
//! It never passes `--admin`, never merges a pull request whose required checks are not
//! all passed on its current head, never merges one that does not contain the current
//! master, never force-pushes, and never closes a pull request — closure is
//! [`super::drain::cleanup`]'s, which demands stronger evidence and an explicit `--apply`.
//! Only one executor per base branch runs at a time ([`IntegrationLease`]).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    local_master, queue_of, refresh, state_path, IntegrationQueue, PullRequestAssessment,
    PullRequestDisposition, EVENTS_FILE,
};

/// A lease untouched for this long belongs to a process that is gone.
pub const LEASE_STALE_AFTER: Duration = Duration::from_secs(1800);

/// How long a merge is given to show as merged on the forge before verification fails.
pub const MERGE_VISIBLE_WITHIN: Duration = Duration::from_secs(60);

/// How long a required check that has not reported (`missing`) on a head the executor pushed
/// still holds the refresh pipeline, counted from the `refreshed` event. A forge creates no
/// check run for an aggregate job until every job it needs has finished, so for most of a CI
/// run the executor's own head reads as `missing`, not `pending`. Four hours sits above the
/// slowest healthy suite measured here (about 150 minutes, `.github/workflows/validate.yml`)
/// with queue time and margin; past it the check is taken never to report, and the pipeline
/// moves on rather than freezing. A run that still reports later costs one wasted CI run.
pub const REFRESHED_HEAD_REPORTS_WITHIN: Duration = Duration::from_secs(4 * 3600);

/// The one executor of a base branch: an exclusive file under the *common* git directory,
/// so every worktree of the repository contends on the same file and no other repository
/// sees it. Read-only observers never take it.
pub struct IntegrationLease {
    path: PathBuf,
    token: String,
}

/// Who holds the lease.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeaseHolder {
    /// The process.
    pub pid: u32,
    /// The machine.
    pub host: String,
    /// The base branch.
    pub base: String,
    /// When it was taken, seconds since the epoch.
    pub since: u64,
}

fn common_dir(root: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("git could not name the common directory".into());
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl IntegrationLease {
    /// The lease file of a base branch.
    pub fn path_for(common: &Path, base: &str) -> PathBuf {
        common
            .join("majordomus/locks")
            .join(format!("integration-{}.lock", base.replace('/', "-")))
    }

    /// Take the lease, or say who holds it. A lease untouched for [`LEASE_STALE_AFTER`] is
    /// reclaimed: its holder stopped without releasing it.
    pub fn acquire(root: &Path, base: &str) -> Result<Self, String> {
        let path = Self::path_for(&common_dir(root)?, base);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let holder = LeaseHolder {
            pid: std::process::id(),
            host: host(),
            base: base.to_string(),
            since: now_secs(),
        };
        let token = serde_json::to_string(&holder).map_err(|e| e.to_string())?;
        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    let _ = f.write_all(token.as_bytes());
                    return Ok(IntegrationLease { path, token });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|m| SystemTime::now().duration_since(m).ok())
                        .is_some_and(|age| age > LEASE_STALE_AFTER);
                    if stale {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    let held = fs::read_to_string(&path).unwrap_or_default();
                    return Err(format!(
                        "another integration executor holds {}: {}",
                        path.display(),
                        held.trim()
                    ));
                }
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Err(format!("{}: could not be taken", path.display()))
    }

    /// Mark the lease alive: a long drain renews it between steps.
    pub fn renew(&self) {
        let _ = fs::write(&self.path, &self.token);
    }

    /// Who holds the lease of `base` now, read without taking it: what an observer shows.
    /// `None` when nobody does.
    pub fn read(root: &Path, base: &str) -> Result<Option<IntegrationLeaseState>, String> {
        let path = Self::path_for(&common_dir(root)?, base);
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(None);
        };
        let age = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .unwrap_or_default();
        Ok(Some(IntegrationLeaseState {
            holder: serde_json::from_str(text.trim()).ok(),
            renewed_seconds_ago: age.as_secs(),
            stale: age > LEASE_STALE_AFTER,
        }))
    }
}

/// The lease of a base as an observer reads it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationLeaseState {
    /// Who holds it; `None` when the file does not say in a form this reads.
    pub holder: Option<LeaseHolder>,
    /// How long ago the holder last renewed it.
    pub renewed_seconds_ago: u64,
    /// Whether it is older than [`LEASE_STALE_AFTER`]: its holder stopped without releasing
    /// it, and the next executor takes it over.
    pub stale: bool,
}

impl Drop for IntegrationLease {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path).is_ok_and(|c| c.trim() == self.token) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// One entry of the audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationEvent {
    /// When, RFC 3339.
    pub at: String,
    /// Who: the git identity, the process and the machine.
    pub actor: String,
    /// What (`selected`, `refresh_selected`, `stale_decision`, `merge_attempted`,
    /// `merge_succeeded`, `merge_failed`, `verification_failed`, `refreshed`,
    /// `refresh_failed`, `closed_superseded`, `idle`, and the two transitions the wait is
    /// folded from: `became_actionable`, `left_actionable`).
    pub action: String,
    /// The pull request, when one.
    pub pr: Option<u64>,
    /// Master before.
    pub master_before: Option<String>,
    /// The head acted on.
    pub head_sha: Option<String>,
    /// Master after.
    pub master_after: Option<String>,
    /// The decision's reason codes.
    pub reasons: Vec<String>,
    /// What happened, for a person.
    pub detail: String,
    /// On a selection: the other actionable pull requests the executor chose this one over,
    /// in rank order. The wait of each is folded from it ([`super::wait`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passed_over: Vec<u64>,
    /// On `refreshed`: the head the executor pushed. The refresh pipeline waits only for
    /// the checks of a head named here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_after: Option<String>,
}

fn actor() -> String {
    let who = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    format!("{who} (pid {} on {})", std::process::id(), host())
}

/// Append one event to the audit trail. The trail is append-only JSON lines.
pub fn record(root: &Path, mut event: IntegrationEvent) -> IntegrationEvent {
    if event.at.is_empty() {
        event.at = crate::peers::rfc3339(SystemTime::now());
    }
    if event.actor.is_empty() {
        event.actor = actor();
    }
    let path = state_path(root, EVENTS_FILE);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let (Ok(line), Ok(mut f)) = (
        serde_json::to_string(&event),
        fs::OpenOptions::new().create(true).append(true).open(&path),
    ) {
        let _ = writeln!(f, "{line}");
    }
    event
}

/// Every recorded event, oldest first.
pub fn events(root: &Path) -> Vec<IntegrationEvent> {
    fs::read_to_string(state_path(root, EVENTS_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn event(
    action: &str,
    pr: Option<&PullRequestAssessment>,
    detail: impl Into<String>,
) -> IntegrationEvent {
    IntegrationEvent {
        at: String::new(),
        actor: String::new(),
        action: action.into(),
        pr: pr.map(|a| a.number),
        master_before: pr.map(|a| a.evaluated_against.master_sha.clone()),
        head_sha: pr.map(|a| a.evaluated_against.head_sha.clone()),
        master_after: None,
        reasons: pr.map(|a| a.reasons.clone()).unwrap_or_default(),
        detail: detail.into(),
        passed_over: Vec::new(),
        head_after: None,
    }
}

/// The other pull requests of `queue` with disposition `d`, in rank order, except `chosen`:
/// what a selection passes over.
fn others(queue: &IntegrationQueue, d: PullRequestDisposition, chosen: u64) -> Vec<u64> {
    queue
        .assessments
        .iter()
        .filter(|a| a.disposition == d && a.number != chosen)
        .map(|a| a.number)
        .collect()
}

/// What one step did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum DrainStepOutcome {
    /// Nothing is ready.
    Idle {
        /// Why the queue did not yield a merge.
        why: String,
    },
    /// Dry run: this would have been merged.
    WouldMerge {
        /// The pull request.
        pr: u64,
    },
    /// Dry run: master would have been brought into this pull request.
    WouldRefresh {
        /// The pull request.
        pr: u64,
    },
    /// Master was brought into the branch; its required checks now run on the new head.
    Refreshed {
        /// The pull request.
        pr: u64,
        /// The head before.
        head_before: String,
        /// The head after.
        head_after: String,
    },
    /// A refreshed pull request is waiting for its checks; refreshing another now would be
    /// wasted, because merging the first re-stales it.
    AwaitingChecks {
        /// The pull request whose checks are running.
        pr: u64,
    },
    /// Bringing master in failed; the candidate is reclassified on the next step.
    RefreshFailed {
        /// The pull request.
        pr: u64,
        /// Why.
        reason: String,
    },
    /// The decision went stale between planning and acting; nothing was merged.
    StaleDecision {
        /// The pull request.
        pr: u64,
        /// What moved.
        what: String,
    },
    /// Merged and verified.
    Merged {
        /// The pull request.
        pr: u64,
        /// Master before.
        master_before: String,
        /// Master after.
        master_after: String,
    },
    /// The forge refused the merge; the candidate is reclassified on the next step.
    MergeRefused {
        /// The pull request.
        pr: u64,
        /// The forge's words.
        reason: String,
    },
    /// Merged, but what landed could not be verified: the drain stops.
    VerificationFailed {
        /// The pull request.
        pr: u64,
        /// What did not hold.
        reason: String,
    },
}

/// A source of queues and a merger. The command line uses the forge and git; the tests use
/// a scripted repository, which is how the re-plan-after-every-merge property is proved
/// without a network.
pub trait Integrator {
    /// Observe the forge now and build the queue against the master observed.
    fn observe(&mut self) -> Result<IntegrationQueue, String>;
    /// Merge one pull request, requiring the forge's head to still be `head_sha`.
    fn merge(&mut self, pr: u64, head_sha: &str, method: &str) -> Result<(), String>;
    /// After a merge: the pull request's state on the forge and this clone's master.
    fn verify(&mut self, pr: u64, head_sha: &str) -> Result<String, String>;
    /// Bring master into a pull request's branch — a merge commit with the derived driver
    /// and a fresh derive, pushed as a fast-forward of the observed head. Returns the new
    /// head. Never a rewrite: the push is refused if the branch moved.
    fn refresh_branch(&mut self, a: &PullRequestAssessment, base: &str) -> Result<String, String>;
}

/// One integration step: observe, decide, observe again, act only on an unchanged
/// decision, verify. `dry_run` stops after the first decision.
pub fn step(
    root: &Path,
    integrator: &mut dyn Integrator,
    dry_run: bool,
    allow_refresh: bool,
) -> Result<DrainStepOutcome, String> {
    let first = integrator.observe()?;
    if !dry_run {
        // what became or stopped being actionable since the trail's last word: the wait
        // of every pull request is folded from these, so a dry run leaves them out too
        super::wait::record_transitions(root, &first);
    }
    let Some(candidate) = first.next_merge.and_then(|n| first.get(n).cloned()) else {
        if allow_refresh {
            if let Some(outcome) = refresh_step(root, integrator, &first, dry_run)? {
                return Ok(outcome);
            }
        }
        let why = if first.next_refresh.is_empty() {
            format!(
                "nothing is ready among {} open pull request(s)",
                first.tallies.open
            )
        } else {
            format!(
                "nothing is ready; {} pull request(s) need master brought in first: {}",
                first.next_refresh.len(),
                first
                    .next_refresh
                    .iter()
                    .map(|n| format!("#{n}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        if !dry_run {
            record(root, event("idle", None, why.clone()));
        }
        return Ok(DrainStepOutcome::Idle { why });
    };
    if dry_run {
        return Ok(DrainStepOutcome::WouldMerge {
            pr: candidate.number,
        });
    }
    let mut selected = event("selected", Some(&candidate), "the first ready pull request");
    selected.passed_over = others(&first, PullRequestDisposition::Ready, candidate.number);
    record(root, selected);
    // the decision is re-taken from a new observation; acting on the first one would be
    // acting on a picture of the repository that may already be wrong
    let second = integrator.observe()?;
    let fresh = second.get(candidate.number);
    let stale = match fresh {
        None => Some("it is no longer open".to_string()),
        Some(a) if a.evaluated_against.master_sha != candidate.evaluated_against.master_sha => {
            Some(format!(
                "master moved from {} to {}",
                candidate.evaluated_against.master_sha, a.evaluated_against.master_sha
            ))
        }
        Some(a) if a.evaluated_against.head_sha != candidate.evaluated_against.head_sha => {
            Some(format!(
                "its head moved from {} to {}",
                candidate.evaluated_against.head_sha, a.evaluated_against.head_sha
            ))
        }
        Some(a) if a.disposition != PullRequestDisposition::Ready => {
            Some(format!("it is {} now", a.disposition.as_str()))
        }
        Some(_) => None,
    };
    if let Some(what) = stale {
        record(
            root,
            event("stale_decision", Some(&candidate), what.clone()),
        );
        return Ok(DrainStepOutcome::StaleDecision {
            pr: candidate.number,
            what,
        });
    }
    let method = second.policy.merge_method.clone();
    record(
        root,
        event("merge_attempted", Some(&candidate), format!("--{method}")),
    );
    if let Err(reason) = integrator.merge(
        candidate.number,
        &candidate.evaluated_against.head_sha,
        &method,
    ) {
        record(
            root,
            event("merge_failed", Some(&candidate), reason.clone()),
        );
        return Ok(DrainStepOutcome::MergeRefused {
            pr: candidate.number,
            reason,
        });
    }
    match integrator.verify(candidate.number, &candidate.evaluated_against.head_sha) {
        Ok(master_after) => {
            let mut e = event("merge_succeeded", Some(&candidate), "merged and verified");
            e.master_after = Some(master_after.clone());
            record(root, e);
            Ok(DrainStepOutcome::Merged {
                pr: candidate.number,
                master_before: candidate.evaluated_against.master_sha.clone(),
                master_after,
            })
        }
        Err(reason) => {
            record(
                root,
                event("verification_failed", Some(&candidate), reason.clone()),
            );
            Ok(DrainStepOutcome::VerificationFailed {
                pr: candidate.number,
                reason,
            })
        }
    }
}

/// The refresh half of a step, when nothing is ready. Pipeline depth one: while a pull
/// request the executor refreshed waits for its checks, no other is refreshed — merging the
/// first would put the second behind again and waste its CI run.
///
/// Only a head the executor pushed holds the pipeline — one a `refreshed` event of the trail
/// names as `head_after` — and only while its required check is *pending*, or *missing* for
/// less than [`REFRESHED_HEAD_REPORTS_WITHIN`] since that event: an aggregate check is not
/// created until the jobs it needs finish, and a check that never reports must not hold every
/// refresh forever. A check running on a head the author pushed is not the executor's run to
/// wait for.
fn refresh_step(
    root: &Path,
    integrator: &mut dyn Integrator,
    first: &IntegrationQueue,
    dry_run: bool,
) -> Result<Option<DrainStepOutcome>, String> {
    // each pushed head with the moment it was pushed; on a trail line without a readable
    // time a pending check still holds, and a missing one does not
    let mut pushed: BTreeMap<(u64, String), Option<i64>> = BTreeMap::new();
    for e in events(root).into_iter().filter(|e| e.action == "refreshed") {
        if let Some(key) = e.pr.zip(e.head_after) {
            let at = crate::peers::epoch_seconds(&e.at);
            let slot = pushed.entry(key).or_insert(at);
            *slot = (*slot).max(at);
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let within = REFRESHED_HEAD_REPORTS_WITHIN.as_secs() as i64;
    if let Some(waiting) = first.assessments.iter().find(|a| {
        a.disposition == PullRequestDisposition::WaitingForChecks
            && matches!(a.relation, super::RelationToMaster::UpToDate { .. })
            && pushed
                .get(&(a.number, a.evaluated_against.head_sha.clone()))
                .is_some_and(|at| {
                    a.required_checks == super::RequiredCheckState::Pending
                        || (a.required_checks == super::RequiredCheckState::Missing
                            && at.is_some_and(|t| now - t < within))
                })
    }) {
        return Ok(Some(DrainStepOutcome::AwaitingChecks {
            pr: waiting.number,
        }));
    }
    let Some(candidate) = first
        .next_refresh
        .first()
        .and_then(|n| first.get(*n))
        .cloned()
    else {
        return Ok(None);
    };
    if dry_run {
        return Ok(Some(DrainStepOutcome::WouldRefresh {
            pr: candidate.number,
        }));
    }
    let mut selected = event(
        "refresh_selected",
        Some(&candidate),
        "the first pull request that needs master",
    );
    selected.passed_over = others(
        first,
        PullRequestDisposition::NeedsRefresh,
        candidate.number,
    );
    record(root, selected);
    let second = integrator.observe()?;
    let fresh = second.get(candidate.number);
    let stale = match fresh {
        None => Some("it is no longer open".to_string()),
        Some(a) if a.evaluated_against != candidate.evaluated_against => Some(format!(
            "master or its head moved (now master {}, head {})",
            a.evaluated_against.master_sha, a.evaluated_against.head_sha
        )),
        Some(a) if a.disposition != PullRequestDisposition::NeedsRefresh => {
            Some(format!("it is {} now", a.disposition.as_str()))
        }
        Some(_) => None,
    };
    if let Some(what) = stale {
        record(
            root,
            event("stale_decision", Some(&candidate), what.clone()),
        );
        return Ok(Some(DrainStepOutcome::StaleDecision {
            pr: candidate.number,
            what,
        }));
    }
    match integrator.refresh_branch(&candidate, &second.base) {
        Ok(head_after) => {
            let mut e = event(
                "refreshed",
                Some(&candidate),
                format!("new head {head_after}"),
            );
            e.master_after = Some(second.master_sha.clone());
            e.head_after = Some(head_after.clone());
            record(root, e);
            Ok(Some(DrainStepOutcome::Refreshed {
                pr: candidate.number,
                head_before: candidate.evaluated_against.head_sha.clone(),
                head_after,
            }))
        }
        Err(reason) => {
            record(
                root,
                event("refresh_failed", Some(&candidate), reason.clone()),
            );
            Ok(Some(DrainStepOutcome::RefreshFailed {
                pr: candidate.number,
                reason,
            }))
        }
    }
}

/// What a drain did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DrainReport {
    /// Whether anything was allowed to change.
    pub dry_run: bool,
    /// Every step, in order.
    pub steps: Vec<DrainStepOutcome>,
    /// Pull requests merged.
    pub merged: Vec<u64>,
    /// Why it stopped.
    pub stopped: String,
}

/// Merge until `max` merges, an idle queue, or a systemic failure. A stale decision or a
/// refused merge is candidate-specific: the next step re-plans from a new observation,
/// bounded by `max_steps` so a flapping queue cannot spin.
pub fn drain(
    root: &Path,
    integrator: &mut dyn Integrator,
    max: usize,
    dry_run: bool,
    allow_refresh: bool,
) -> Result<DrainReport, String> {
    let mut report = DrainReport {
        dry_run,
        steps: Vec::new(),
        merged: Vec::new(),
        stopped: String::new(),
    };
    let max_steps = max.saturating_mul(3).max(3);
    while report.merged.len() < max {
        if report.steps.len() >= max_steps {
            report.stopped = format!("{max_steps} steps without reaching {max} merge(s)");
            break;
        }
        let outcome = step(root, integrator, dry_run, allow_refresh)?;
        let stop = match &outcome {
            DrainStepOutcome::Idle { why } => Some(why.clone()),
            DrainStepOutcome::WouldMerge { pr } => Some(format!(
                "dry run: #{pr} would be merged; nothing after it is planned, because its merge would change the queue"
            )),
            DrainStepOutcome::Merged { pr, .. } => {
                report.merged.push(*pr);
                None
            }
            DrainStepOutcome::StaleDecision { .. } | DrainStepOutcome::MergeRefused { .. } => None,
            DrainStepOutcome::WouldRefresh { pr } => Some(format!(
                "dry run: master would be brought into #{pr}; its checks must then pass before it can merge"
            )),
            DrainStepOutcome::Refreshed { pr, .. } => Some(format!(
                "#{pr} now contains master; its required checks run on the new head, and the next drain merges it when they pass"
            )),
            DrainStepOutcome::AwaitingChecks { pr } => Some(format!(
                "#{pr} contains master and its checks are running; nothing else is refreshed meanwhile"
            )),
            DrainStepOutcome::RefreshFailed { pr, reason } => {
                Some(format!("bringing master into #{pr} failed: {reason}"))
            }
            DrainStepOutcome::VerificationFailed { pr, reason } => {
                Some(format!("#{pr} could not be verified after merging: {reason}"))
            }
        };
        report.steps.push(outcome);
        if let Some(why) = stop {
            report.stopped = why;
            return Ok(report);
        }
    }
    if report.stopped.is_empty() {
        report.stopped = format!("{max} merge(s), the bound asked for");
    }
    Ok(report)
}

/// The polling interval a continuous drain accepts, in seconds. The floor keeps a drain from
/// hammering the forge; the ceiling keeps it well inside [`LEASE_STALE_AFTER`], so a worker
/// waiting between cycles is never taken for a dead one.
pub const INTERVAL_SECONDS: std::ops::RangeInclusive<u64> = 30..=900;

/// What a continuous drain did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContinuousReport {
    /// Cycles run; each is a bounded [`drain`] that observes before every step.
    pub cycles: usize,
    /// Pull requests merged, in order, across every cycle.
    pub merged: Vec<u64>,
    /// Why it stopped.
    pub stopped: String,
    /// The systemic failure it stopped on, when one: the forge or git could not be read even
    /// after the bounded retries.
    pub failure: Option<String>,
}

/// How a continuous drain is bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousOptions {
    /// Merges per cycle at most.
    pub max_per_cycle: usize,
    /// Whether a cycle may bring master into a pull request.
    pub allow_refresh: bool,
    /// The wait between cycles; inside [`INTERVAL_SECONDS`].
    pub interval: Duration,
    /// Stop after this many cycles; `None` runs until stopped.
    pub cycles: Option<usize>,
}

/// Drain, wait, and drain again, until `stop` is set, a cycle bound is reached, a merge
/// could not be verified, or the repository cannot be read. Every cycle is a [`drain`], so
/// every action is preceded by a fresh observation and no plan outlives a merge; the wait
/// is taken in one-second slices so a stop is honoured within a second of the step in
/// progress finishing. `sleep` takes the waits (a test records them); `on_cycle` hears each
/// cycle's report as it ends. The caller holds the lease for the whole run.
pub fn continuous(
    root: &Path,
    integrator: &mut dyn Integrator,
    opts: ContinuousOptions,
    stop: &std::sync::atomic::AtomicBool,
    sleep: &mut dyn FnMut(Duration),
    on_cycle: &mut dyn FnMut(usize, &DrainReport),
) -> ContinuousReport {
    use std::sync::atomic::Ordering;
    let mut out = ContinuousReport {
        cycles: 0,
        merged: Vec::new(),
        stopped: String::new(),
        failure: None,
    };
    loop {
        if stop.load(Ordering::SeqCst) {
            out.stopped = "asked to stop; the step in progress finished first".into();
            break;
        }
        let report = match drain(
            root,
            integrator,
            opts.max_per_cycle,
            false,
            opts.allow_refresh,
        ) {
            Ok(r) => r,
            Err(e) => {
                out.stopped =
                    "the repository could not be read; nothing further is attempted".into();
                out.failure = Some(e);
                break;
            }
        };
        out.cycles += 1;
        out.merged.extend(report.merged.iter().copied());
        on_cycle(out.cycles, &report);
        if let Some(DrainStepOutcome::VerificationFailed { pr, reason }) = report
            .steps
            .iter()
            .find(|s| matches!(s, DrainStepOutcome::VerificationFailed { .. }))
        {
            out.stopped = format!(
                "#{pr} could not be verified after merging ({reason}); a person looks before anything else merges"
            );
            break;
        }
        if opts.cycles.is_some_and(|n| out.cycles >= n) {
            out.stopped = format!("{} cycle(s), the bound asked for", out.cycles);
            break;
        }
        let mut left = opts.interval;
        while !left.is_zero() && !stop.load(Ordering::SeqCst) {
            let slice = left.min(Duration::from_secs(1));
            sleep(slice);
            left = left.saturating_sub(slice);
        }
    }
    out
}

/// Ask this process to stop at the next safe point on SIGINT or SIGTERM: the returned flag
/// is set by the first signal, and a second one ends the process at once the way the signal
/// would have. Installed once per process; the handler does only async-signal-safe work.
pub fn stop_on_signals() -> &'static std::sync::atomic::AtomicBool {
    signals::install()
}

#[cfg(unix)]
mod signals {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Once;

    static STOP: AtomicBool = AtomicBool::new(false);
    static INSTALL: Once = Once::new();

    pub fn install() -> &'static AtomicBool {
        INSTALL.call_once(|| {
            let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            for signal in [libc::SIGTERM, libc::SIGINT] {
                // SAFETY: the handler only touches an atomic and, on a second signal, calls
                // async-signal-safe libc functions
                unsafe { libc::signal(signal, handler) };
            }
        });
        &STOP
    }

    extern "C" fn on_signal(signal: libc::c_int) {
        if STOP.swap(true, Ordering::SeqCst) {
            // the second signal: the person means it
            // SAFETY: restoring the default disposition and re-raising are async-signal-safe
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
            }
        }
    }
}

#[cfg(not(unix))]
mod signals {
    use std::sync::atomic::AtomicBool;

    static STOP: AtomicBool = AtomicBool::new(false);

    pub fn install() -> &'static AtomicBool {
        &STOP
    }
}

/// The forge and git of this checkout.
pub struct ForgeIntegrator<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// The lease held while this integrator exists; renewed on every observation.
    pub lease: Option<&'a IntegrationLease>,
}

fn gh(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("gh")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("gh could not run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

impl Integrator for ForgeIntegrator<'_> {
    fn observe(&mut self) -> Result<IntegrationQueue, String> {
        if let Some(l) = self.lease {
            l.renew();
        }
        refresh(self.root)?;
        queue_of(self.root)
    }

    fn merge(&mut self, pr: u64, head_sha: &str, method: &str) -> Result<(), String> {
        // never --admin: a refusal by the branch protection is the answer, not an obstacle.
        // Never retried either: a merge that timed out may have landed, and asking again
        // would be a second merge request; the verification after it finds out
        // (retry::forge there).
        gh(
            self.root,
            &[
                "pr",
                "merge",
                &pr.to_string(),
                &format!("--{method}"),
                "--match-head-commit",
                head_sha,
            ],
        )
        .map(|_| ())
    }

    fn verify(&mut self, pr: u64, head_sha: &str) -> Result<String, String> {
        let deadline = Instant::now() + MERGE_VISIBLE_WITHIN;
        loop {
            // a read, so asked again on an outage: the merge before it is never retried,
            // and this is what finds out whether a merge that timed out landed after all
            let state = super::retry::forge(|| {
                gh(
                    self.root,
                    &[
                        "pr",
                        "view",
                        &pr.to_string(),
                        "--json",
                        "state",
                        "--jq",
                        ".state",
                    ],
                )
            })?;
            if state.trim() == "MERGED" {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "the forge still says {} after {:?}",
                    state.trim(),
                    MERGE_VISIBLE_WITHIN
                ));
            }
            std::thread::sleep(Duration::from_secs(3));
        }
        let obs = super::load_observation(self.root)?
            .ok_or_else(|| "no observation to verify against".to_string())?;
        super::retry::forge(|| {
            let fetch = Command::new("git")
                .arg("-C")
                .arg(self.root)
                .args([
                    "fetch",
                    "--quiet",
                    "--no-tags",
                    "origin",
                    &format!("+refs/heads/{0}:refs/remotes/origin/{0}", obs.base),
                ])
                .output()
                .map_err(|e| e.to_string())?;
            if fetch.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "git fetch of the base failed after the merge: {}",
                    String::from_utf8_lossy(&fetch.stderr).trim()
                ))
            }
        })?;
        let master = local_master(self.root, &obs.base)
            .ok_or_else(|| "master is unreadable after the merge".to_string())?;
        let contained = Command::new("git")
            .arg("-C")
            .arg(self.root)
            .args(["merge-base", "--is-ancestor", head_sha, &master])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !contained {
            return Err(format!(
                "the merged head {head_sha} is not an ancestor of master {master}"
            ));
        }
        Ok(master)
    }

    fn refresh_branch(&mut self, a: &PullRequestAssessment, base: &str) -> Result<String, String> {
        let root = self.root;
        let common = common_dir(root)?;
        let dir = common
            .join("majordomus/integration")
            .join(format!("pr-{}", a.number));
        let head = a.evaluated_against.head_sha.clone();
        let master = a.evaluated_against.master_sha.clone();
        // a scratch worktree of our own: the person's checkouts are never touched
        let _ = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["worktree", "remove", "--force"])
            .arg(&dir)
            .status();
        let git_in = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        };
        let add = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["worktree", "add", "--detach"])
            .arg(&dir)
            .arg(&head)
            .output()
            .map_err(|e| e.to_string())?;
        if !add.status.success() {
            return Err(format!(
                "git worktree add: {}",
                String::from_utf8_lossy(&add.stderr).trim()
            ));
        }
        let result = (|| -> Result<String, String> {
            if let Err(e) = git_in(&["merge", "--no-commit", "--no-ff", &master]) {
                let _ = git_in(&["merge", "--abort"]);
                return Err(format!("the merge of master conflicts after all: {e}"));
            }
            // the derived artifacts of the merge result, regenerated rather than resolved
            let target = root.join("apps/majordomus-cli/target");
            let derive = Command::new(dir.join("scripts/derive"))
                .current_dir(&dir)
                .env_remove("MAJORDOMUS_SHARE")
                .env("CARGO_TARGET_DIR", &target)
                .output()
                .map_err(|e| format!("scripts/derive could not run: {e}"))?;
            if !derive.status.success() {
                let _ = git_in(&["merge", "--abort"]);
                return Err(format!(
                    "scripts/derive failed: {}",
                    String::from_utf8_lossy(&derive.stderr)
                        .lines()
                        .rev()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
            git_in(&["add", "-A"])?;
            let message = format!("Merge {base} into {} with its derived artifacts regenerated\n\nBrought in by majordomus prs drain so that the required checks run against master {master}.", a.head_ref);
            let commit = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["commit", "--no-edit", "-m", &message])
                .env_remove("MAJORDOMUS_SHARE")
                .output()
                .map_err(|e| e.to_string())?;
            if !commit.status.success() {
                let _ = git_in(&["merge", "--abort"]);
                return Err(format!(
                    "the merge commit was refused: {}",
                    String::from_utf8_lossy(&commit.stderr)
                        .lines()
                        .rev()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
            let new_head = git_in(&["rev-parse", "HEAD"])?;
            // a plain push: git refuses anything but a fast-forward of the branch, so a branch
            // that moved since it was observed is refused, never overwritten
            let push = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["push", "origin", &format!("HEAD:refs/heads/{}", a.head_ref)])
                .env_remove("MAJORDOMUS_SHARE")
                .output()
                .map_err(|e| e.to_string())?;
            if !push.status.success() {
                return Err(format!(
                    "the push was refused: {}",
                    String::from_utf8_lossy(&push.stderr).trim()
                ));
            }
            Ok(new_head)
        })();
        let _ = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["worktree", "remove", "--force"])
            .arg(&dir)
            .status();
        result
    }
}

/// What cleanup would do, or did, for one pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CleanupItem {
    /// The pull request.
    pub pr: u64,
    /// Its disposition when decided.
    pub disposition: PullRequestDisposition,
    /// The evidence, as reason codes.
    pub reasons: Vec<String>,
    /// `would_close`, `closed`, `left_for_a_person`, or `close_failed: <why>`.
    pub action: String,
}

/// Close pull requests whose work is provably on master already, and nothing else.
///
/// Only [`PullRequestDisposition::Superseded`] qualifies — the head is an ancestor of master, or
/// merging it changes no file. A pull request whose merge would change only derived
/// artifacts ([`PullRequestDisposition::PossiblyRedundant`]) is listed for a person and never closed:
/// "the generated output differs" is not proof that the authored change landed. Age,
/// shared paths and similar titles are not evidence of anything here. The queue is
/// observed afresh before closing, and each closure names the master that proved it.
pub fn cleanup(
    root: &Path,
    integrator: &mut dyn Integrator,
    apply: bool,
) -> Result<Vec<CleanupItem>, String> {
    let queue = integrator.observe()?;
    let mut items = Vec::new();
    for a in &queue.assessments {
        let action = match a.disposition {
            PullRequestDisposition::Superseded if apply => {
                let body = format!(
                    "Closed by `majordomus prs cleanup`: its work is already on `{}`.\n\nEvidence: {} (master {}, head {}).",
                    queue.base,
                    a.reasons.join(", "),
                    a.evaluated_against.master_sha,
                    a.evaluated_against.head_sha
                );
                match gh(
                    root,
                    &["pr", "close", &a.number.to_string(), "--comment", &body],
                ) {
                    Ok(_) => {
                        record(root, event("closed_superseded", Some(a), body));
                        "closed".to_string()
                    }
                    Err(e) => format!("close_failed: {e}"),
                }
            }
            PullRequestDisposition::Superseded => "would_close".into(),
            PullRequestDisposition::PossiblyRedundant => "left_for_a_person".into(),
            _ => continue,
        };
        items.push(CleanupItem {
            pr: a.number,
            disposition: a.disposition,
            reasons: a.reasons.clone(),
            action,
        });
    }
    Ok(items)
}
