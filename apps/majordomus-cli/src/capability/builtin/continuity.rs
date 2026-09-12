//! The `continuity` module: what this checkout's lifecycle is holding right now — the open
//! episode, the record the next worker would resume from, the newest progress note, what is
//! blocking acceptance, and whether the provider's events actually reach the tool.
//!
//! Everything else in this executable reads the shared half of the layer: objects that are
//! tracked, identical in every clone, and safe to project onto a public site. This module
//! reads the other half, `.ai/local/state/`, and the difference decides the whole design.
//!
//! **It is read, never written.** The lifecycle is the shell tool's; this process is one of
//! its readers. A second writer for the same records would be a second account of events the
//! ledger already holds, which is the thing that design refuses.
//!
//! **It is served, never published.** These records name this machine — a worktree path, the
//! checkpoint that happened to be newest on this disk — and a fact about a disk is not a
//! fact about the repository (ADR 0014). So the exposure is the loopback server and MCP,
//! both of which serve the worker sitting in front of this checkout, and there is no
//! projection into `docs/generated/` or `site/`. The static site documents the mechanism and
//! carries none of its content. A generator that published a session's briefing would
//! publish the one thing in the layer that no other clone can reproduce.
//!
//! **Absence is an answer.** Every field here can be legitimately empty: a fresh clone has
//! no ledger, a worker outside a task has no task, and a branch with no prior episode has no
//! handover. Each of those is reported as itself rather than as a failure, and the resolver
//! below never widens its search to make one go away — a record from another worktree
//! silently becoming your context is worse than having none, because you cannot tell it is
//! wrong until you have acted on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::{self, GitState};
use crate::metadata::frontmatter;
use crate::metadata::yaml;
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `continuity.state` is read as an MCP resource.
pub const CONTINUITY_URI: &str = "majordomus://continuity";

/// The local half of the layer, relative to the repository root. Not configurable here: the
/// shell tool decides where its state lives, and a second opinion about the path would be a
/// second source of truth for the one thing both halves must agree on.
const STATE_DIR: &str = ".ai/local/state";

/// How far a record's recorded commit is from the commit this checkout is on.
///
/// The four words are the shell tool's, deliberately. A reader that met `advanced` from one
/// surface and `stale` from another would have to learn the same four facts twice.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Divergence {
    /// Written at this commit. Trust it.
    Exact,
    /// Git has moved forward since. Trust it, and expect some of it to be done.
    Advanced,
    /// The recorded commit is not an ancestor: history was rewritten. Trust git, not this.
    Diverged,
    /// Another branch or another worktree. This record is not about your work.
    DifferentContext,
    /// Git could not answer, so this process says so rather than guessing `exact`.
    Unknown,
}

impl Divergence {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Divergence;
    /// assert_eq!(Divergence::Advanced.as_str(), "advanced");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Divergence::Exact => "exact",
            Divergence::Advanced => "advanced",
            Divergence::Diverged => "diverged",
            Divergence::DifferentContext => "different_context",
            Divergence::Unknown => "unknown",
        }
    }

    /// Whether a record carrying this label may be read as current knowledge. `diverged`
    /// and `different_context` may not: the first describes a history that no longer
    /// exists, the second describes somebody else's work.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Divergence;
    /// assert!(Divergence::Advanced.trustworthy());
    /// assert!(!Divergence::Diverged.trustworthy());
    /// ```
    pub fn trustworthy(self) -> bool {
        matches!(self, Divergence::Exact | Divergence::Advanced)
    }
}

/// Which tier of the resolution rule matched. There is no third tier on purpose: a record
/// from an unrelated worktree or branch is never offered.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    /// Same repository, same worktree, same branch.
    SameWorktreeSameBranch,
    /// Same repository, same branch, another worktree of it.
    SameBranch,
}

/// One durable record of the local half, as much of it as a reader needs to decide whether
/// to open the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Record {
    /// Repository-relative path. The body is at the path; it is not copied here.
    pub path: String,
    /// When the record says it was written.
    pub created_at: String,
    /// The task it belongs to, or `none`.
    pub task_id: String,
    /// The branch it was written on.
    pub branch: String,
    /// The commit it was written at.
    pub head: String,
    /// Whether the working tree was clean or dirty then.
    pub working_tree: String,
    /// Which tier of the resolution rule matched.
    pub matched: Match,
    /// How far its commit is from this one.
    pub divergence: Divergence,
    /// The section a resuming worker acts on, when the record has one. A handover's `Next
    /// Action`; empty for a record that carries no sections.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_action: String,
}

