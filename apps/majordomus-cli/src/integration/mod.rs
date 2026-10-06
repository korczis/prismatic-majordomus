//! Pull-request integration: the next provably safe change, onto the current master, one
//! at a time (ADR 0101).
//!
//! # The pipeline
//!
//! ```text
//! forge (gh) ──► ForgeObservation ──► relation to master (git merge-tree, cached by SHA pair)
//!                (recorded locally)            │
//!                                              ▼
//!                               classify ──► PullRequestAssessment (one per PR, with evidence)
//!                                              │
//!                                              ▼
//!                                 plan ──► IntegrationQueue (ranked, deterministic)
//!                                              │
//!           CLI · HTTP · MCP · Cockpit ◄───────┤
//!                                              ▼
//!                           drain: lease ► re-observe ► re-decide ► act ► verify ► re-plan
//! ```text
//!
//! The observation is the only network product and it is recorded with its moment; every
//! read surface renders it offline. A decision names the master and head it was taken
//! against ([`EvaluatedAgainst`]), and the executor acts only when a fresh observation
//! still says the same — a merge invalidates every earlier plan by construction, because
//! the next step starts from a new observation of a new master.
//!
//! # Why this module is not public
//!
//! It is `pub(crate)`. Integration is an operation of this executable, reached through its
//! command line (`majordomus prs`) and its capability registry (`integration.*` on HTTP, MCP
//! and OpenAPI), and not a library other crates link against, so it is not part of the
//! crate's exported API and the exported-surface gate (`quality report`) does not cover it.
//! What covers it instead: the unit tests beside it (`tests.rs`: every disposition, the
//! stale-decision refusal, the re-plan after every merge, the cleanup threshold, relations
//! computed by real git, and two properties), and
//! `test/cases/720_integration_follows_the_current_master.sh`, which drives the real
//! command line against a scripted forge.
//!
//! # Why the forge's `mergeable` is not read
//!
//! This repository resolves derived files with a per-clone merge driver the forge cannot
//! run, so the forge calls nearly every pull request conflicting. The relation to master is
//! decided here, by git, with the drivers ([`relation`]).

pub mod classify;
pub mod drain;
pub mod exclusive;
pub mod forge;
pub mod metrics;
pub mod model;
pub mod proof;
pub mod relation;
pub mod repair;
pub mod retry;
#[cfg(test)]
mod tests;
pub mod wait;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub(crate) use classify::declared_dependencies;
#[cfg(test)]
pub(crate) use classify::LabelEffect;
pub use classify::{classify, IntegrationPolicy, QueueContext, LABEL_POLICY};
pub use forge::{ForgeObservation, OBSERVATION_SCHEMA};
pub use model::*;
#[cfg(test)]
pub(crate) use relation::relation_to_master;

/// Where this checkout's integration state lives, relative to the repository root.
pub const STATE_DIR: &str = ".ai/local/state/integration";
/// Where the repository's integration state lives, relative to the common git directory:
/// what every worktree shares — the audit trail and the last queue's summary.
pub const COMMON_STATE_DIR: &str = "majordomus/integration";
/// The last forge observation.
pub const OBSERVATION_FILE: &str = "observation.json";
/// Relations already computed, keyed by `master..head` commit ids: immutable, so never
/// invalidated. Nothing but a pair of full commit ids is ever a key.
pub const RELATIONS_FILE: &str = "relations.json";
/// The audit trail of every integration action, one per repository.
pub const EVENTS_FILE: &str = "events.jsonl";
/// Left beside the repository's trail once a checkout's own trail was moved into it: the
/// move happens once, ever.
pub const TRAIL_MOVED_MARKER: &str = "events.moved-from";

/// The path of one state file of this checkout.
pub fn state_path(root: &Path, file: &str) -> PathBuf {
    root.join(STATE_DIR).join(file)
}

/// The path of one state file of the repository, under its common git directory.
pub fn common_state_path(root: &Path, file: &str) -> Result<PathBuf, String> {
    Ok(drain::common_dir(root)?.join(COMMON_STATE_DIR).join(file))
}

/// The error of a file operation, naming the file.
pub(crate) fn at<E: std::fmt::Display>(path: &Path) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{}: {e}", path.display())
}

/// The repository's audit trail. A checkout that still carries a trail of its own from
/// before the trail was the repository's (`.ai/local/state/integration/events.jsonl`) has it
/// appended to the repository's the first time this is asked while the repository has none;
/// a marker keeps that from ever happening twice. A second checkout's old trail is left
/// where it is: appended after another's, its lines would fold out of order.
pub fn events_path(root: &Path) -> Result<PathBuf, String> {
    let path = common_state_path(root, EVENTS_FILE)?;
    let old = state_path(root, EVENTS_FILE);
    if path.exists() || !old.is_file() {
        return Ok(path);
    }
    move_trail(&old, &path).map(|()| path)
}

