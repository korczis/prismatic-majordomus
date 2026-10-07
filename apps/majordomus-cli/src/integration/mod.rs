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
///
/// A record of another schema is refused, older or newer, and never read as empty or as
/// current: its schema is read before anything else of it, so a record a newer executable
/// wrote is refused by its number and not by whatever field this one cannot parse. The
/// message names both schemas and the one command that records an observation this reads.
pub fn load_observation(root: &Path) -> Result<Option<ForgeObservation>, String> {
    let path = state_path(root, OBSERVATION_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let at = |e: serde_json::Error| format!("{}: {e}", path.display());
    let record: serde_json::Value = serde_json::from_str(&text).map_err(at)?;
    let other = record
        .get("schema")
        .and_then(serde_json::Value::as_u64)
        .filter(|schema| *schema != u64::from(OBSERVATION_SCHEMA));
    match other {
        Some(schema) => Err(format!(
            "{}: schema {schema}, this reads {OBSERVATION_SCHEMA}; majordomus prs refresh \
             records an observation this reads",
            path.display()
        )),
        // one that names no schema is no observation: the fields say so
        None => serde_json::from_value(record).map(Some).map_err(at),
    }
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

/// The relation cache: `master..head` → relation, and beside it under the same key what the
/// merge changes by kind ([`ChangeShape`]). Both SHAs are immutable, and both answers read
/// derived attributes from master's own `.gitattributes`, so an entry is true forever and a
/// change of either commit stales both together; the file is bounded by dropping entries
/// whose master is not the current.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RelationCache {
    entries: BTreeMap<String, RelationToMaster>,
    #[serde(default)]
    shapes: BTreeMap<String, ChangeShape>,
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

/// [`relation::change_shape`] through the cache, under exactly the relation's key: only a pair
/// of full commit ids is decided, and an unanswered shape is not cached.
fn shape_cached(
    root: &Path,
    cache: &mut RelationCache,
    master: &str,
    head: &str,
) -> Option<ChangeShape> {
    let key = format!("{master}..{head}");
    (is_object_id(master) && is_object_id(head))
        .then(|| {
            cache.shapes.get(&key).cloned().or_else(|| {
                relation::change_shape(root, master, head)
                    .inspect(|s| drop(cache.shapes.insert(key.clone(), s.clone())))
            })
        })
        .flatten()
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
/// refreshable* pull requests it overlaps (fewer first: landing it invalidates less), then how many declared dependents wait on it (more first: landing it unblocks
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

/// [`build_queue_shaped`] with no shape answered: what a test with a table of relations builds.
#[cfg(test)]
pub fn build_queue(
    obs: &ForgeObservation,
    master_sha: &str,
    relation: impl FnMut(&PullRequestObservation) -> RelationToMaster,
) -> IntegrationQueue {
    build_queue_shaped(obs, master_sha, relation, |_| None)
}

/// Build the queue from an observation, against `master_sha`. `relation` answers what a
/// head is to master; the command line passes git, a test passes a table. It is asked about
/// every open pull request, and about every authorised declared successor that is no longer
/// open (a pull request built from what the forge reported of it, at its head and then, when
/// the head is not on master, at its merge commit): such a successor landed exactly when one
/// of the two is `contained`, whatever the forge calls it. A declaration nobody entitled made
/// asks nothing of it. `shape` answers what each open pull request's merge changes
/// by kind ([`ChangeShape`]): its authored paths decide overlaps and risk, and two that raise
/// the version or change a release overlap as such. A pull request it does not answer is
/// assessed on its relation's paths alone.
pub fn build_queue_shaped(
    obs: &ForgeObservation,
    master_sha: &str,
    mut relation: impl FnMut(&PullRequestObservation) -> RelationToMaster,
    shape: impl FnMut(&PullRequestObservation) -> Option<ChangeShape>,
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
        // a context bound to an app whose check runs were not read with their writer
        Some(required) => {
            diagnostics.extend(classify::unread_writers(&obs.pull_requests, required))
        }
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
        shapes: obs
            .pull_requests
            .iter()
            .map(shape)
            .zip(&obs.pull_requests)
            .filter_map(|(s, p)| s.map(|s| (p.number, s)))
            .collect(),
        superseded_by: BTreeMap::new(),
        possible_supersessions: BTreeMap::new(),
        references_unread: BTreeMap::new(),
        dependency_states: obs
            .resolved
            .iter()
            .map(|(n, r)| {
                let state = if r.merged {
                    DependencyState::Merged
                } else {
                    DependencyState::ClosedUnmerged
                };
                (*n, state)
            })
            .collect(),
        cycles: BTreeMap::new(),
    };
    queue.cycles = dependency_cycles(obs, &queue.heads);
    if obs.pull_requests.len() >= forge::OPEN_LIMIT {
        diagnostics.push(format!(
            "the forge listed {} open pull requests, its limit: more may be open; a dependency on one of those reads as unread, and a listed one that one of those says it supersedes is held as unread",
            obs.pull_requests.len()
        ));
    }
    let declared = successors(obs, &queue.open, &mut relation);
    queue.superseded_by = declared.authorised;
    queue.possible_supersessions = declared.possible;
    // a pull request whose cross-references were not read whole is held, and the queue says
    // which and why: a declaration that was not read is never taken for none
    queue.references_unread = obs
        .pull_requests
        .iter()
        .filter(|p| p.cross_references != CrossReferenceRead::Whole)
        .map(|p| (p.number, p.cross_references))
        .collect();
    diagnostics.extend(references_unread_diagnostic(&queue.references_unread));
    for (p, r) in obs.pull_requests.iter().zip(&relations) {
        let authored = match (queue.shapes.get(&p.number), r) {
            (Some(s), _) => s.authored.clone(),
            (
                None,
                RelationToMaster::UpToDate { authored } | RelationToMaster::Behind { authored, .. },
            ) => authored.clone(),
            (None, RelationToMaster::Conflicting { paths }) => paths.clone(),
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

/// The open pull requests caught in a cycle of confirmed dependencies
/// ([`classify::confirmed_dependencies`], between open pull requests only), each with the
/// others it can reach and that reach it. A pull request on such a cycle waits for itself,
/// so nothing about the others can let it land first. The module is private, so the
/// example is text; `dependency_queue` runs the same assertions.
///
/// ```text
/// use majordomus_cli::integration::{forge, ForgeObservation, OBSERVATION_SCHEMA};
/// let pr = |n: u64, body: &str| forge::pull_request_of(&serde_json::json!({
///     "number": n, "title": "t", "author": {"login": "a"}, "headRefName": format!("f/{n}"),
///     "headRefOid": format!("{n:040}"), "baseRefName": "master", "isDraft": false,
///     "labels": [], "createdAt": "t", "updatedAt": "t", "body": body,
///     "statusCheckRollup": [], "reviewDecision": "", "autoMergeRequest": null,
///     "isCrossRepository": false})).unwrap();
/// let mut obs: ForgeObservation = serde_json::from_value(serde_json::json!({
///     "schema": OBSERVATION_SCHEMA, "repository": "o/r", "base": "master", "base_sha": "m",
///     "observed_at": "t", "required_checks": null, "review_policy": null,
///     "merge_methods": ["merge"], "pull_requests": []})).unwrap();
/// obs.pull_requests = vec![pr(1, "Depends on #2"), pr(2, "Depends on #1"), pr(3, "Depends on #1")];
/// let cycles = majordomus_cli::integration::dependency_cycles(&obs, &Default::default());
/// assert_eq!(cycles.get(&1), Some(&vec![2]));
/// assert!(cycles.get(&3).is_none(), "waiting on a cycle is not being in one");
/// ```
pub fn dependency_cycles(
    obs: &ForgeObservation,
    heads: &BTreeMap<String, u64>,
) -> BTreeMap<u64, Vec<u64>> {
    let open: BTreeSet<u64> = obs.pull_requests.iter().map(|p| p.number).collect();
    let edges: BTreeMap<u64, Vec<u64>> = obs
        .pull_requests
        .iter()
        .map(|p| {
            let (declared, stacked) = classify::confirmed_dependencies(p, &obs.base, heads);
            let to: Vec<u64> = declared
                .into_iter()
                .chain(stacked)
                .filter(|n| open.contains(n))
                .collect();
            (p.number, to)
        })
        .collect();
    // everything each pull request reaches through its edges
    let reach = |from: u64| -> BTreeSet<u64> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(n) = stack.pop() {
            for m in edges.get(&n).into_iter().flatten() {
                if seen.insert(*m) {
                    stack.push(*m);
                }
            }
        }
        seen
    };
    let reaches: BTreeMap<u64, BTreeSet<u64>> = open.iter().map(|n| (*n, reach(*n))).collect();
    open.iter()
        .filter(|n| reaches[n].contains(n))
        .map(|n| {
            let others: Vec<u64> = reaches[n]
                .iter()
                .filter(|m| *m != n && reaches[*m].contains(n))
                .copied()
                .collect();
            (*n, others)
        })
        .collect()
}

/// Add the dependencies git implies: a pull request whose observed head contains another's
/// observed head carries that one's commits, so landing it lands both. Inferred, and evidence
/// only — a block is only ever declared ([`DependencyCertainty::Confirmed`]), never guessed,
/// and nothing here changes a disposition or a rank. `containing` answers, for a head, the
/// pull requests whose mirrored head contains it and the commit each mirror names; a mirror
/// that is not the head the queue decided on implies nothing.
pub fn infer_dependencies(
    queue: &mut IntegrationQueue,
    mut containing: impl FnMut(&str) -> Vec<(u64, String)>,
) {
    let heads: BTreeMap<u64, String> = queue
        .assessments
        .iter()
        .map(|a| (a.number, a.evaluated_against.head_sha.clone()))
        .collect();
    let mut implied: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    for (n, head) in &heads {
        for (m, tip) in containing(head) {
            if m != *n && heads.get(&m) == Some(&tip) {
                implied.entry(m).or_default().insert(*n);
            }
        }
    }
    for a in &mut queue.assessments {
        for n in implied.get(&a.number).into_iter().flatten() {
            if a.dependencies.iter().any(|d| d.number == *n) {
                continue;
            }
            a.dependencies.push(PullRequestDependency {
                number: *n,
                certainty: DependencyCertainty::Inferred,
                satisfied: false,
                state: DependencyState::Open,
            });
            a.evidence.push(IntegrationEvidence {
                kind: EvidenceKind::Dependency,
                status: "inferred".into(),
                detail: format!("#{n}'s head is in this head: landing it lands #{n} too"),
                source: Some(EvidenceSource::Git {
                    master_sha: a.evaluated_against.master_sha.clone(),
                    head_sha: a.evaluated_against.head_sha.clone(),
                }),
            });
        }
    }
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

/// The queue's word on the pull requests whose cross-references were not read whole: one
/// line naming them, and which were read in part and which not at all. Nothing when every one
/// was read whole.
fn references_unread_diagnostic(unread: &BTreeMap<u64, CrossReferenceRead>) -> Option<String> {
    let named = |want: Option<CrossReferenceRead>| {
        unread
            .iter()
            .filter(|(_, read)| want.is_none_or(|w| w == **read))
            .map(|(n, _)| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let part = |label: &str, want: CrossReferenceRead| {
        Some(named(Some(want)))
            .filter(|numbers| !numbers.is_empty())
            .map(|numbers| format!("{label}: {numbers}"))
    };
    let parts: Vec<String> = [
        part("truncated", CrossReferenceRead::Truncated),
        part("unread", CrossReferenceRead::Unread),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!unread.is_empty()).then(|| {
        format!(
            "the pull requests that mention {} were not all read ({}): each is held, never \
             merged and never closed. majordomus prs refresh reads an unread one again; a \
             truncated one has more mentions than are read ({} were read, and a mention by \
             an issue or from another repository counts toward that limit), which no \
             refresh clears: a person decides it (docs/INTEGRATION.md, held by its mentions)",
            named(None),
            parts.join("; "),
            forge::REFERENCE_PAGES * 100
        )
    })
}

/// What the bodies of an observation declare about replacement, sorted by who said it.
struct DeclaredSuccessors {
    /// Replaced → its authorised successors, each with what became of it.
    authorised: BTreeMap<u64, Vec<classify::Successor>>,
    /// Replaced → the declarations nobody entitled made about it.
    possible: BTreeMap<u64, Vec<classify::PossibleSupersession>>,
}

/// One declaration, by the pull request whose body carries it.
struct Declarer<'a> {
    number: u64,
    login: &'a str,
    association: &'a str,
    cross_repository: bool,
    /// An open one of this repository of which nothing was read: neither who its author is
    /// to the repository nor the pull requests that mention it.
    unread: bool,
}

impl Declarer<'_> {
    fn authorised(&self) -> bool {
        classify::declaration_is_authorised(self.association, self.cross_repository)
    }

    /// How much its word weighs when two bodies declare one pair: an authorised one over one
    /// that could not be read, and that over one that is not authorised.
    fn weight(&self) -> u8 {
        if self.authorised() {
            2
        } else {
            u8::from(self.unread)
        }
    }
}

/// The declared successors of every open pull request. A declaration counts only from someone
/// the repository lets declare one ([`classify::declaration_is_authorised()`]), tested on the
/// author of the body that speaks: the replaced one's own (`Superseded by`), an open
/// successor's, or a resolved one's (`Supersedes`). An authorised successor comes with what
/// became of it: open; or no longer open and landed when `relation` finds its head, or else
/// its merge commit, contained in master — whatever the forge calls it; or not landed; or
/// unread, when the forge did not report it or git could not answer. Anyone else's declaration
/// is listed as possible and nothing is asked of git about it. When two bodies declare one
/// pair, an authorised one stands over an unauthorised one, and of two alike the replaced
/// one's own body stands.
///
/// Git's word is asked only of a successor that can have landed. One the forge says changes
/// no file brought nothing to master wherever its head points, and one closed unmerged with
/// its head in a fork has a head its owner could point anywhere before closing: neither is a
/// landing, whatever master contains. And an open pull request of which nothing could be read
/// may be one whose word counts: what it says it supersedes is held until a refresh reads it,
/// never released on a read that failed.
fn successors<'a>(
    obs: &'a ForgeObservation,
    open: &BTreeSet<u64>,
    relation: &mut impl FnMut(&PullRequestObservation) -> RelationToMaster,
) -> DeclaredSuccessors {
    use classify::{declared_supersessions, PossibleSupersession, Successor, SuccessorState};
    // (replaced, successor) → whose body said so
    let mut declared: BTreeMap<(u64, u64), Declarer<'a>> = BTreeMap::new();
    let mut declare = |replaced: u64, successor: u64, by: Declarer<'a>| {
        let pair = (replaced, successor);
        let stands = declared
            .get(&pair)
            .is_some_and(|kept| kept.weight() >= by.weight());
        if replaced != successor && open.contains(&replaced) && !stands {
            declared.insert(pair, by);
        }
    };
    let of_open = |p: &'a PullRequestObservation| Declarer {
        number: p.number,
        login: &p.author,
        association: &p.author_association,
        cross_repository: p.cross_repository,
        unread: !p.cross_repository
            && p.author_association.is_empty()
            && p.cross_references == CrossReferenceRead::Unread,
    };
    // the replaced one's own body first, so that of two authorised bodies it is the one named
    for p in &obs.pull_requests {
        for m in declared_supersessions(&p.body).superseded_by {
            declare(p.number, m, of_open(p));
        }
    }
    for p in &obs.pull_requests {
        for n in declared_supersessions(&p.body).supersedes {
            declare(n, p.number, of_open(p));
        }
    }
    for (m, r) in &obs.resolved {
        for n in declared_supersessions(&r.body).supersedes {
            let by = Declarer {
                number: *m,
                login: &r.author,
                association: &r.author_association,
                cross_repository: r.cross_repository,
                unread: false,
            };
            declare(n, *m, by);
        }
    }
    let mut state_of = |m: u64| -> SuccessorState {
        if open.contains(&m) {
            return SuccessorState::Open;
        }
        let Some(r) = obs.resolved.get(&m) else {
            return SuccessorState::Unread;
        };
        // the two vetoes below read "a fork's" and "no file" as facts, and each releases what
        // a successor closed unmerged held. A reading that left either out said neither.
        if !r.whole {
            return SuccessorState::Unread;
        }
        // what cannot be a landing is decided before git is asked where its head is
        if r.changed_files == 0 {
            return SuccessorState::Empty { merged: r.merged };
        }
        if r.cross_repository && !r.merged {
            return SuccessorState::ForkClosed;
        }
        // the pull request the forge described, at one commit of it
        let at = |sha: &str| PullRequestObservation {
            number: m,
            title: String::new(),
            author: r.author.clone(),
            head_ref: String::new(),
            head_sha: sha.to_string(),
            base_ref: obs.base.clone(),
            draft: false,
            labels: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
            body: r.body.clone(),
            checks: Vec::new(),
            review_decision: String::new(),
            auto_merge: false,
            cross_repository: r.cross_repository,
            latest_reviews: Vec::new(),
            review_requests: Vec::new(),
            author_association: r.author_association.clone(),
            cross_references: CrossReferenceRead::Whole,
        };
        // landed is git's fact: the head, or the merge commit, is in master. The forge's
        // word that it merged is in no condition here.
        let head = relation(&at(&r.head_sha));
        let contained = |r: &RelationToMaster| matches!(r, RelationToMaster::Contained);
        if contained(&head) {
            SuccessorState::Landed {
                head_sha: r.head_sha.clone(),
                merged: r.merged,
                by_merge_commit: false,
            }
        } else if !r.merge_commit.is_empty() && contained(&relation(&at(&r.merge_commit))) {
            SuccessorState::Landed {
                head_sha: r.merge_commit.clone(),
                merged: r.merged,
                by_merge_commit: true,
            }
        } else if matches!(head, RelationToMaster::Unknown { .. }) {
            SuccessorState::Unread
        } else {
            SuccessorState::NotLanded { merged: r.merged }
        }
    };
    let mut states: BTreeMap<u64, SuccessorState> = BTreeMap::new();
    let mut found = DeclaredSuccessors {
        authorised: BTreeMap::new(),
        possible: BTreeMap::new(),
    };
    for ((replaced, successor), by) in declared {
        // its own body's word that it replaces another, when nothing was read of it
        let unread = by.unread && by.number == successor;
        if by.authorised() || unread {
            let state = if unread {
                SuccessorState::DeclarerUnread
            } else {
                states
                    .entry(successor)
                    .or_insert_with(|| state_of(successor))
                    .clone()
            };
            found
                .authorised
                .entry(replaced)
                .or_default()
                .push(Successor {
                    number: successor,
                    declared_in: by.number,
                    state,
                });
        } else {
            found
                .possible
                .entry(replaced)
                .or_default()
                .push(PossibleSupersession {
                    successor,
                    declared_in: by.number,
                    declared_by: by.login.to_string(),
                    association: by.association.to_string(),
                    cross_repository: by.cross_repository,
                });
        }
    }
    found
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
    let current = |k: &String| {
        k.split_once("..")
            .is_some_and(|(m, h)| m == master && is_object_id(m) && is_object_id(h))
    };
    cache.entries.retain(|k, _| current(k));
    cache.shapes.retain(|k, _| current(k));
    // the relation and the shape share the cache, and each is asked from its own closure
    let cell = std::cell::RefCell::new(cache);
    let mut queue = build_queue_shaped(
        &obs,
        &master,
        |p| {
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
            relation_cached(root, &mut cell.borrow_mut(), &master, &p.head_sha)
        },
        |p| shape_cached(root, &mut cell.borrow_mut(), &master, &p.head_sha),
    );
    let cache = cell.into_inner();
    infer_dependencies(&mut queue, |head| relation::containing(root, head));
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
        .map(forge::read_whole)
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
                merge_commit: String::new(),
                author: "a".into(),
                author_association: "OWNER".into(),
                cross_repository: false,
                base_ref: "master".into(),
                changed_files: 1,
                whole: true,
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

#[cfg(test)]
mod declared_successors {
    //! Who may declare a successor, which body is believed when two speak, and what git —
    //! never the forge — says became of one: over hand-built observations and a relation
    //! keyed on the commit asked about.

    use super::*;
    use std::cell::RefCell;

    fn open(n: u64, body: &str, association: &str, fork: bool) -> PullRequestObservation {
        let mut p = forge::pull_request_of(&serde_json::json!({
            "number": n, "title": "t", "author": {"login": format!("u{n}")},
            "headRefName": format!("f/{n}"), "headRefOid": format!("h{n}"),
            "baseRefName": "master", "body": body, "isCrossRepository": fork
        }))
        .map(forge::read_whole)
        .unwrap();
        p.author_association = association.into();
        p
    }

    fn gone(merged: bool, n: u64, body: &str, association: &str) -> forge::ResolvedPullRequest {
        forge::ResolvedPullRequest {
            merged,
            head_sha: format!("h{n}"),
            body: body.into(),
            merge_commit: String::new(),
            author: format!("u{n}"),
            author_association: association.into(),
            cross_repository: false,
            base_ref: "master".into(),
            changed_files: 1,
            whole: true,
        }
    }

    fn obs(
        prs: Vec<PullRequestObservation>,
        resolved: Vec<(u64, forge::ResolvedPullRequest)>,
    ) -> ForgeObservation {
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
            pull_requests: prs,
            resolved: resolved.into_iter().collect(),
            delete_branch_on_merge: None,
        }
    }

    /// The queue of `o`, with git answering by commit: `contained` are in master, `unknown`
    /// could not be read, and every other commit is behind. Returns the queue and every pull
    /// request the relation was asked about, as it was handed over.
    fn queue(
        o: &ForgeObservation,
        contained: &[&str],
        unknown: &[&str],
    ) -> (IntegrationQueue, Vec<PullRequestObservation>) {
        let asked = RefCell::new(Vec::new());
        let q = build_queue(o, "m", |p| {
            asked.borrow_mut().push(p.clone());
            if contained.contains(&p.head_sha.as_str()) {
                RelationToMaster::Contained
            } else if unknown.contains(&p.head_sha.as_str()) {
                RelationToMaster::Unknown {
                    reason: "not fetched".into(),
                }
            } else {
                RelationToMaster::Behind {
                    behind: 1,
                    authored: vec!["a.txt".into()],
                }
            }
        });
        (q, asked.into_inner())
    }

    fn supersession(a: &PullRequestAssessment) -> Vec<(&str, &str)> {
        a.evidence
            .iter()
            .filter(|e| e.kind == EvidenceKind::Supersession)
            .map(|e| (e.status.as_str(), e.detail.as_str()))
            .collect()
    }

    fn gate_passed(a: &PullRequestAssessment) -> bool {
        a.gates
            .iter()
            .any(|g| g.gate == IntegrationGate::Supersession && g.passed)
    }

    #[test]
    fn a_declaration_counts_only_from_who_may_declare() {
        let mut fork = gone(true, 3, "Supersedes #1", "COLLABORATOR");
        fork.cross_repository = true;
        let mut o = obs(
            vec![
                open(1, "", "OWNER", false),
                open(4, "Supersedes #1", "NONE", false),
            ],
            vec![
                (2, gone(true, 2, "Supersedes #1", "CONTRIBUTOR")),
                (3, fork),
            ],
        );
        // every one of them is on master, and that changes nothing
        let (q, asked) = queue(&o, &["h2", "h3", "h5"], &[]);
        let a = q.get(1).unwrap();
        assert!(gate_passed(a), "{:?}", a.reasons);
        assert_ne!(a.disposition, PullRequestDisposition::Superseded);
        assert_ne!(a.disposition, PullRequestDisposition::WaitingForDependency);
        assert_eq!(a.superseded_by, None);
        let said = supersession(a);
        assert_eq!(
            said.iter().map(|(status, _)| *status).collect::<Vec<_>>(),
            ["possible_supersession"; 3],
            "{said:?}"
        );
        assert!(said[0]
            .1
            .starts_with("#2 by u2 (CONTRIBUTOR) says it supersedes #1"));
        assert!(said[1]
            .1
            .starts_with("#3 by u3 (COLLABORATOR, from a fork) says"));
        assert!(said[2]
            .1
            .starts_with("#4 by u4 (NONE) says it supersedes #1"));
        assert_eq!(
            asked
                .iter()
                .map(|p| p.head_sha.as_str())
                .collect::<Vec<_>>(),
            ["h1", "h4"],
            "git is not asked about a declaration nobody entitled made"
        );
        assert!(gate_passed(q.get(4).unwrap()), "the declarer is not held");

        // a member says so too: that one counts, and the others are still only said
        o.resolved
            .insert(5, gone(true, 5, "Supersedes #1", "MEMBER"));
        let (q, _) = queue(&o, &["h2", "h3", "h5"], &[]);
        let a = q.get(1).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert_eq!(a.superseded_by, Some(5));
        let said = supersession(a);
        assert_eq!(said[0].0, "landed", "{said:?}");
        assert_eq!(
            said.iter()
                .filter(|(status, _)| *status == "possible_supersession")
                .count(),
            3
        );
    }

    #[test]
    fn the_replaced_ones_own_body_is_tested_on_its_own_author() {
        let decided = |association: &str, fork: bool| {
            let o = obs(
                vec![
                    open(1, "Superseded by #2", association, fork),
                    open(2, "", "OWNER", false),
                ],
                vec![],
            );
            queue(&o, &[], &[]).0.get(1).unwrap().clone()
        };
        let a = decided("OWNER", false);
        assert_eq!(a.disposition, PullRequestDisposition::WaitingForDependency);
        assert_eq!(a.reasons[0], "successor_open:#2");
        for (association, fork, who) in [
            ("CONTRIBUTOR", false, "its author u1 (CONTRIBUTOR) is not"),
            ("OWNER", true, "its author u1 (OWNER, from a fork) is not"),
            ("", false, "its author u1 (association unread) is not"),
        ] {
            let a = decided(association, fork);
            assert!(gate_passed(&a), "{association} {fork}: {:?}", a.reasons);
            assert!(
                !a.reasons.iter().any(|r| *r == "successor_open:#2"),
                "{:?}",
                a.reasons
            );
            let said = supersession(&a);
            assert_eq!(said.len(), 1, "{said:?}");
            assert_eq!(said[0].0, "possible_supersession");
            assert!(
                said[0].1.starts_with("its body says superseded by #2, but")
                    && said[0].1.contains(who),
                "{}",
                said[0].1
            );
        }
    }

    #[test]
    fn a_pair_both_bodies_declare_is_the_replaced_ones_own() {
        let said_of = |replaced: &str, successor: &str| {
            let o = obs(
                vec![
                    open(2, "Supersedes #3", successor, false),
                    open(3, "Superseded by #2", replaced, false),
                ],
                vec![],
            );
            let (q, _) = queue(&o, &[], &[]);
            supersession(q.get(3).unwrap())
                .into_iter()
                .map(|(status, detail)| (status.to_string(), detail.to_string()))
                .collect::<Vec<_>>()
        };
        // both may declare: one successor, named by the replaced one's own body
        let said = said_of("OWNER", "OWNER");
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].0, "open");
        assert!(
            said[0].1.contains("its body says superseded by #2"),
            "{}",
            said[0].1
        );
        // only the successor may: its declaration stands over the one that does not count
        let said = said_of("CONTRIBUTOR", "OWNER");
        assert_eq!(
            said.len(),
            1,
            "no possible supersession beside it: {said:?}"
        );
        assert_eq!(said[0].0, "open");
        assert!(
            said[0].1.contains("#2 says it supersedes #3"),
            "{}",
            said[0].1
        );
        // only the replaced one may: its own stands, and the other adds nothing
        let said = said_of("OWNER", "CONTRIBUTOR");
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].1.contains("its body says superseded by #2"),
            "{}",
            said[0].1
        );
        // neither may: said once, by the one who said it first, and nothing is held
        let said = said_of("NONE", "CONTRIBUTOR");
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].0, "possible_supersession");
        assert!(
            said[0].1.starts_with("its body says superseded by #2, but"),
            "{}",
            said[0].1
        );
    }

    /// A successor closed unmerged releases what it held when the forge said it is a fork's
    /// or changes no file. When the reading left those out, they are defaults and not facts:
    /// the successor is unread, the hold stands, and git is not asked about it.
    #[test]
    fn a_successor_read_without_its_place_or_its_files_releases_nothing() {
        let decided = |whole: bool, fork: bool, files: u64| {
            let mut two = gone(false, 2, "", "CONTRIBUTOR");
            two.cross_repository = fork;
            two.changed_files = files;
            two.whole = whole;
            let o = obs(
                vec![open(1, "Superseded by #2", "OWNER", false)],
                vec![(2, two)],
            );
            let (q, asked) = queue(&o, &[], &[]);
            (q.get(1).unwrap().clone(), asked.len())
        };
        // read whole: a fork closed unmerged, and one that changes nothing, release the hold
        for (fork, files) in [(true, 1), (false, 0)] {
            let (a, _) = decided(true, fork, files);
            assert!(
                !a.reasons
                    .contains(&ReasonCode::SuccessorUnread { number: 2 }),
                "{:?}",
                a.reasons
            );
        }
        // the same values as defaults of a reading that did not say: unread, held, by refresh
        for (fork, files) in [(true, 0), (true, 1), (false, 0)] {
            let (a, asked) = decided(false, fork, files);
            assert!(
                a.reasons
                    .contains(&ReasonCode::SuccessorUnread { number: 2 }),
                "fork {fork}, files {files}: {:?}",
                a.reasons
            );
            assert_eq!(
                asked, 1,
                "git is asked about #1 alone, never about the unread one"
            );
        }
    }

    #[test]
    fn a_successor_landed_when_git_finds_its_head_or_its_merge_commit() {
        // #1's own body, an owner's, names #2: what became of #2 is all that varies
        let fork = std::cell::Cell::new(true);
        let files = std::cell::Cell::new(1);
        let decided = |merged: bool, merge_commit: &str, contained: &[&str], unknown: &[&str]| {
            let mut two = gone(merged, 2, "", "CONTRIBUTOR");
            two.merge_commit = merge_commit.into();
            two.cross_repository = fork.get();
            two.changed_files = files.get();
            let o = obs(
                vec![open(1, "Superseded by #2", "OWNER", false)],
                vec![(2, two)],
            );
            let (q, asked) = queue(&o, contained, unknown);
            let commits: Vec<String> = asked.iter().map(|p| p.head_sha.clone()).collect();
            (q.get(1).unwrap().clone(), commits, asked)
        };
        let evidence = |a: &PullRequestAssessment| supersession(a)[0].1.to_string();

        let (a, commits, asked) = decided(true, "mc2", &["h2"], &[]);
        assert_eq!(a.superseded_by, Some(2));
        assert!(evidence(&a).contains("is merged, and master contains its head h2"));
        assert_eq!(
            commits,
            ["h1", "h2"],
            "its head landed: the merge commit is not asked"
        );
        // git is asked about the pull request the forge described, not a blank one
        let two = &asked[1];
        assert_eq!(
            (
                two.number,
                two.author.as_str(),
                two.author_association.as_str(),
                two.cross_repository,
                two.base_ref.as_str(),
                two.cross_references
            ),
            (
                2,
                "u2",
                "CONTRIBUTOR",
                true,
                "master",
                CrossReferenceRead::Whole
            )
        );

        let (a, commits, _) = decided(true, "mc2", &["mc2"], &[]);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert!(
            evidence(&a).contains("is merged, and master contains its merge commit mc2"),
            "{}",
            evidence(&a)
        );
        assert_eq!(commits, ["h1", "h2", "mc2"]);

        let (a, commits, _) = decided(true, "", &[], &[]);
        assert_eq!(a.disposition, PullRequestDisposition::PossiblyRedundant);
        assert_eq!(a.reasons[0], "successor_not_landed:#2");
        assert_eq!(
            commits,
            ["h1", "h2"],
            "no merge commit: nothing more to ask"
        );

        let (a, commits, _) = decided(true, "mc2", &[], &[]);
        assert_eq!(a.reasons[0], "successor_not_landed:#2");
        assert_eq!(commits, ["h1", "h2", "mc2"]);

        // closed unmerged with its head in a fork: wherever that head points — on master
        // here — it replaces nothing, and git is not asked about it
        let (a, commits, _) = decided(false, "mc2", &["h2", "mc2"], &[]);
        assert!(gate_passed(&a), "{:?}", a.reasons);
        assert_eq!(a.superseded_by, None);
        assert!(
            evidence(&a).contains("was closed unmerged and its head lives in a fork"),
            "{}",
            evidence(&a)
        );
        assert_eq!(commits, ["h1"], "a fork's closed head proves nothing");
        fork.set(false);

        // closed unmerged and nowhere on master: it replaces nothing
        let (a, _, _) = decided(false, "", &[], &[]);
        assert!(gate_passed(&a), "{:?}", a.reasons);
        assert!(
            evidence(&a).contains("was closed unmerged, and master contains neither"),
            "{}",
            evidence(&a)
        );

        // a head git cannot read says nothing, and a merge commit on master still lands it
        let (a, _, _) = decided(true, "mc2", &["mc2"], &["h2"]);
        assert_eq!(a.superseded_by, Some(2));
        assert!(evidence(&a).contains("its merge commit mc2"));
        let (a, _, _) = decided(true, "", &[], &["h2"]);
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        assert_eq!(a.reasons[0], "successor_unread:#2");
        let (a, _, _) = decided(true, "mc2", &[], &["h2"]);
        assert_eq!(
            a.reasons[0], "successor_unread:#2",
            "unread, never `not landed`"
        );

        // the forge calls it closed; a batch carried its head into master: it landed
        let (a, _, _) = decided(false, "", &["h2"], &[]);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert!(
            evidence(&a).contains("is closed, and master contains its head h2"),
            "{}",
            evidence(&a)
        );
        // and the forge's word that it merged lands nothing
        let (a, _, _) = decided(true, "mc2", &[], &[]);
        assert_ne!(a.disposition, PullRequestDisposition::Superseded);

        // one the forge says changes no file brought nothing, wherever its head points: a
        // branch reset onto master and closed releases, and one called merged is a person's
        files.set(0);
        let (a, commits, _) = decided(false, "mc2", &["h2", "mc2"], &[]);
        assert!(gate_passed(&a), "{:?}", a.reasons);
        assert_eq!(a.superseded_by, None);
        assert!(
            evidence(&a).contains("#2 (its body says superseded by #2) is closed, and the forge names no file it changes")
                && evidence(&a).contains("and #1 is decided on its own"),
            "{}",
            evidence(&a)
        );
        assert_eq!(
            commits,
            ["h1"],
            "git is not asked where an empty one points"
        );
        let (a, _, _) = decided(true, "mc2", &["h2", "mc2"], &[]);
        assert_eq!(a.disposition, PullRequestDisposition::PossiblyRedundant);
        assert_eq!(a.reasons[0], "successor_not_landed:#2");
        assert!(
            evidence(&a).contains("is merged, and the forge names no file it changes")
                && !evidence(&a).contains("decided on its own"),
            "{}",
            evidence(&a)
        );
    }

    #[test]
    fn an_open_declarer_nothing_was_read_of_holds_what_it_names() {
        // #2 says it supersedes #1, and neither who its author is nor what mentions it could
        // be read: its word may count, so #1 is held, never released on a read that failed
        let unread = |fork: bool| {
            let mut two = open(2, "Supersedes #1", "", fork);
            two.cross_references = CrossReferenceRead::Unread;
            two
        };
        let o = obs(vec![open(1, "", "OWNER", false), unread(false)], vec![]);
        let (q, asked) = queue(&o, &[], &[]);
        let a = q.get(1).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        assert_eq!(a.reasons[0], "successor_unread:#2");
        assert_eq!(a.superseded_by, None);
        let said = supersession(a);
        assert_eq!(said[0].0, "unread");
        assert!(
            said[0]
                .1
                .contains("#2 (#2 says it supersedes #1) is open, and who its author is")
                && said[0].1.contains("#1 is held until it is read"),
            "{}",
            said[0].1
        );
        assert_eq!(asked.len(), 2, "nothing is asked of git about a successor");
        // the replaced one's own word, a contributor's, does not stand over it
        let o = obs(
            vec![
                open(1, "Superseded by #2", "CONTRIBUTOR", false),
                unread(false),
            ],
            vec![],
        );
        let (q, _) = queue(&o, &[], &[]);
        assert_eq!(q.get(1).unwrap().reasons[0], "successor_unread:#2");
        // and an owner's own word does: #2 is its successor, open
        let o = obs(
            vec![open(1, "Superseded by #2", "OWNER", false), unread(false)],
            vec![],
        );
        let (q, _) = queue(&o, &[], &[]);
        assert_eq!(q.get(1).unwrap().reasons[0], "successor_open:#2");
        // one the list calls a fork declares nothing, read or not: evidence only
        let o = obs(vec![open(1, "", "OWNER", false), unread(true)], vec![]);
        let (q, _) = queue(&o, &[], &[]);
        let a = q.get(1).unwrap();
        assert!(gate_passed(a), "{:?}", a.reasons);
        assert_eq!(supersession(a)[0].0, "possible_supersession");
        // nor does one whose references were read and whose head the read did not place
        let mut placed = unread(false);
        placed.cross_references = CrossReferenceRead::Whole;
        let o = obs(vec![open(1, "", "OWNER", false), placed], vec![]);
        let (q, _) = queue(&o, &[], &[]);
        assert!(gate_passed(q.get(1).unwrap()));
        // what an unread one's own body says replaces it holds nothing more than its own hold
        let mut own = unread(false);
        own.body = "Superseded by #1".into();
        let o = obs(vec![open(1, "", "OWNER", false), own], vec![]);
        let (q, _) = queue(&o, &[], &[]);
        let a = q.get(2).unwrap();
        assert_eq!(a.reasons[0], "declarations_unread");
        assert_eq!(supersession(a)[1].0, "possible_supersession");
    }

    #[test]
    fn a_pull_request_whose_references_were_not_read_is_held_and_the_queue_says_so() {
        let mut truncated = open(1, "", "OWNER", false);
        truncated.cross_references = CrossReferenceRead::Truncated;
        let mut unread = open(2, "", "", false);
        unread.cross_references = CrossReferenceRead::Unread;
        let o = obs(
            vec![truncated, unread.clone(), open(3, "", "OWNER", false)],
            vec![],
        );
        let (q, _) = queue(&o, &[], &[]);
        for n in [1, 2] {
            let a = q.get(n).unwrap();
            assert_eq!(a.disposition, PullRequestDisposition::Unknown, "#{n}");
            assert_eq!(a.reasons[0], "declarations_unread", "#{n}");
        }
        assert!(gate_passed(q.get(3).unwrap()));
        let said: Vec<&String> = q
            .diagnostics
            .iter()
            .filter(|d| d.contains("were not all read"))
            .collect();
        assert_eq!(
            said,
            ["the pull requests that mention #1 #2 were not all read (truncated: #1; unread: #2): \
              each is held, never merged and never closed. majordomus prs refresh reads an \
              unread one again; a truncated one has more mentions than are read (5000 were \
              read, and a mention by an issue or from another repository counts toward that \
              limit), which no refresh clears: a person decides it (docs/INTEGRATION.md, held \
              by its mentions)"]
        );
        let (q, _) = queue(&obs(vec![unread], vec![]), &[], &[]);
        assert!(
            q.diagnostics
                .iter()
                .any(|d| d.contains("mention #2 were not all read (unread: #2)")),
            "{:?}",
            q.diagnostics
        );
        let (q, _) = queue(&obs(vec![open(3, "", "OWNER", false)], vec![]), &[], &[]);
        assert!(
            !q.diagnostics
                .iter()
                .any(|d| d.contains("were not all read")),
            "{:?}",
            q.diagnostics
        );
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
                .map(forge::read_whole)
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

#[cfg(test)]
mod dependency_queue {
    //! The queue-wide dependency facts: cycles, the forge's list limit, inferred containment.

    use super::*;

    fn pr(n: u64, body: &str) -> PullRequestObservation {
        forge::pull_request_of(&serde_json::json!({
            "number": n, "title": "t", "author": {"login": "a"}, "headRefName": format!("f/{n}"),
            "headRefOid": format!("{n:040}"), "baseRefName": "master", "isDraft": false,
            "labels": [], "createdAt": "2026-10-01T00:00:00Z",
            "updatedAt": "2026-10-01T00:00:00Z", "body": body, "statusCheckRollup": [],
            "reviewDecision": "", "autoMergeRequest": null, "isCrossRepository": false
        }))
        .map(forge::read_whole)
        .unwrap()
    }

    fn obs(prs: Vec<PullRequestObservation>) -> ForgeObservation {
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
            pull_requests: prs,
            resolved: Default::default(),
            delete_branch_on_merge: None,
        }
    }

    fn unknown(_: &PullRequestObservation) -> RelationToMaster {
        RelationToMaster::Unknown {
            reason: "not asked".into(),
        }
    }

    #[test]
    fn a_cycle_blocks_its_members_and_a_resolved_dependency_reads_its_state() {
        let mut o = obs(vec![
            pr(1, "Depends on #2"),
            pr(2, "Depends on #1"),
            pr(3, "Depends on #8\nDepends on #9"),
        ]);
        for (n, merged) in [(8, true), (9, false)] {
            o.resolved.insert(
                n,
                forge::ResolvedPullRequest {
                    merged,
                    head_sha: format!("{n:040}"),
                    body: String::new(),
                    merge_commit: String::new(),
                    author: "a".into(),
                    author_association: "OWNER".into(),
                    cross_repository: false,
                    base_ref: "master".into(),
                    changed_files: 1,
                    whole: true,
                },
            );
        }
        // a relation that passes, so the dependency gate is the one that decides
        let q = build_queue(&o, "m", |_| RelationToMaster::UpToDate {
            authored: vec!["a.txt".into()],
        });
        for n in [1, 2] {
            assert_eq!(
                q.get(n).unwrap().disposition,
                PullRequestDisposition::Blocked,
                "#{n}"
            );
        }
        let three = q.get(3).unwrap();
        let states: Vec<(u64, DependencyState)> = three
            .dependencies
            .iter()
            .map(|d| (d.number, d.state))
            .collect();
        assert_eq!(
            states,
            [
                (8, DependencyState::Merged),
                (9, DependencyState::ClosedUnmerged)
            ]
        );
    }

    #[test]
    fn as_many_open_as_the_forge_lists_says_more_may_be_open() {
        let many: Vec<PullRequestObservation> =
            (1..=forge::OPEN_LIMIT as u64).map(|n| pr(n, "")).collect();
        let q = build_queue(&obs(many), "m", unknown);
        assert!(
            q.diagnostics.iter().any(|d| d.contains("its limit")),
            "{:?}",
            q.diagnostics
        );
        let q = build_queue(&obs(vec![pr(1, "")]), "m", unknown);
        assert!(!q.diagnostics.iter().any(|d| d.contains("its limit")));
    }

    #[test]
    fn containment_is_evidence_and_never_a_block() {
        let mut q = build_queue(
            &obs(vec![pr(1, ""), pr(2, ""), pr(3, "Depends on #1")]),
            "m",
            unknown,
        );
        let before: Vec<PullRequestDisposition> =
            q.assessments.iter().map(|a| a.disposition).collect();
        let head = |n: u64| format!("{n:040}");
        // #2's head contains #1's; #3's contains #1's too but #3 already declares it; a mirror
        // of #2 at another commit than the one decided on implies nothing about #2
        infer_dependencies(&mut q, |h| {
            if h == head(1) {
                vec![(2, head(2)), (3, head(3)), (1, head(1))]
            } else if h == head(3) {
                vec![(2, "elsewhere".into())]
            } else {
                Vec::new()
            }
        });
        let after: Vec<PullRequestDisposition> =
            q.assessments.iter().map(|a| a.disposition).collect();
        assert_eq!(before, after, "inference changes no disposition");
        let two = q.get(2).unwrap();
        assert_eq!(two.dependencies.len(), 1);
        assert_eq!(two.dependencies[0].number, 1);
        assert_eq!(two.dependencies[0].certainty, DependencyCertainty::Inferred);
        assert!(two.evidence.iter().any(|e| e.status == "inferred"));
        assert_eq!(
            q.get(3).unwrap().dependencies.len(),
            1,
            "a declared edge is not inferred again"
        );
    }
}