/// The open episode this checkout points at, when there is one.
///
/// One execution episode belongs to one provider session, and several can be open in one
/// checkout at once — two windows of the same provider are two workers. What is read here
/// is `state/session-current.yaml`, the pointer to the episode of this checkout; a worker
/// that knows its own provider session resolves its own episode instead, which is what
/// `mj_session_here_file` in `lib/common.sh` does and what stamps each ledger line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenSession {
    /// The episode's id.
    pub session_id: String,
    /// When it opened.
    pub started_at: String,
    /// Who opened it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    /// The worker identity, when one was supplied. Never inferred.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker: String,
    /// The provider whose event opened it, when one did.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// That provider's own session identity — the string the prompt archive stamps on the
    /// same worker's records, and the name this episode is keyed by. Empty for an episode
    /// opened by hand, which is the one episode no provider session owns.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
    /// The branch it opened on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it opened at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_head: String,
    /// True when the open record here belongs to another checkout. Such a record is
    /// reported and never treated as this checkout's episode.
    pub foreign: bool,
}

/// The active task of this checkout, when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActiveTask {
    /// The task id.
    pub id: String,
    /// What is being worked on.
    pub task: String,
    /// The execution profile it runs under.
    pub profile: String,
    /// Its typed outcome so far: `active`, `handed_over`, or a finished one.
    pub outcome: String,
    /// The paths it claims.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// The obligations it owes before the outcome `completed` is available.
    ///
    /// Beside `scope` and not inside it, because the two are different promises: scope is
    /// containment — where a worker may write — and this is delivery. A change can sit
    /// entirely inside its scope and still be uncommitted on a laptop (ADR 0030).
    /// [`super::obligations`] is what judges each of these against its evidence; here it
    /// is reported as declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// When it started.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
    /// The commit it started at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
}

/// What the lifecycle of this checkout is holding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Continuity {
    /// Whether this checkout's lifecycle has ever run: a ledger line, a task, an episode or
    /// a record. False in a fresh clone, which is not a fault.
    ///
    /// It is not "the directory exists". Several things create that directory before
    /// anything has been recorded in it, so a reader that took its presence for evidence
    /// would be told the lifecycle had run in a checkout where it never had.
    pub present: bool,
    /// The worktree this answer is about. Every selection below is scoped to it.
    pub worktree: String,
    /// The branch, or `DETACHED`.
    pub branch: String,
    /// The commit this checkout is on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
    /// `clean` or `dirty`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub working_tree: String,
    /// The open episode, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<OpenSession>,
    /// The active task, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActiveTask>,
    /// The record the next worker would resume from, or `None` when nothing resolves here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover: Option<Record>,
    /// The newest progress note for this worktree and branch, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Record>,
    /// Unresolved questions on this branch. Every one refuses `finish --outcome completed`,
    /// whichever task opened it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    /// How many records of each kind this checkout holds, against the policy's caps.
    pub tallies: BTreeMap<String, usize>,
    /// What a reader should know before trusting any of the above: a diverged record, a
    /// foreign open session, a malformed file that was skipped. Empty is the good case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- reading

/// The scalar fields of a YAML mapping, flattened to strings.
fn scalars(map: serde_json::Map<String, serde_json::Value>) -> BTreeMap<String, String> {
    map.into_iter()
        .filter_map(|(k, v)| yaml::scalar_string(&v).map(|s| (k, s)))
        .collect()
}

/// The front matter of a Markdown record. A file without front matter, or with front matter
/// that does not parse, yields `None` and is counted as skipped by the caller: a malformed
/// record degrades the answer, it never fails the call.
fn front(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let split = frontmatter::split(&text).ok()?;
    Some(scalars(yaml::parse_mapping(split.front?).ok()?))
}

/// A whole YAML document, for the two records of the local half that are not Markdown.
///
/// `session-current.yaml` (a symlink into `state/sessions-open/`) and `current.yaml` carry
/// no `---` fences — they are the state
/// itself rather than a document about it — so reading them with the front-matter splitter
/// finds nothing and reports an absent episode in a checkout that has one. Two shapes, two
/// readers, and the difference stated here rather than discovered.
pub(crate) fn document(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(scalars(yaml::parse_mapping(&text).ok()?))
}