/// Append `old` to the repository's trail at `path`, once. The marker is taken with
/// `create_new` before anything is appended, so of two processes that both find no trail,
/// one appends; and a trail removed afterwards is not refilled.
fn move_trail(old: &Path, path: &Path) -> Result<(), String> {
    let dir = path.parent().unwrap_or(path);
    let marker = dir.join(TRAIL_MOVED_MARKER);
    // a directory that cannot be made is said by the marker's open below, which then fails
    let _ = std::fs::create_dir_all(dir);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        taken => taken
            .and_then(|mut m| writeln!(m, "{}", old.display()))
            .and_then(|()| std::fs::read_to_string(old))
            .and_then(|text| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut trail| writeln!(trail, "{}", text.trim_end_matches('\n')))
            })
            .map_err(at(path)),
    }
}

/// The last recorded observation, if any.
pub fn load_observation(root: &Path) -> Result<Option<ForgeObservation>, String> {
    let path = state_path(root, OBSERVATION_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let obs: ForgeObservation =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if obs.schema != OBSERVATION_SCHEMA {
        return Err(format!(
            "{}: schema {}, this reads {OBSERVATION_SCHEMA}",
            path.display(),
            obs.schema
        ));
    }
    Ok(Some(obs))
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Record an observation.
pub fn store_observation(root: &Path, obs: &ForgeObservation) -> Result<(), String> {
    let text = serde_json::to_string_pretty(obs).map_err(|e| e.to_string())?;
    write_atomic(&state_path(root, OBSERVATION_FILE), &(text + "\n"))
}

/// The relation cache: `master..head` → relation. Both SHAs are immutable, so an entry is
/// true forever; the file is bounded by dropping entries whose master is not the current.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RelationCache {
    entries: BTreeMap<String, RelationToMaster>,
}

fn relation_cached(
    root: &Path,
    cache: &mut RelationCache,
    master: &str,
    head: &str,
) -> RelationToMaster {
    // a ref or an abbreviation may name another commit tomorrow: only a pair of commit ids
    // is decided here, so that every key of the cache is a fact forever
    if !is_object_id(master) || !is_object_id(head) {
        return RelationToMaster::Unknown {
            reason: format!("{master}..{head} is not a pair of full commit ids"),
        };
    }
    let key = format!("{master}..{head}");
    if let Some(r) = cache.entries.get(&key) {
        return r.clone();
    }
    let r = relation::relation_to_master(root, master, head);
    // an unknown is not a fact about the SHAs; it is not cached
    if !matches!(r, RelationToMaster::Unknown { .. }) {
        cache.entries.insert(key, r.clone());
    }
    r
}

/// Whether `s` is a full commit id, SHA-1 or SHA-256, as git prints one.
fn is_object_id(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The master commit this clone has for `base`, as fetched.
pub fn local_master(root: &Path, base: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/remotes/origin/{base}^{{commit}}"),
        ])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// How many pull requests are in each disposition and lane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueueTallies {
    /// Open pull requests.
    pub open: usize,
    /// By disposition word.
    pub by_disposition: BTreeMap<String, usize>,
    /// By lane word.
    pub by_lane: BTreeMap<String, usize>,
}

/// The integration queue: every open pull request assessed against one master, ranked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationQueue {
    /// `owner/name`.
    pub repository: String,
    /// The integration base.
    pub base: String,
    /// The master commit every assessment was decided against (this clone's fetched ref).
    pub master_sha: String,
    /// When the forge was observed.
    pub observed_at: String,
    /// The base commit the forge reported at that moment.
    pub observed_base_sha: String,
    /// The policy in force.
    pub policy: IntegrationPolicy,
    /// Every open pull request, in rank order: the first `ready` one is the next merge.
    pub assessments: Vec<PullRequestAssessment>,
    /// The next merge, when anything is ready.
    pub next_merge: Option<u64>,
    /// The pull requests the executor could refresh (bring master in), in rank order.
    pub next_refresh: Vec<u64>,
    /// Counts.
    pub tallies: QueueTallies,
    /// What makes this queue less than a full answer.
    pub diagnostics: Vec<String>,
    /// The actionable pull requests the executor has passed over at least
    /// [`wait::STARVING_AFTER`] times in their current wait, in rank order. Visible, never
    /// promoted: see [`wait`].
    #[serde(default)]
    pub starving: Vec<u64>,
}

