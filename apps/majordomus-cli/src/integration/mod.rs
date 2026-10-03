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
pub mod forge;
pub mod model;
pub mod relation;
pub mod retry;
#[cfg(test)]
mod tests;
pub mod wait;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub(crate) use classify::declared_dependencies;
pub use classify::{classify, IntegrationPolicy, QueueContext, BLOCKING_LABELS};
pub use forge::{ForgeObservation, OBSERVATION_SCHEMA, PR_REF_PREFIX};
pub use model::*;
#[cfg(test)]
pub(crate) use relation::relation_to_master;

/// Where this checkout's integration state lives, relative to the repository root.
pub const STATE_DIR: &str = ".ai/local/state/integration";
/// The last forge observation.
pub const OBSERVATION_FILE: &str = "observation.json";
/// Relations already computed, keyed by `master..head`: immutable, so never invalidated.
pub const RELATIONS_FILE: &str = "relations.json";
/// The audit trail of every integration action.
pub const EVENTS_FILE: &str = "events.jsonl";

/// The path of one state file.
pub fn state_path(root: &Path, file: &str) -> PathBuf {
    root.join(STATE_DIR).join(file)
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
/// the blocking labels declared once in [`BLOCKING_LABELS`], and the merge method the
/// repository allows — a merge commit when it allows one, because the derived-file driver
/// resolves merges and a squash or rebase would replay commits it never saw.
pub fn policy_of(obs: &ForgeObservation) -> IntegrationPolicy {
    let merge_method = ["merge", "squash", "rebase"]
        .iter()
        .find(|m| obs.merge_methods.iter().any(|x| x == *m))
        .unwrap_or(&"merge")
        .to_string();
    IntegrationPolicy {
        base: obs.base.clone(),
        required_checks: obs.required_checks.clone(),
        reviews_required: obs.reviews_required,
        blocking_labels: BLOCKING_LABELS.iter().map(|s| s.to_string()).collect(),
        merge_method,
    }
}

/// The rank key: lane, then disposition, then risk, then how many other *ready or
/// refreshable* pull requests share an authored path (fewer first: landing it invalidates
/// less), then age (older first, so easy new work cannot starve old work), then number.
/// Every component is a value of the assessment; the order is total and input-order free.
fn rank_key(
    a: &PullRequestAssessment,
    contenders: &BTreeSet<u64>,
) -> (u8, u8, u8, usize, String, u64) {
    let lane = a.lane as u8;
    let disposition = a.disposition as u8;
    let risk = a.risk as u8;
    let contention = a
        .overlaps
        .iter()
        .filter(|o| contenders.contains(&o.number))
        .count();
    (
        lane,
        disposition,
        risk,
        contention,
        a.created_at.clone(),
        a.number,
    )
}

/// Rank assessments. Deterministic whatever order they arrive in.
pub fn rank(mut assessments: Vec<PullRequestAssessment>) -> Vec<PullRequestAssessment> {
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
    assessments.sort_by_cached_key(|a| rank_key(a, &contenders));
    assessments
}

/// Build the queue from an observation, against `master_sha`. `relation` answers what a
/// head is to master; the command line passes git, a test passes a table.
pub fn build_queue(
    obs: &ForgeObservation,
    master_sha: &str,
    relation: impl FnMut(&PullRequestObservation) -> RelationToMaster,
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
    if policy.required_checks.is_none() {
        diagnostics
            .push("the branch protection could not be read: no pull request can be ready".into());
    }
    let relations: Vec<RelationToMaster> = obs.pull_requests.iter().map(relation).collect();
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
    };
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
        .map(|(p, r)| classify(p, r, master_sha, &policy, &queue))
        .collect();
    let assessments = rank(assessments);
    let mut tallies = QueueTallies {
        open: assessments.len(),
        ..Default::default()
    };
    for a in &assessments {
        *tallies
            .by_disposition
            .entry(a.disposition.as_str().to_string())
            .or_default() += 1;
        *tallies.by_lane.entry(classify::word(&a.lane)).or_default() += 1;
    }
    IntegrationQueue {
        repository: obs.repository.clone(),
        base: obs.base.clone(),
        master_sha: master_sha.to_string(),
        observed_at: obs.observed_at.clone(),
        observed_base_sha: obs.base_sha.clone(),
        next_merge: assessments
            .iter()
            .find(|a| a.disposition == PullRequestDisposition::Ready)
            .map(|a| a.number),
        next_refresh: assessments
            .iter()
            .filter(|a| a.disposition == PullRequestDisposition::NeedsRefresh)
            .map(|a| a.number)
            .collect(),
        policy,
        assessments,
        tallies,
        diagnostics,
        starving: Vec::new(),
    }
}

/// The queue of this checkout, from the last recorded observation, with relations decided
/// by git against this clone's fetched master. Offline: it reads the network's last answer
/// and never asks it again.
pub fn queue_of(root: &Path) -> Result<IntegrationQueue, String> {
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
    // bounded: only the current master's entries are worth keeping
    cache
        .entries
        .retain(|k, _| k.starts_with(&format!("{master}..")));
    let mut queue = build_queue(&obs, &master, |p| {
        let head = format!("{PR_REF_PREFIX}{}", p.number);
        // the fetched ref must still be the observed head: a head that moved since is
        // decided against the SHA the forge reported, which the fetch brought in
        let sha = if relation::has_commit(root, &p.head_sha) {
            p.head_sha.clone()
        } else {
            head
        };
        relation_cached(root, &mut cache, &master, &sha)
    });
    if let Ok(text) = serde_json::to_string(&cache) {
        let _ = write_atomic(&cache_path, &text);
    }
    wait::annotate(&mut queue, &drain::events(root));
    // the summary a briefing reads without deciding a single relation (QueueSummary)
    if let Ok(text) = serde_json::to_string_pretty(&QueueSummary::of(&queue)) {
        let _ = write_atomic(&state_path(root, SUMMARY_FILE), &(text + "\n"));
    }
    Ok(queue)
}

/// The last queue built in this checkout, summarised.
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

    /// The summary recorded in this checkout, if any.
    pub fn load(root: &Path) -> Option<Self> {
        std::fs::read_to_string(state_path(root, SUMMARY_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    }
}

/// Observe the forge, fetch what it names, and record the observation. The network step;
/// the only one outside the executor.
pub fn refresh(root: &Path) -> Result<ForgeObservation, String> {
    use forge::Forge;
    let obs = forge::GhForge { root }.observe().map_err(|e| e.0)?;
    forge::fetch(root, &obs).map_err(|e| e.0)?;
    store_observation(root, &obs)?;
    Ok(obs)
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}