/// One level-one section of a Markdown record, body only, trimmed. Used to lift the part of
/// a handover a resuming worker acts on without copying the document into every answer.
fn section(path: &Path, want: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut out: Vec<&str> = Vec::new();
    let mut on = false;
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("# ") {
            on = head.trim_end() == want;
            continue;
        }
        if on {
            out.push(line);
        }
    }
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Compare a record's commit with this checkout's, the same way the shell tool does: by
/// ancestry, not by equality of timestamps. `git` being unavailable yields `Unknown`, which
/// is the honest answer and not `Exact`.
pub(crate) fn divergence(root: &Path, theirs: &str, ours: Option<&str>) -> Divergence {
    let Some(ours) = ours else {
        return Divergence::Unknown;
    };
    if theirs == ours {
        return Divergence::Exact;
    }
    if theirs.is_empty() {
        return Divergence::Unknown;
    }
    match git::is_ancestor(root, theirs, ours) {
        Some(true) => Divergence::Advanced,
        Some(false) => Divergence::Diverged,
        None => Divergence::Unknown,
    }
}

/// The most relevant record in `dir` for this worktree and branch.
///
/// Two tiers and no third, exactly as `mj_resolve_latest` has them: same worktree and
/// branch, then same branch in another worktree of the same repository, then nothing. The
/// ordering inside a tier is by the timestamp the record asserts — never by filesystem
/// modification time, which does not survive a clone and is not the time the record claims.
///
/// Returns the record and the number of files that were skipped because they could not be
/// read as records.
fn resolve(root: &Path, dir: &Path, branch: &str, head: Option<&str>) -> (Option<Record>, usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (None, 0);
    };
    let worktree = root.to_string_lossy().to_string();
    let mut best: Option<(u8, String, Record)> = None;
    let mut skipped = 0usize;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    // A deterministic walk, so two runs over one directory agree about which of two
    // records written in the same second is newer.
    paths.sort();

    for path in paths {
        let Some(f) = front(&path) else {
            skipped += 1;
            continue;
        };
        let (Some(created), Some(rhead)) = (f.get("created_at"), f.get("head")) else {
            skipped += 1;
            continue;
        };
        if f.get("schema_version").map(String::as_str) != Some("1") {
            skipped += 1;
            continue;
        }
        let rbranch = f.get("branch").cloned().unwrap_or_default();
        let rworktree = f.get("worktree").cloned().unwrap_or_default();
        let tier = if rworktree == worktree && rbranch == branch {
            0u8
        } else if branch != "DETACHED" && rbranch == branch {
            1u8
        } else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let record = Record {
            path: rel,
            created_at: created.clone(),
            task_id: f.get("task_id").cloned().unwrap_or_else(|| "none".into()),
            branch: rbranch,
            head: rhead.clone(),
            working_tree: f.get("working_tree").cloned().unwrap_or_default(),
            matched: if tier == 0 {
                Match::SameWorktreeSameBranch
            } else {
                Match::SameBranch
            },
            divergence: divergence(root, rhead, head),
            next_action: section(&path, "Next Action"),
        };
        let key = created.clone();
        let better = match &best {
            None => true,
            Some((btier, bkey, _)) => tier < *btier || (tier == *btier && key > *bkey),
        };
        if better {
            best = Some((tier, key, record));
        }
    }
    (best.map(|(_, _, r)| r), skipped)
}

/// How many `.md` records a directory holds.
fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
                .count()
        })
        .unwrap_or(0)
}