impl IntegrationQueue {
    /// One assessment by number.
    pub fn get(&self, number: u64) -> Option<&PullRequestAssessment> {
        self.assessments.iter().find(|a| a.number == number)
    }
}

/// The policy of an observation: required checks and reviews as the forge reported them,
/// the label policy declared once in [`LABEL_POLICY`], and the merge method — a merge commit
/// when the repository's settings allow one (`allow_merge_commit`, which the observation
/// carries as `merge`), and none otherwise. Never a squash or a rebase: the derived-file
/// driver resolves merges, and either would replay commits it never saw. With none, every
/// pull request not already on master is held (`merge_commit_not_allowed`); one that is stays
/// `redundant`, since closing it needs no merge.
pub fn policy_of(obs: &ForgeObservation) -> IntegrationPolicy {
    let merge_method = obs
        .merge_methods
        .iter()
        .any(|m| m == "merge")
        .then(|| "merge".to_string());
    IntegrationPolicy {
        base: obs.base.clone(),
        required_checks: obs.required_checks.clone(),
        review_policy: obs.review_policy,
        skipped_permitted: Vec::new(),
        labels: LABEL_POLICY.to_vec(),
        merge_method,
        up_to_date_required: obs.up_to_date_required,
    }
}

/// The rank key: lane, then disposition, then risk, then how many other *ready or
/// refreshable* pull requests share an authored path (fewer first: landing it invalidates
/// less), then how many declared dependents wait on it (more first: landing it unblocks
/// them), then how many authored paths it changes (fewer first), then age (older first, so
/// easy new work cannot starve old work), then number. Every component is a value of the
/// assessment or of the queue around it; the order is total and input-order free.
fn rank_factors(
    a: &PullRequestAssessment,
    contenders: &BTreeSet<u64>,
    dependents: &BTreeMap<u64, usize>,
) -> RankFactors {
    RankFactors {
        lane: a.lane,
        disposition: a.disposition,
        risk: a.risk,
        contention: a
            .overlaps
            .iter()
            .filter(|o| contenders.contains(&o.number))
            .count(),
        dependents: dependents.get(&a.number).copied().unwrap_or(0),
        authored_paths: a.authored_paths.len(),
        created_at: a.created_at.clone(),
        number: a.number,
    }
}

fn rank_key(
    f: &RankFactors,
) -> (
    u8,
    u8,
    u8,
    usize,
    std::cmp::Reverse<usize>,
    usize,
    String,
    u64,
) {
    (
        f.lane as u8,
        f.disposition as u8,
        f.risk as u8,
        f.contention,
        std::cmp::Reverse(f.dependents),
        f.authored_paths,
        f.created_at.clone(),
        f.number,
    )
}

/// Rank assessments. Deterministic whatever order they arrive in; each carries the factors
/// it was ranked by.
pub fn rank(assessments: Vec<PullRequestAssessment>) -> Vec<PullRequestAssessment> {
    let contenders: BTreeSet<u64> = assessments
        .iter()
        .filter(|a| {
            matches!(
                a.disposition,
                PullRequestDisposition::Ready | PullRequestDisposition::NeedsRefresh
            )
        })
        .map(|a| a.number)
        .collect();
    // declared, unsatisfied edges into each pull request: what landing it would unblock
    let mut dependents: BTreeMap<u64, usize> = BTreeMap::new();
    for a in &assessments {
        for d in &a.dependencies {
            if d.certainty == DependencyCertainty::Confirmed && !d.satisfied {
                *dependents.entry(d.number).or_default() += 1;
            }
        }
    }
    let mut ranked: Vec<PullRequestAssessment> = assessments
        .into_iter()
        .map(|mut a| {
            a.rank_factors = Some(rank_factors(&a, &contenders, &dependents));
            a
        })
        .collect();
    ranked.sort_by_cached_key(|a| rank_key(a.rank_factors.as_ref().expect("set above")));
    ranked
}