/// The unresolved entries of the blocker store. The line format is machine-written precisely
/// so a reader like this one can rely on it; a line that does not match is not silently
/// dropped, it is simply not unresolved, and `doctor` is what reports a store that does not
/// parse.
///
/// The HTML comment block is skipped, exactly as the shell reader skips it. The store's
/// template documents its own line format inside a comment, so a reader that does not skip
/// it reports a blocker that does not exist in every fresh checkout — and a phantom blocker
/// is worse than a missed one, because it refuses work nobody can unblock.
fn blockers(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_comment = false;
    for line in text.lines() {
        if line.contains("<!--") {
            in_comment = true;
        }
        if in_comment {
            if line.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("- [unresolved] ") {
            out.push(rest.trim().to_string());
        }
    }
    out
}

// --------------------------------------------------------------------- handler

fn state(ctx: &Context, _: Empty) -> Result<Continuity, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = root.join(STATE_DIR);

    let (branch, head, working_tree) = match &ctx.index.repository.git {
        GitState::Available(info) => (
            info.branch.clone().unwrap_or_else(|| "DETACHED".into()),
            info.head.clone(),
            info.working_tree.clone(),
        ),
        GitState::Unavailable { .. } => ("DETACHED".into(), None, String::new()),
    };

    let mut findings = Vec::new();
    let mut skipped = 0usize;

    // --- the open episode
    let session = match document(&dir.join("session-current.yaml")) {
        Some(f) => {
            let sworktree = f.get("worktree").cloned().unwrap_or_default();
            let foreign = !sworktree.is_empty() && sworktree != root.to_string_lossy();
            if foreign {
                findings.push(format!(
                    "the open session record here belongs to {sworktree}, not this checkout; nothing about it is about your work"
                ));
            }
            f.get("session_id").map(|id| OpenSession {
                session_id: id.clone(),
                started_at: f.get("started_at").cloned().unwrap_or_default(),
                owner: f.get("owner").cloned().unwrap_or_default(),
                worker: f.get("worker").cloned().unwrap_or_default(),
                provider: f.get("provider").cloned().unwrap_or_default(),
                provider_session: f.get("provider_session").cloned().unwrap_or_default(),
                branch: f.get("branch").cloned().unwrap_or_default(),
                start_head: f.get("start_head").cloned().unwrap_or_default(),
                foreign,
            })
        }
        None => None,
    };

    // --- the active task. Its `scope` is a list, so it is read from the document rather
    // than from the scalar flattening the other fields use.
    let task = read_task(&dir.join("current.yaml"));

    // --- the two resolved records
    let (handover, s1) = resolve(&root, &dir.join("handovers"), &branch, head.as_deref());
    let (checkpoint, s2) = resolve(&root, &dir.join("checkpoints"), &branch, head.as_deref());
    skipped += s1 + s2;
    if skipped > 0 {
        findings.push(format!(
            "{skipped} file(s) under {STATE_DIR} could not be read as records and were skipped; run `majordomus doctor`"
        ));
    }
    for (what, r) in [("handover", &handover), ("checkpoint", &checkpoint)] {
        if let Some(r) = r {
            if !r.divergence.trustworthy() {
                findings.push(format!(
                    "the resolved {what} is {}: it was written at {} and that commit is not in this history; trust git over it",
                    r.divergence.as_str(),
                    &r.head[..7.min(r.head.len())]
                ));
            }
        }
    }

    let blockers = blockers(&dir.join("open-questions.md"));
    if !blockers.is_empty() {
        findings.push(format!(
            "{} unresolved question(s) on this branch; every one refuses `majordomus finish --outcome completed`",
            blockers.len()
        ));
    }

    let mut tallies = BTreeMap::new();
    tallies.insert("handovers".to_string(), count(&dir.join("handovers")));
    tallies.insert("checkpoints".to_string(), count(&dir.join("checkpoints")));
    // No `sessions` tally. A closed episode is an object of the shared layer, so the index
    // already counts it and `objects.list --kind session` already answers it; a second
    // count here, over a directory the closed records no longer live in, is the drift this
    // repository forbids rather than a convenience.
    tallies.insert("blockers".to_string(), blockers.len());
    tallies.insert(
        "ledger_lines".to_string(),
        std::fs::read_to_string(dir.join("ledger.jsonl"))
            .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
    );

    let present = task.is_some() || session.is_some() || tallies.values().any(|n| *n > 0);

    Ok(Continuity {
        present,
        worktree: root.to_string_lossy().to_string(),
        branch,
        head: head.unwrap_or_default(),
        working_tree,
        session,
        task,
        handover,
        checkpoint,
        blockers,
        tallies,
        findings,
    })
}

/// The active task record. Read whole rather than through the scalar flattening, because
/// `scope` and `requires` are lists, and a task without its scope is a task whose claim
/// nobody can check.
///
/// Shared with [`super::obligations`], which expands `requires` against the evidence in the
/// ledger. One reader for one file: a second parse of the same record is how two surfaces
/// come to disagree about what the task said.
pub(crate) fn read_task(path: &Path) -> Option<ActiveTask> {
    let text = std::fs::read_to_string(path).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let s = |k: &str| map.get(k).and_then(yaml::scalar_string).unwrap_or_default();
    let id = s("id");
    if id.is_empty() {
        return None;
    }
    let list = |k: &str| -> Vec<String> {
        map.get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(yaml::scalar_string).collect())
            .unwrap_or_default()
    };
    let scope = list("scope");
    let requires = list("requires");
    Some(ActiveTask {
        id,
        task: s("task"),
        profile: s("profile"),
        outcome: s("outcome"),
        scope,
        requires,
        started_at: s("started_at"),
        head: s("head"),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "continuity",
        title: "Continuity",
        description: "What this checkout's lifecycle is holding: the open episode, the record the next worker would resume from with the label that says how far to trust it, the newest progress note, and what is blocking acceptance. Read from the local half of the layer, which this process serves to the worker in front of it and never publishes.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "continuity.state",
                title: "What the lifecycle is holding",
                description: "The open episode, the active task, the handover and checkpoint that resolve for this worktree and branch, each with its divergence label, the unresolved questions that refuse completion, and the record tallies. Selection is two-tiered and never repository-wide: a record from an unrelated worktree or branch is not offered, because a briefing that is quietly about somebody else is worse than none. Absence is reported as absence.",
                input: Empty,
                output: Continuity,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_continuity".into()),
                        resource: Some(McpResource { uri: CONTINUITY_URI.into(), name: "continuity".into() }),
                    }),
                    http: get("/api/v1/continuity"),
                    cli: None,
                },
                tags: ["continuity", "session", "handover"],
                // Short-lived: the records under it are written by another process, and a
                // reader that cached them for a minute would answer with an episode that
                // had already closed.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: state,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// A repository with two commits, so that the three answers `is_ancestor` can give are
    /// all reachable. `git` itself, because the thing under test is what this module asks
    /// git and what it does with the reply; a fake would be testing the fake.
    struct Repo(tempfile::TempDir);
    impl Repo {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("a temporary directory");
            let r = Repo(dir);
            r.git(&["init", "-q", "."]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "t"]);
            r.git(&["commit", "-q", "--allow-empty", "-m", "one"]);
            r
        }
        fn root(&self) -> &Path {
            self.0.path()
        }
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.root())
                .args(args)
                .output()
                .expect("git");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        fn head(&self) -> String {
            self.git(&["rev-parse", "HEAD"])
        }
        fn write(&self, rel: &str, body: &str) -> PathBuf {
            let p = self.root().join(rel);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&p, body).expect("write");
            p
        }
    }

    /// The front matter of a local record, as the lifecycle writes it.
    fn record(created: &str, head: &str, branch: &str, worktree: &str, next: &str) -> String {
        format!(
            "---\nschema_version: 1\ncreated_at: {created}\ntask_id: t-1\nprofile: implementation\n\
             repository_id: x\nworktree: {worktree}\nbranch: {branch}\nhead: {head}\n\
             working_tree: clean\nchanged_files:\n---\n\n# Next Action\n\n{next}\n"
        )
    }

    // ------------------------------------------------------------------ vocabulary

    /// Every word, in both directions. The serialised form is the shell tool's, and a
    /// rename here would be a second vocabulary for the same four facts.
    #[test]
    fn every_divergence_word_is_the_shell_tools_and_says_whether_it_may_be_trusted() {
        let all = [
            (Divergence::Exact, "exact", true),
            (Divergence::Advanced, "advanced", true),
            (Divergence::Diverged, "diverged", false),
            (Divergence::DifferentContext, "different_context", false),
            (Divergence::Unknown, "unknown", false),
        ];
        for (d, word, trust) in all {
            assert_eq!(d.as_str(), word);
            assert_eq!(d.trustworthy(), trust, "{word}");
            assert_eq!(
                serde_json::to_value(d).expect("json"),
                serde_json::json!(word)
            );
        }
        // and the two tiers, which have no third
        assert_eq!(
            serde_json::to_value(Match::SameWorktreeSameBranch).expect("json"),
            serde_json::json!("same_worktree_same_branch")
        );
        assert_eq!(
            serde_json::to_value(Match::SameBranch).expect("json"),
            serde_json::json!("same_branch")
        );
    }

    // ------------------------------------------------------------------ divergence

    /// The four reachable answers and the two ways of reaching `unknown`. `unknown` is the
    /// one that matters: a reader told `exact` because git could not answer would act on a
    /// record nothing had checked.
    #[test]
    fn divergence_answers_unknown_rather_than_guessing_when_git_cannot_say() {
        let repo = Repo::new();
        let first = repo.head();
        repo.git(&["commit", "-q", "--allow-empty", "-m", "two"]);
        let second = repo.head();

        assert_eq!(
            divergence(repo.root(), &first, Some(&first)),
            Divergence::Exact
        );
        assert_eq!(
            divergence(repo.root(), &first, Some(&second)),
            Divergence::Advanced
        );
        // a commit this history has never seen is not a yes, and not an unknown either
        assert_eq!(
            divergence(
                repo.root(),
                "0000000000000000000000000000000000000000",
                Some(&second)
            ),
            Divergence::Diverged
        );
        // this checkout has no head at all
        assert_eq!(divergence(repo.root(), &first, None), Divergence::Unknown);
        // the record names no commit
        assert_eq!(
            divergence(repo.root(), "", Some(&second)),
            Divergence::Unknown
        );
    }

    // ------------------------------------------------------------------ reading

    /// A record that cannot be read yields `None` and is counted, never a failed call. The
    /// three ways a file in that directory can fail to be one are all here, because the
    /// resolver's `skipped` count is what becomes the finding a reader is shown.
    #[test]
    fn a_file_that_is_not_a_record_is_none_rather_than_an_error() {
        let repo = Repo::new();
        assert!(front(&repo.root().join("nothing.md")).is_none());
        assert!(front(&repo.write("plain.md", "just prose, no front matter\n")).is_none());
        assert!(front(&repo.write("broken.md", "---\n: : :\n---\nbody\n")).is_none());
        let good = repo.write(
            "good.md",
            &record("2026-01-01T00:00:00Z", "abc", "main", "/w", "Go"),
        );
        let f = front(&good).expect("a record");
        assert_eq!(f.get("branch").map(String::as_str), Some("main"));
        // a list is not a scalar and is dropped rather than stringified
        assert!(!f.contains_key("changed_files"));
    }

    /// The two records of the local half that are not Markdown carry no `---` fences, so
    /// they are read whole. Reading one with the front-matter splitter finds nothing and
    /// reports an absent episode in a checkout that has one.
    #[test]
    fn a_fenceless_document_is_read_whole_and_a_missing_one_is_none() {
        let repo = Repo::new();
        assert!(document(&repo.root().join("nothing.yaml")).is_none());
        let p = repo.write(
            "session.yaml",
            "session_id: s-1\nstarted_at: t\nowner: \"me\"\n",
        );
        let d = document(&p).expect("a document");
        assert_eq!(d.get("session_id").map(String::as_str), Some("s-1"));
        assert_eq!(d.get("owner").map(String::as_str), Some("me"));
        // and one that does not parse is absence, not a failure
        assert!(document(&repo.write("bad.yaml", "\t: [unclosed\n")).is_none());
    }

    /// The section a resuming worker acts on: found, absent, surrounded by blank lines, and
    /// in a file that is not there. Trimming both ends matters because the value is quoted
    /// into a briefing, where a leading blank line is a paragraph break that changes what it
    /// looks like it is saying.
    #[test]
    fn one_section_is_lifted_whole_and_trimmed_at_both_ends() {
        let repo = Repo::new();
        assert_eq!(section(&repo.root().join("nothing.md"), "Next Action"), "");
        let p = repo.write(
            "r.md",
            "# Objective\n\nSomething.\n\n# Next Action\n\n\nDo the thing.\nThen the other.\n\n\n# After\n\nNo.\n",
        );
        assert_eq!(section(&p, "Next Action"), "Do the thing.\nThen the other.");
        assert_eq!(section(&p, "Objective"), "Something.");
        assert_eq!(section(&p, "Nothing Like This"), "");
        // a heading with trailing whitespace is the same heading
        let q = repo.write("s.md", "# Next Action   \n\nHere.\n");
        assert_eq!(section(&q, "Next Action"), "Here.");
        // a section that is only blank lines trims to nothing rather than to whitespace
        let e = repo.write("e.md", "# Next Action\n\n\n\n");
        assert_eq!(section(&e, "Next Action"), "");
    }

    /// The store's own documentation declares its line format inside an HTML comment. A
    /// reader that does not skip the comment reports a blocker that does not exist in every
    /// fresh checkout — and a phantom blocker is worse than a missed one, because it refuses
    /// work nobody can unblock.
    #[test]
    fn only_unresolved_entries_outside_the_comment_are_blockers() {
        let repo = Repo::new();
        assert!(blockers(&repo.root().join("nothing.md")).is_empty());
        let p = repo.write(
            "q.md",
            "# Open questions\n\n<!--\n- [unresolved] the template's own example\n-->\n\n\
             - [unresolved] can we ship it\n- [resolved] we could not\n- [unresolved]   spaced   \n\
             not a list line at all\n",
        );
        assert_eq!(
            blockers(&p),
            vec!["can we ship it".to_string(), "spaced".to_string()]
        );
        // a comment opened and closed on one line does not swallow the rest of the file
        let q = repo.write("q2.md", "<!-- a note -->\n- [unresolved] still counted\n");
        assert_eq!(blockers(&q), vec!["still counted".to_string()]);
    }

    /// How many records a directory holds, and none when there is no directory. Not "how
    /// many files": the store legitimately holds its own README.md and a staging file.
    #[test]
    fn only_markdown_is_counted_and_a_missing_directory_is_zero() {
        let repo = Repo::new();
        assert_eq!(count(&repo.root().join("nowhere")), 0);
        repo.write("d/a.md", "x");
        repo.write("d/b.md", "x");
        repo.write("d/c.yaml", "x");
        repo.write("d/.tmp.abcdef", "x");
        assert_eq!(count(&repo.root().join("d")), 2);
    }

    // ------------------------------------------------------------------ the resolver

    /// Two tiers and no third, and the ordering inside a tier is by the time the record
    /// asserts. The record from another worktree on this branch is offered *only* when this
    /// worktree has none: a briefing that is quietly about somebody else's work is worse
    /// than no briefing, and the tier is what the reader is told so it can weigh it.
    #[test]
    fn the_nearer_tier_wins_and_the_newer_record_wins_inside_one() {
        let repo = Repo::new();
        let head = repo.head();
        let root = repo.root().to_string_lossy().to_string();
        let dir = repo.root().join("h");

        // nothing at all, and not even a directory
        let (none, skipped) = resolve(repo.root(), &dir, "main", Some(&head));
        assert!(none.is_none() && skipped == 0);

        repo.write(
            "h/elsewhere.md",
            &record("2026-01-03T00:00:00Z", &head, "main", "/other", "Theirs"),
        );
        let (r, _) = resolve(repo.root(), &dir, "main", Some(&head));
        let r = r.expect("the other worktree's record, for want of one here");
        assert_eq!(r.matched, Match::SameBranch);
        assert_eq!(r.next_action, "Theirs");

        // ...and it stops being offered the moment this worktree has one, even an older one
        repo.write(
            "h/mine.md",
            &record("2026-01-01T00:00:00Z", &head, "main", &root, "Mine"),
        );
        let (r, _) = resolve(repo.root(), &dir, "main", Some(&head));
        let r = r.expect("this worktree's record");
        assert_eq!(r.matched, Match::SameWorktreeSameBranch);
        assert_eq!(r.next_action, "Mine");
        assert_eq!(r.divergence, Divergence::Exact);
        assert_eq!(r.task_id, "t-1");
        assert_eq!(r.working_tree, "clean");
        assert!(
            r.path.starts_with("h/"),
            "the path is repository-relative: {}",
            r.path
        );

        // newer wins inside the tier
        repo.write(
            "h/newer.md",
            &record("2026-01-02T00:00:00Z", &head, "main", &root, "Newer"),
        );
        let (r, _) = resolve(repo.root(), &dir, "main", Some(&head));
        assert_eq!(r.expect("a record").next_action, "Newer");

        // another branch is never offered, however new
        repo.write(
            "h/other-branch.md",
            &record("2026-09-09T00:00:00Z", &head, "topic", &root, "No"),
        );
        let (r, _) = resolve(repo.root(), &dir, "main", Some(&head));
        assert_eq!(r.expect("a record").next_action, "Newer");
    }

    /// A detached HEAD has no branch to match on, so tier 1 is unreachable: "the same
    /// branch" is not a relation a detached checkout has with anything.
    #[test]
    fn a_detached_checkout_is_offered_no_other_worktrees_record() {
        let repo = Repo::new();
        let head = repo.head();
        repo.write(
            "h/theirs.md",
            &record("2026-01-01T00:00:00Z", &head, "DETACHED", "/other", "No"),
        );
        let (r, skipped) = resolve(repo.root(), &repo.root().join("h"), "DETACHED", Some(&head));
        assert!(
            r.is_none(),
            "a detached checkout was handed another worktree's record"
        );
        assert_eq!(
            skipped, 0,
            "a record that does not match is not a record that is broken"
        );
    }

    /// Each way a file in the store can fail to be a record, counted rather than ignored —
    /// the count is what becomes the finding `majordomus doctor` is recommended from. A
    /// version this executable does not read is in that list: it parsed, and reading its
    /// fields under a contract that no longer describes them is the silent accept that is
    /// the twin of a silent skip.
    #[test]
    fn every_unreadable_file_is_counted_and_a_future_version_is_one_of_them() {
        let repo = Repo::new();
        let head = repo.head();
        let root = repo.root().to_string_lossy().to_string();
        repo.write("h/prose.md", "no front matter here\n");
        repo.write(
            "h/no-created.md",
            "---\nschema_version: 1\nhead: abc\n---\n",
        );
        repo.write(
            "h/no-head.md",
            "---\nschema_version: 1\ncreated_at: x\n---\n",
        );
        repo.write(
            "h/future.md",
            &record("2026-01-01T00:00:00Z", &head, "main", &root, "No")
                .replace("schema_version: 1", "schema_version: 2"),
        );
        // not a record at all, and not counted as a broken one either
        repo.write("h/README.md.yaml", "x\n");
        let (r, skipped) = resolve(repo.root(), &repo.root().join("h"), "main", Some(&head));
        assert!(r.is_none(), "an unreadable file was offered as a record");
        assert_eq!(skipped, 4);
    }

    /// A record that names no task is a record of work done outside one, and the field says
    /// `none` rather than being absent. Work outside a task is the ordinary case since
    /// ADR 0041 — a checkpoint written when no task is open records `task: none` — so the
    /// fallback here is on the normal path and not an edge.
    #[test]
    fn a_record_with_no_task_says_none_rather_than_leaving_the_field_empty() {
        let repo = Repo::new();
        let head = repo.head();
        let root = repo.root().to_string_lossy().to_string();
        let body = record("2026-01-01T00:00:00Z", &head, "main", &root, "Go")
            .replace("task_id: t-1\n", "");
        repo.write("h/no-task.md", &body);
        let (r, skipped) = resolve(repo.root(), &repo.root().join("h"), "main", Some(&head));
        let r = r.expect("a record without a task is still a record");
        assert_eq!(r.task_id, "none");
        assert_eq!(skipped, 0);
    }

    /// Two records written in the same second resolve the same way twice. The directory
    /// walk is sorted for exactly this: `read_dir` order is the filesystem's, and a briefing
    /// that changes between two runs over an unchanged store is one nobody can reason about.
    #[test]
    fn two_records_in_one_second_resolve_the_same_way_every_time() {
        let repo = Repo::new();
        let head = repo.head();
        let root = repo.root().to_string_lossy().to_string();
        for name in ["b", "a", "c"] {
            repo.write(
                &format!("h/{name}.md"),
                &record("2026-01-01T00:00:00Z", &head, "main", &root, name),
            );
        }
        let first = resolve(repo.root(), &repo.root().join("h"), "main", Some(&head)).0;
        for _ in 0..5 {
            let again = resolve(repo.root(), &repo.root().join("h"), "main", Some(&head)).0;
            assert_eq!(first, again);
        }
    }

    // ------------------------------------------------------------------ the active task

    /// The task is read whole rather than through the scalar flattening, because `scope` and
    /// `requires` are lists and a task without its scope is a task whose claim nobody can
    /// check. A record with no id is no task: several things create the file before anything
    /// is in it.
    #[test]
    fn a_task_is_read_with_its_lists_and_a_file_without_an_id_is_no_task() {
        let repo = Repo::new();
        assert!(read_task(&repo.root().join("nothing.yaml")).is_none());
        assert!(read_task(&repo.write("empty.yaml", "")).is_none());
        assert!(read_task(&repo.write("broken.yaml", "\t: [unclosed\n")).is_none());
        assert!(read_task(&repo.write("idless.yaml", "task: something\nprofile: x\n")).is_none());
        let p = repo.write(
            "current.yaml",
            "id: t-1\ntask: do it\nprofile: implementation\noutcome: active\n\
             started_at: 2026-01-01T00:00:00Z\nhead: abc\nscope:\n  - lib\n  - test\n\
             requires:\n  - committed\n",
        );
        let t = read_task(&p).expect("a task");
        assert_eq!(t.id, "t-1");
        assert_eq!(t.outcome, "active");
        assert_eq!(t.scope, vec!["lib".to_string(), "test".to_string()]);
        assert_eq!(t.requires, vec!["committed".to_string()]);
        assert_eq!(t.head, "abc");
        // a task that declares neither is a task with empty lists, never a missing one
        let q = repo.write("bare.yaml", "id: t-2\n");
        let t = read_task(&q).expect("a task");
        assert!(t.scope.is_empty() && t.requires.is_empty() && t.profile.is_empty());
    }

    /// The declaration is the only place the id, the tool name, the resource URI and the
    /// route exist. A refactor that dropped one of them would still compile, and every
    /// suite that tests the report itself would still pass; this is the assertion that
    /// would not — which is what `project.rust-command-tested-in-file` asks of a command.
    #[test]
    fn the_declaration_yields_the_identity_and_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "continuity");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["continuity.state"]);

        let c = &m.capabilities[0].capability;
        let mcp = c.exposure.mcp.as_ref().expect("an MCP projection");
        assert_eq!(mcp.tool.as_deref(), Some("majordomus_continuity"));
        assert_eq!(
            mcp.resource.as_ref().map(|r| r.uri.as_str()),
            Some(CONTINUITY_URI)
        );
        assert_eq!(
            c.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/continuity")
        );
        // read-only, and the registry refuses an executable capability that is not
        assert!(c.kind.is_read_only() && c.kind.is_executable());
        // and the module stamps its own namespace on what it composes
        assert!(ids.iter().all(|id| id.starts_with("continuity.")));
    }
}