/// Build the queue from an observation, against `master_sha`. `relation` answers what a
/// head is to master; the command line passes git, a test passes a table. It is asked about
/// every open pull request, and about every declared successor that is no longer open (a
/// pull request built from what the forge reported of it): such a successor landed exactly
/// when its head is `contained`.
pub fn build_queue(
    obs: &ForgeObservation,
    master_sha: &str,
    mut relation: impl FnMut(&PullRequestObservation) -> RelationToMaster,
) -> IntegrationQueue {
    let policy = policy_of(obs);
    let mut diagnostics = Vec::new();
    if !obs.base_sha.is_empty() && obs.base_sha != master_sha {
        diagnostics.push(format!(
            "stale observation: the forge reported {} at {} but this clone's master is {}; majordomus prs refresh",
            short(&obs.base_sha),
            obs.observed_at,
            short(master_sha)
        ));
    }
    match &policy.required_checks {
        None => diagnostics.push(
            "the branch protection or rulesets could not be read: no pull request can be ready"
                .into(),
        ),
        Some(r) if r.is_empty() => diagnostics.push(format!(
            "{} requires no check, in its protection or any ruleset: a head can prove nothing \
             to it, so no pull request can be ready (owner decision D5)",
            policy.base
        )),
        Some(_) => {}
    }
    if policy.merge_method.is_none() {
        diagnostics.push(format!(
            "{} allows no merge commit (it allows: {}): the executor never squashes or rebases, \
             so no pull request can be ready",
            obs.repository,
            if obs.merge_methods.is_empty() {
                "nothing".to_string()
            } else {
                obs.merge_methods.join(", ")
            }
        ));
    }
    // in number order, whatever order the forge listed them in
    let armed: BTreeSet<u64> = obs
        .pull_requests
        .iter()
        .filter(|p| p.auto_merge)
        .map(|p| p.number)
        .collect();
    if !armed.is_empty() {
        diagnostics.push(format!(
            "auto-merge is armed on {}: the forge would merge them on its own, outside the \
             executor, so they are unsafe until it is disarmed (gh pr merge <n> --disable-auto)",
            armed
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    let relations: Vec<RelationToMaster> = obs.pull_requests.iter().map(&mut relation).collect();
    let mut queue = QueueContext {
        open: obs.pull_requests.iter().map(|p| p.number).collect(),
        // a fork's branch is not a branch of this repository, whatever it is called: a fork
        // named `master` would otherwise stack every pull request onto itself
        heads: obs
            .pull_requests
            .iter()
            .filter(|p| !p.cross_repository)
            .map(|p| (p.head_ref.clone(), p.number))
            .collect(),
        authored: BTreeMap::new(),
        superseded_by: BTreeMap::new(),
    };
    queue.superseded_by = successors(obs, &queue.open, &mut relation);
    for (p, r) in obs.pull_requests.iter().zip(&relations) {
        let authored = match r {
            RelationToMaster::UpToDate { authored } | RelationToMaster::Behind { authored, .. } => {
                authored.clone()
            }
            RelationToMaster::Conflicting { paths } => paths.clone(),
            _ => Vec::new(),
        };
        queue.authored.insert(p.number, authored);
    }
    let assessments: Vec<PullRequestAssessment> = obs
        .pull_requests
        .iter()
        .zip(&relations)
        .map(|(p, r)| classify(p, r, master_sha, &obs.observed_at, &policy, &queue))
        .collect();
    let mut queue = IntegrationQueue {
        repository: obs.repository.clone(),
        base: obs.base.clone(),
        master_sha: master_sha.to_string(),
        observed_at: obs.observed_at.clone(),
        observed_base_sha: obs.base_sha.clone(),
        next_merge: None,
        next_refresh: Vec::new(),
        policy,
        assessments: rank(assessments),
        tallies: QueueTallies::default(),
        diagnostics,
        starving: Vec::new(),
    };
    derive_heads(&mut queue);
    queue
}

/// What the queue's ranked assessments imply: the next merge, the refresh candidates and the
/// counts. Derived again whenever a disposition changes after the queue was built.
fn derive_heads(queue: &mut IntegrationQueue) {
    let mut tallies = QueueTallies {
        open: queue.assessments.len(),
        ..Default::default()
    };
    for a in &queue.assessments {
        *tallies
            .by_disposition
            .entry(a.disposition.as_str().to_string())
            .or_default() += 1;
        *tallies.by_lane.entry(classify::word(&a.lane)).or_default() += 1;
    }
    queue.tallies = tallies;
    queue.next_merge = queue
        .assessments
        .iter()
        .find(|a| a.disposition == PullRequestDisposition::Ready)
        .map(|a| a.number);
    queue.next_refresh = queue
        .assessments
        .iter()
        .filter(|a| a.disposition == PullRequestDisposition::NeedsRefresh)
        .map(|a| a.number)
        .collect();
}

/// The executor's own refusals, folded from the trail: a candidate the forge refused to
/// merge, or whose refresh failed, at the head and master it still has, is held back as
/// `needs_repair` instead of being chosen again, so one refused change does not block the
/// queue. A new head or a new master clears it, because the key no longer matches; a
/// transient failure never holds anything back, because it says nothing about the change.
/// No plan survives: the trail is the only memory, and every queue is folded from it again.
pub fn executor_feedback(queue: &mut IntegrationQueue, trail: &[drain::IntegrationEvent]) {
    use drain::{FailureClass, IntegrationAction};
    let refused = |action: IntegrationAction| -> BTreeSet<(u64, String, String)> {
        trail
            .iter()
            .filter(|e| e.action == action && e.class != Some(FailureClass::Transient))
            .filter_map(|e| Some((e.pr?, e.head_sha.clone()?, e.master_before.clone()?)))
            .collect()
    };
    let merges = refused(IntegrationAction::MergeFailed);
    let refreshes = refused(IntegrationAction::RefreshFailed);
    let master = queue.master_sha.clone();
    let mut changed = false;
    for a in &mut queue.assessments {
        let key = (
            a.number,
            a.evaluated_against.head_sha.clone(),
            master.clone(),
        );
        let held = match a.disposition {
            PullRequestDisposition::Ready if merges.contains(&key) => {
                Some(ReasonCode::ExecutorMergeRefused {
                    head: key.1.clone(),
                })
            }
            PullRequestDisposition::NeedsRefresh if refreshes.contains(&key) => {
                Some(ReasonCode::ExecutorRefreshFailed {
                    master: master.clone(),
                })
            }
            _ => None,
        };
        if let Some(reason) = held {
            a.disposition = PullRequestDisposition::NeedsRepair;
            a.lane = a.disposition.lane();
            a.reasons.insert(0, reason);
            changed = true;
        }
    }
    if changed {
        queue.assessments = rank(std::mem::take(&mut queue.assessments));
        derive_heads(queue);
    }
}

/// The declared successors of every open pull request, each with what became of it: open; or
/// no longer open and landed when `relation` finds its head contained in master; or not landed;
/// or unread, when the forge did not report it or git could not answer.
fn successors(
    obs: &ForgeObservation,
    open: &BTreeSet<u64>,
    relation: &mut impl FnMut(&PullRequestObservation) -> RelationToMaster,
) -> BTreeMap<u64, Vec<classify::Successor>> {
    use classify::{declared_supersessions, Successor, SuccessorState};
    // replaced → successor → whose body said so (the first to)
    let mut declared: BTreeMap<u64, BTreeMap<u64, u64>> = BTreeMap::new();
    let mut declare = |replaced: u64, successor: u64, by: u64| {
        if replaced != successor && open.contains(&replaced) {
            declared
                .entry(replaced)
                .or_default()
                .entry(successor)
                .or_insert(by);
        }
    };
    for p in &obs.pull_requests {
        let s = declared_supersessions(&p.body);
        for m in s.superseded_by {
            declare(p.number, m, p.number);
        }
        for n in s.supersedes {
            declare(n, p.number, p.number);
        }
    }
    for (m, r) in &obs.resolved {
        for n in declared_supersessions(&r.body).supersedes {
            declare(n, *m, *m);
        }
    }
    let mut state_of = |m: u64| -> SuccessorState {
        if open.contains(&m) {
            return SuccessorState::Open;
        }
        let Some(r) = obs.resolved.get(&m) else {
            return SuccessorState::Unread;
        };
        let head = PullRequestObservation {
            number: m,
            title: String::new(),
            author: String::new(),
            head_ref: String::new(),
            head_sha: r.head_sha.clone(),
            base_ref: obs.base.clone(),
            draft: false,
            labels: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
            body: r.body.clone(),
            checks: Vec::new(),
            review_decision: String::new(),
            auto_merge: false,
            cross_repository: false,
            latest_reviews: Vec::new(),
            review_requests: Vec::new(),
        };
        match relation(&head) {
            RelationToMaster::Contained => SuccessorState::Landed {
                head_sha: r.head_sha.clone(),
                merged: r.merged,
            },
            RelationToMaster::Unknown { .. } => SuccessorState::Unread,
            _ => SuccessorState::NotLanded { merged: r.merged },
        }
    };
    let mut states: BTreeMap<u64, SuccessorState> = BTreeMap::new();
    declared
        .into_iter()
        .map(|(n, by)| {
            let list = by
                .into_iter()
                .map(|(m, declared_in)| Successor {
                    number: m,
                    declared_in,
                    state: states.entry(m).or_insert_with(|| state_of(m)).clone(),
                })
                .collect();
            (n, list)
        })
        .collect()
}

/// The queue of this checkout, from the last recorded observation, with relations decided
/// by git against this clone's fetched master. Offline: it reads the network's last answer
/// and never asks it again.
pub fn queue_of(root: &Path) -> Result<IntegrationQueue, String> {
    computed(root, std::time::SystemTime::now()).map(|(queue, _)| queue)
}

/// [`queue_of`], and what it learnt kept for the next reader: the relations decided (the
/// cache, by commit pair) and the summary a briefing reads without deciding one. Only the
/// paths that just observed the forge write — `prs refresh` and the executor — so every
/// read, from the command line, the capability or the Cockpit, leaves the checkout as it was.
pub fn queue_and_record(root: &Path) -> Result<IntegrationQueue, String> {
    let (queue, cache) = computed(root, std::time::SystemTime::now())?;
    // both are caches a later read rebuilds: a write that fails costs that rebuild, never this
    // answer, so neither failure is the caller's
    let _ = serde_json::to_string(&cache)
        .ok()
        .map(|text| write_atomic(&state_path(root, RELATIONS_FILE), &text));
    // the summary a briefing reads without deciding a single relation (QueueSummary), the
    // repository's like the trail it sits beside
    let _ = common_state_path(root, SUMMARY_FILE)
        .ok()
        .zip(serde_json::to_string_pretty(&QueueSummary::of(&queue)).ok())
        .map(|(path, text)| write_atomic(&path, &(text + "\n")));
    Ok(queue)
}

/// An observation older than this describes a forge that has moved on: the queue built from
/// it is said to be stale, and nothing reading it may take it for the present.
pub const OBSERVATION_STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(3600);

/// The queue of the recorded observation, and the relation cache as it stands after deciding
/// it. Writes nothing.
/// Name each assessment's issue and milestone: the issue its head branch names among
/// `issues` (id → the milestone its record declares), by
/// [`crate::worktree::state::issue_of`]. Neither decides a disposition or a rank: they say
/// what the change is part of.
pub fn link_issues(queue: &mut IntegrationQueue, issues: &BTreeMap<String, Option<String>>) {
    let ids: Vec<String> = issues.keys().cloned().collect();
    for a in &mut queue.assessments {
        a.issue = crate::worktree::state::issue_of(&a.head_ref, &ids);
        a.milestone = a
            .issue
            .as_ref()
            .and_then(|i| issues.get(i).cloned().flatten());
    }
}

/// The issues this checkout's project model declares, each with the milestone its record
/// names: [`crate::worktree::state::issue_ids`], each record read with the layer's own YAML
/// reader. A record that does not parse names no milestone; the issue still links.
pub fn issue_milestones(root: &Path) -> BTreeMap<String, Option<String>> {
    crate::worktree::state::issue_ids(root)
        .into_iter()
        .map(|id| {
            let milestone = std::fs::read_to_string(
                root.join(".ai/repo/project/issues")
                    .join(format!("{id}.yaml")),
            )
            .ok()
            .and_then(|t| crate::metadata::yaml::parse_mapping(&t).ok())
            .and_then(|m| {
                m.get("milestone")
                    .and_then(crate::metadata::yaml::scalar_string)
            })
            .filter(|m| !m.is_empty());
            (id, milestone)
        })
        .collect()
}

fn computed(
    root: &Path,
    now: std::time::SystemTime,
) -> Result<(IntegrationQueue, RelationCache), String> {
    let obs = load_observation(root)?.ok_or_else(|| {
        "no forge observation is recorded in this checkout; majordomus prs refresh".to_string()
    })?;
    let master = local_master(root, &obs.base).ok_or_else(|| {
        format!(
            "this clone has no refs/remotes/origin/{}; majordomus prs refresh fetches it",
            obs.base
        )
    })?;
    let cache_path = state_path(root, RELATIONS_FILE);
    let mut cache: RelationCache = std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // bounded: only the current master's entries are worth keeping, and only a pair of
    // commit ids is a fact — a key written before refs were refused names a ref, which may
    // hold another commit tomorrow, and is dropped even while master stands still
    cache.entries.retain(|k, _| {
        k.split_once("..")
            .is_some_and(|(m, h)| m == master && is_object_id(m) && is_object_id(h))
    });
    let mut queue = build_queue(&obs, &master, |p| {
        // decided on exactly the head the forge reported, which the assessment names as
        // evaluated: a head that moved during the refresh is not in this clone, and what
        // the fetched ref holds now is another head nobody observed
        if !relation::has_commit(root, &p.head_sha) {
            return RelationToMaster::Unknown {
                reason: format!(
                    "the observed head {} is not fetched; the pull request moved during the refresh — majordomus prs refresh",
                    p.head_sha
                ),
            };
        }
        relation_cached(root, &mut cache, &master, &p.head_sha)
    });
    link_issues(&mut queue, &issue_milestones(root));
    let now_secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    if let Some(age) = crate::peers::epoch_seconds(&obs.observed_at)
        .map(|then| now_secs.saturating_sub(then))
        .filter(|age| *age > i64::try_from(OBSERVATION_STALE_AFTER.as_secs()).unwrap_or(i64::MAX))
    {
        queue.diagnostics.push(format!(
            "the observation is {} min old (observed {}): the forge has moved on since; majordomus prs refresh",
            age / 60,
            obs.observed_at
        ));
    }
    let trail = drain::events(root);
    executor_feedback(&mut queue, &trail);
    wait::annotate(&mut queue, &trail);
    Ok((queue, cache))
}

/// The last queue built in the repository, summarised.
pub const SUMMARY_FILE: &str = "summary.json";

/// What the last queue built said, in a few numbers: what a session briefing prints without
/// building a queue, which would decide every relation again. Written by [`queue_of`] every
/// time it builds one, so it is exactly as current as the last read of the queue, and says
/// when that was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueueSummary {
    /// The integration base.
    pub base: String,
    /// The master commit the queue was decided against.
    pub master_sha: String,
    /// When the forge was observed.
    pub observed_at: String,
    /// When the queue was built, RFC 3339.
    pub built_at: String,
    /// The counts.
    pub tallies: QueueTallies,
    /// The next merge, when anything was ready.
    pub next_merge: Option<u64>,
    /// The starving pull requests.
    pub starving: Vec<u64>,
    /// Whether the queue carried diagnostics (a stale observation first).
    pub diagnostics: usize,
}

impl QueueSummary {
    /// The summary of a queue, built now.
    pub fn of(q: &IntegrationQueue) -> Self {
        QueueSummary {
            base: q.base.clone(),
            master_sha: q.master_sha.clone(),
            observed_at: q.observed_at.clone(),
            built_at: crate::peers::rfc3339(std::time::SystemTime::now()),
            tallies: q.tallies.clone(),
            next_merge: q.next_merge,
            starving: q.starving.clone(),
            diagnostics: q.diagnostics.len(),
        }
    }

    /// The summary recorded in the repository, if any.
    pub fn load(root: &Path) -> Option<Self> {
        std::fs::read_to_string(common_state_path(root, SUMMARY_FILE).ok()?)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    }
}

/// Observe the forge, fetch what it names, and record the observation — in the checkout's
/// state, and as an `observed` line of the repository's trail. The network step; the only
/// one outside the executor.
pub fn refresh(root: &Path) -> Result<ForgeObservation, String> {
    use forge::Forge;
    let obs = forge::GhForge { root }.observe().map_err(|e| e.0)?;
    forge::fetch(root, &obs).map_err(|e| e.0)?;
    store_observation(root, &obs)?;
    drain::record(
        root,
        drain::IntegrationEvent {
            master_before: Some(obs.base_sha.clone()),
            detail: format!(
                "{} open pull request(s) of {} at {}",
                obs.pull_requests.len(),
                obs.repository,
                obs.observed_at
            ),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::Observed)
        },
    )?;
    Ok(obs)
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

#[cfg(test)]
mod queue_branches {
    //! The queue's less common inputs: trail events that name too little, a successor nobody
    //! could read, and a recording read with nothing observed.

    use super::*;

    fn observation(
        body: &str,
        resolved: BTreeMap<u64, forge::ResolvedPullRequest>,
    ) -> ForgeObservation {
        let pr = forge::pull_request_of(&serde_json::json!({
            "number": 1, "title": "t", "author": {"login": "a"}, "headRefName": "fix/1",
            "headRefOid": "h1", "baseRefName": "master", "isDraft": false, "labels": [],
            "createdAt": "2026-10-01T00:00:00Z", "updatedAt": "2026-10-01T00:00:00Z",
            "body": body, "statusCheckRollup": [], "reviewDecision": "",
            "autoMergeRequest": null, "isCrossRepository": false
        }))
        .unwrap();
        ForgeObservation {
            schema: OBSERVATION_SCHEMA,
            repository: "o/r".into(),
            base: "master".into(),
            base_sha: "m".into(),
            observed_at: "t".into(),
            required_checks: None,
            review_policy: None,
            up_to_date_required: None,
            merge_methods: vec!["merge".into()],
            pull_requests: vec![pr],
            resolved,
            delete_branch_on_merge: None,
        }
    }

    fn unknown(_: &PullRequestObservation) -> RelationToMaster {
        RelationToMaster::Unknown {
            reason: "not fetched".into(),
        }
    }

    #[test]
    fn a_failure_that_names_too_little_holds_nothing_back() {
        let mut q = build_queue(&observation("", BTreeMap::new()), "m", unknown);
        let before = q.clone();
        let event =
            |pr: Option<u64>, head: Option<&str>, master: Option<&str>| drain::IntegrationEvent {
                pr,
                head_sha: head.map(str::to_string),
                master_before: master.map(str::to_string),
                ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeFailed)
            };
        let trail = vec![
            event(None, Some("h1"), Some("m")),
            event(Some(1), None, Some("m")),
            event(Some(1), Some("h1"), None),
        ];
        executor_feedback(&mut q, &trail);
        assert_eq!(q, before);
    }

    #[test]
    fn a_successor_whose_head_could_not_be_read_is_unread() {
        let resolved = BTreeMap::from([(
            2,
            forge::ResolvedPullRequest {
                merged: true,
                head_sha: "h2".into(),
                body: String::new(),
            },
        )]);
        let q = build_queue(&observation("Superseded by #2", resolved), "m", unknown);
        let a = q.get(1).unwrap();
        assert!(
            a.reasons.iter().any(|r| *r == "successor_unread:#2"),
            "{:?}",
            a.reasons
        );
    }

    #[test]
    fn a_recording_read_with_nothing_observed_refuses() {
        let dir = tempfile::tempdir().unwrap();
        assert!(queue_and_record(dir.path()).is_err());
    }
}

/// Tests only: a queue with one open pull request per head branch, for the surfaces that
/// render an assessment.
#[cfg(test)]
pub(crate) fn issue_test_queue(branches: &[&str]) -> IntegrationQueue {
    issue_tests::queue_with(branches)
}

#[cfg(test)]
mod issue_tests {
    //! Each assessment names the issue its branch names and that issue's milestone (WP24).

    use super::*;

    /// The queue of one observation with an open pull request per head branch.
    pub(crate) fn queue_with(branches: &[&str]) -> IntegrationQueue {
        let pull_requests = branches
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                forge::pull_request_of(&serde_json::json!({
                    "number": i + 1, "title": format!("change {}", i + 1),
                    "author": {"login": "someone"}, "headRefName": b,
                    "headRefOid": format!("{:040}", i + 1), "baseRefName": "master",
                    "isDraft": false, "labels": [], "createdAt": "2026-10-01T00:00:00Z",
                    "updatedAt": "2026-10-01T00:00:00Z", "body": "",
                    "statusCheckRollup": [], "reviewDecision": "", "autoMergeRequest": null,
                    "isCrossRepository": false
                }))
            })
            .collect();
        let obs = ForgeObservation {
            schema: OBSERVATION_SCHEMA,
            repository: "o/r".into(),
            base: "master".into(),
            base_sha: "m".into(),
            observed_at: "t".into(),
            required_checks: None,
            review_policy: None,
            up_to_date_required: None,
            merge_methods: vec!["merge".into()],
            pull_requests,
            resolved: Default::default(),
            delete_branch_on_merge: None,
        };
        build_queue(&obs, "m", |_| RelationToMaster::Unknown {
            reason: "not asked".into(),
        })
    }

    #[test]
    fn the_branch_names_the_issue_and_its_record_the_milestone() {
        let mut q = queue_with(&["feature/I0810-manifest", "fix/I0901", "fix/plain", "I0700x"]);
        let issues = BTreeMap::from([
            ("I0810".to_string(), Some("M003".to_string())),
            ("I0901".to_string(), None),
            ("I0700".to_string(), Some("M001".to_string())),
        ]);
        link_issues(&mut q, &issues);
        let got: Vec<(u64, Option<&str>, Option<&str>)> = {
            let mut v: Vec<_> = q
                .assessments
                .iter()
                .map(|a| (a.number, a.issue.as_deref(), a.milestone.as_deref()))
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            got,
            [
                (1, Some("I0810"), Some("M003")),
                (2, Some("I0901"), None),
                (3, None, None),
                (4, None, None),
            ],
            "a component only begins with an id when the id is followed by '-'"
        );
    }

    #[test]
    fn the_milestone_is_read_from_the_record_and_a_broken_record_names_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(issue_milestones(dir.path()).is_empty(), "no project model");
        let issues = dir.path().join(".ai/repo/project/issues");
        std::fs::create_dir_all(&issues).unwrap();
        std::fs::write(issues.join("I0001.yaml"), "id: I0001\nmilestone: M001\n").unwrap();
        std::fs::write(issues.join("I0002.yaml"), "id: I0002\n").unwrap();
        std::fs::write(issues.join("I0003.yaml"), "id: [unclosed\n").unwrap();
        std::fs::write(issues.join("I0004.yaml"), "id: I0004\nmilestone: \"\"\n").unwrap();
        std::fs::write(issues.join("README.md"), "not an issue\n").unwrap();
        let got = issue_milestones(dir.path());
        assert_eq!(
            got,
            BTreeMap::from([
                ("I0001".to_string(), Some("M001".to_string())),
                ("I0002".to_string(), None),
                ("I0003".to_string(), None),
                ("I0004".to_string(), None),
            ])
        );
    }
}
