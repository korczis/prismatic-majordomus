//! The `lifecycle` module: the episode lifecycle as something a person can watch.
//!
//! The module is named for the lifecycle rather than for the session because `session` is
//! already a kind of the layer — the closed records under the sessions section — and the
//! registry composes one module per declarative kind. A capability module that took the
//! same namespace would be composed twice and the registry refuses to build, which is the
//! right answer: two different things called `session` in one namespace is exactly the
//! ambiguity that refusal exists to catch.
//!
//! [`super::continuity`] answers one question well — *what would a worker resuming in this
//! checkout be handed?* — and it answers it about **one** episode, the one
//! `state/session-current.yaml` points at. That is the right answer for a briefing and the
//! wrong answer for an operator, because the pointer is a symlink that the most recent
//! `session start` re-aims: on 2026-09-11 this repository held five open episodes in one
//! checkout and `continuity.state` could name exactly one of them. Four workers were
//! invisible to every surface the tool has, and the episode that never closes is the one
//! nobody can see.
//!
//! So this module reads the store rather than the pointer, and it reports the things a
//! subsystem needs somebody to look at:
//!
//! - **`lifecycle.episodes`** — every open episode in the local store, which of them this checkout
//!   points at, which belong to another worktree, and what the ledger last saw each do.
//! - **`lifecycle.recovery`** — the episodes that can no longer close themselves, the temporary
//!   files a killed close left in the tracked directory, whether the pointer has been
//!   migrated to the per-provider layout, and the arithmetic ADR 0041 asks for: how many
//!   `session.started` events the ledger holds against how many `session.closed`.
//! - **`lifecycle.runtime`** — the commit this process is serving answers about, against the commit
//!   the repository is on right now. A long-lived server that built its index once and held
//!   it served a six-hour-old HEAD from the API, from MCP and from this Cockpit with nothing
//!   saying the picture was old (`fix/server-sees-the-current-tree`). Whether that is still
//!   possible is a fact this page should state rather than a fact a reader should assume.
//! - **`lifecycle.providers`** — which lifecycle events each provider's adapter declares, whether it
//!   can capture prompts, and which of this repository's enforcement entries are actually
//!   wired to its hook. Declared, never inferred from the presence of a file.
//! - **`lifecycle.closed`** — the tracked durable projection under the layer's sessions section: the
//!   only half of this subsystem that survives a clone.
//!
//! **Read, never written.** Like `continuity`, this module is one of the lifecycle's
//! readers; the writer is the shell tool. A second writer would be a second account of
//! events the ledger already holds, which is what that design refuses.
//!
//! **Served, never published.** Everything under `.ai/local/` names this machine, so none of
//! it is projected into `docs/generated/` or `site/` — the claim `local-state-ignored` owns
//! that, over the whole local half (ADR 0014). The half that belongs to these five is that
//! none of them declares a command line, because a command line is how a value reaches a
//! script, a log and eventually a commit; `test/cases/171` asserts exactly that.
//!
//! **No thresholds and no clock.** Nothing here decides that a record is old. Age is
//! `session.freshness` in the policy and it is [`super::continuity`]'s to judge; a second
//! engine for it in this file would be the second source of truth ADR 0041 names. What this
//! module reports is what it can observe without an opinion: a timestamp as recorded, a
//! count, a path that exists or does not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::{self, GitState};
use crate::{capability, module};

use super::continuity::document;
use super::{get, Empty};

/// The URI under which `lifecycle.episodes` is read as an MCP resource.
pub const EPISODES_URI: &str = "majordomus://lifecycle/episodes";
/// The URI under which `lifecycle.recovery` is read as an MCP resource.
pub const RECOVERY_URI: &str = "majordomus://lifecycle/recovery";

/// The local half of the layer, relative to the repository root. The same constant
/// `continuity` carries, and for the same reason: the shell tool decides where its state
/// lives and a second opinion about the path would be a second source of truth for the one
/// thing both halves must agree on.
const STATE_DIR: &str = ".ai/local/state";
/// The open-episode store, one file per provider session (`lib/session.sh`).
const OPEN_DIR: &str = "sessions-open";
/// The pointer this checkout follows, a symlink into [`OPEN_DIR`] since the per-provider
/// layout landed.
const POINTER: &str = "session-current.yaml";
/// The append-only ledger, the only account of events either half keeps.
const LEDGER: &str = "ledger.jsonl";
/// How many closed records [`closed`] carries in full. The total is always exact; the
/// window exists because the tracked directory grows without bound and a Cockpit card is
/// not a listing page.
const CLOSED_WINDOW: usize = 20;

// --------------------------------------------------------------------- open episodes

/// Where an open episode stands, as far as the repository can observe it.
///
/// It is worth being precise about what is *not* here. Whether the provider that opened an
/// episode is still attached to its conversation is not a fact of this repository: the
/// episode file records no pid, the peer board records no episode id, and a client that
/// exits without firing its end event leaves a file identical to one a live worker is using.
/// Guessing would produce exactly the confident-and-wrong answer this subsystem exists to
/// avoid, so the vocabulary below is the observable one — where the file is, whose worktree
/// it names, and whether the pointer follows it — and `note` always says which.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeStanding {
    /// This checkout's pointer resolves to it. `continuity.state` is about this one.
    Current,
    /// Open, and it names this worktree, but the pointer names a different episode. Two
    /// windows of one provider on one worktree are two workers, and only one is pointed at.
    Open,
    /// It names another worktree of this repository. Reported, never treated as this
    /// checkout's — a record about somebody else's work quietly becoming yours is the
    /// failure the two-tier resolution rule exists to prevent.
    Foreign,
    /// The worktree it names is gone from disk. Nothing can close it: the close path
    /// composes the record from git in the worktree the episode opened in.
    Stranded,
}

impl EpisodeStanding {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::lifecycle::EpisodeStanding;
    /// assert_eq!(EpisodeStanding::Stranded.as_str(), "stranded");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EpisodeStanding::Current => "current",
            EpisodeStanding::Open => "open",
            EpisodeStanding::Foreign => "foreign",
            EpisodeStanding::Stranded => "stranded",
        }
    }
}

/// One open episode, as its file records it and as the ledger has seen it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Episode {
    /// The episode id the ledger stamps on every line this worker wrote.
    pub session_id: String,
    /// Repository-relative path of the open record. The body is at the path.
    pub path: String,
    /// The provider whose event opened it, when one did. Empty for an episode opened by
    /// hand, which is the one episode no provider session owns.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// That provider's own session identity: the file's stem, and the string the prompt
    /// archive stamps on the same worker's records.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
    /// Who opened it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    /// The worker identity, when one was supplied. Never inferred.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker: String,
    /// The worktree it opened in, absolute, as recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worktree: String,
    /// The branch it opened on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it opened at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_head: String,
    /// When it opened, as recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
    /// Where it stands.
    pub standing: EpisodeStanding,
    /// How many ledger lines carry this episode id. Zero for an episode that opened and did
    /// nothing, which is ordinary and not a fault.
    pub events: usize,
    /// The timestamp of the newest ledger line carrying this episode id, as recorded.
    /// Empty when it has written none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_activity: String,
    /// That line's event name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_event: String,
    /// The tasks this episode's ledger lines name, in the order it first touched each.
    ///
    /// A relation, not a container. An episode may touch three tasks or none, and a task may
    /// span three episodes; the two are joined here by what the ledger recorded rather than
    /// by either one owning the other. An episode with no task is ordinary — work outside a
    /// task is permitted — and it is emphatically not a reason for the episode's own records
    /// to go unwritten, which is the premise ADR 0041 removes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    /// Why it stands where it does, in words. Never empty: a standing a reader cannot act
    /// on is the failure this subsystem is being corrected for.
    pub note: String,
}

/// Every open episode of this checkout's store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Episodes {
    /// Whether the open-episode store exists. False in a fresh clone, which is not a fault.
    pub present: bool,
    /// The store, repository-relative.
    pub store: String,
    /// This worktree, absolute. Every standing above is decided against it.
    pub worktree: String,
    /// The episode id the pointer resolves to, when it resolves to one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub current: String,
    /// Every open episode, newest first by recorded start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<Episode>,
    /// What a reader should know before trusting the above: a file that does not parse, a
    /// pointer that resolves to nothing. Empty is the good case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- recovery

/// The layout of the pointer, which is how far the per-provider migration has got here.
///
/// Before the episode was keyed by its provider session, `session-current.yaml` *was* the
/// open record: one regular file, one episode per checkout. Seven concurrent sessions on
/// 2026-09-09 produced one record between them under that layout. A regular file here today
/// is a checkout that has not run the migration, and its episodes are being overwritten.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PointerLayout {
    /// A symlink into `sessions-open/`: the per-provider layout.
    Pointer,
    /// A regular file: the pre-migration layout, one episode per checkout.
    Inline,
    /// Nothing is there. No episode is open here, which is not a fault.
    Absent,
}

impl PointerLayout {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::lifecycle::PointerLayout;
    /// assert_eq!(PointerLayout::Inline.as_str(), "inline");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            PointerLayout::Pointer => "pointer",
            PointerLayout::Inline => "inline",
            PointerLayout::Absent => "absent",
        }
    }
}

/// What the pointer is and whether it leads anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pointer {
    /// The layout.
    pub layout: PointerLayout,
    /// What the symlink names, as written, when it is one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// Whether reading through it yields an episode.
    pub resolves: bool,
    /// The episode it resolves to.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// What that means, in words.
    pub note: String,
}

/// An open episode that cannot close itself, and what to do about it.
///
/// The remedy here is the mechanical one — remove the file — because it is correct in every
/// checkout and needs nothing that is not already installed. `lib/recover.sh` on
/// `feature/session-store-recovery` is the same three degradations seen from the writing
/// side: it classifies each candidate by reading it, prints the evidence before acting, and
/// closes a stranded episode into a real record rather than deleting it. When that lands,
/// the remedy a finding carries should become `majordomus recover episodes` and the two
/// detections should become one — a reader and a writer that each decide for themselves
/// what "stranded" means are two definitions, and this repository has been bitten by that
/// shape often enough to name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stranded {
    /// The episode id.
    pub session_id: String,
    /// The open record, repository-relative.
    pub path: String,
    /// Why it is stranded.
    pub reason: String,
    /// The command that clears it. A finding with no remedy is a complaint.
    pub remedy: String,
}

/// A temporary file left in a record directory by a close that did not finish.
///
/// The close path writes the durable record to `.tmp.XXXX` in the destination directory and
/// renames it, so that no reader ever sees half a record. A process killed between the two
/// leaves the temporary file behind, where it is untracked, invisible to every reader of the
/// section, and — because the directory is tracked — offered to the next `git add .`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Orphan {
    /// Repository-relative path.
    pub path: String,
    /// Its size in bytes, which is how a reader tells an empty stub from a lost record.
    pub bytes: u64,
}

/// What the ledger says about episode lifecycle against what the store holds.
///
/// The invariant ADR 0041 asks for, stated as arithmetic a person can check: every episode
/// that started either closed or is still open. When `started` exceeds `closed + open`, the
/// difference is episodes whose close was never recorded — the shape of the outage that ADR
/// describes, where events kept arriving and the records they should have produced did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Balance {
    /// `session.started` lines in the ledger.
    pub started: usize,
    /// `session.closed` lines.
    pub closed: usize,
    /// Open records in the store.
    pub open: usize,
    /// `started - closed - open`. Zero is the good case; negative means the ledger was
    /// rotated under the records, which is ordinary.
    pub unaccounted: i64,
    /// Whether the three agree.
    pub agrees: bool,
    /// What the numbers mean, in words.
    pub note: String,
}

/// What this checkout's session store needs somebody to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recovery {
    /// Whether the local half exists at all.
    pub present: bool,
    /// The pointer and its layout.
    pub pointer: Pointer,
    /// Episodes that can no longer close themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stranded: Vec<Stranded>,
    /// Temporary files left in the tracked sessions section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orphans: Vec<Orphan>,
    /// The started/closed/open arithmetic.
    pub balance: Balance,
    /// Everything above, said once, for a reader that wants the summary. Empty is the good
    /// case and means there is nothing to recover.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- runtime

/// The commit this process answers about, against the commit the repository is on.
///
/// Both halves are read here and neither is assumed. `served` is what the index this
/// process is holding was built from; `repository` is what `git` says now, on this call. A
/// server that follows the repository reports them equal and pays one `git` invocation for
/// the proof; a server that froze its index at start-up reports them different, which is
/// the only way a reader could ever have found that out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeView {
    /// The repository root this process is serving.
    pub root: String,
    /// The commit the served index was built at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub served_head: String,
    /// Its branch, or `DETACHED`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub served_branch: String,
    /// Whether the working tree was clean when the index was built.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub served_working_tree: String,
    /// The commit the repository is on, read on this call.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repository_head: String,
    /// Its branch now.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repository_branch: String,
    /// The working tree now.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repository_working_tree: String,
    /// Whether the two agree on the commit and the branch.
    pub agree: bool,
    /// How many objects the served index holds, and whether it was degraded.
    pub objects: usize,
    /// `ok` or `degraded`.
    pub index_state: String,
    /// What the comparison means, and what to do when it disagrees.
    pub note: String,
}

// --------------------------------------------------------------------- providers

/// One provider's lifecycle surface: what its adapter declares it can do, and what this
/// repository has wired to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderLifecycle {
    /// The provider id: the bootstrap template's file stem.
    pub id: String,
    /// The name a person knows it by.
    pub title: String,
    /// The lifecycle events its adapter declares, in the provider's own vocabulary
    /// (`SessionStart`, `SessionEnd`, `PreCompact`). Empty means the tool ships no
    /// lifecycle adapter for it: such a provider loses the automation and none of the
    /// model, because every command remains the same.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle: Vec<String>,
    /// Whether its adapter can archive the worker's prompts.
    pub prompt_capture: bool,
    /// The file it reads a project-scoped MCP client configuration from, when it has one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_config: String,
    /// The enforcement entries of *this repository's* policy that name this provider's hook
    /// in `wired_by`. Declared wiring, not a file on disk: whether a shim is installed is a
    /// question for `majordomus capture status`, which is the command that owns it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wired: Vec<String>,
    /// What the two together mean.
    pub note: String,
}

/// Every provider the distribution ships an adapter for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderLifecycles {
    /// The providers, in declaration order.
    pub providers: Vec<ProviderLifecycle>,
    /// How many of them declare a lifecycle adapter.
    pub with_lifecycle: usize,
    /// What a reader should know: a provider this repository wires but the distribution
    /// declares no lifecycle for, and the like.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- closed

/// One closed episode of the tracked, durable projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClosedSession {
    /// The episode id.
    pub session_id: String,
    /// `majordomus://session/<identity>`, for the object page.
    pub uri: String,
    /// Repository-relative path.
    pub path: String,
    /// When it closed, as recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    /// The branch it closed on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it closed at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
    /// The typed outcome the close recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub outcome: String,
    /// Its title, when the record carries one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
}

/// The tracked half of the subsystem: the records a clone receives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClosedSessions {
    /// How many closed records the index holds. Exact.
    pub total: usize,
    /// How many of them closed on the branch this checkout is on.
    pub on_this_branch: usize,
    /// The newest [`CLOSED_WINDOW`], newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub newest: Vec<ClosedSession>,
    /// How many are carried in `newest`, against the total.
    pub window: usize,
}

// --------------------------------------------------------------------- reading

/// One ledger line, reduced to the four fields this module reads.
#[derive(Debug, Clone, Default)]
struct Line {
    /// The episode that wrote it, empty when the line belongs to none.
    session: String,
    /// The event name.
    event: String,
    /// When it was written, as recorded.
    ts: String,
    /// The task it belonged to, when it belonged to one. This is the relation ADR 0041
    /// makes optional in the other direction: an episode names the tasks it touched, and
    /// no task decides whether the episode's own records are written.
    task_id: String,
}

/// Every ledger line, oldest first.
///
/// One pass over the file, and a line that does not parse is skipped rather than failing the
/// call: the ledger is append-only and written by another process, and a reader that refused
/// the whole history because one line was truncated by a killed write would take the
/// subsystem's only account of itself away at exactly the moment it is needed.
fn ledger_lines(path: &Path) -> Vec<Line> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let field = |v: &serde_json::Value, k: &str| -> String {
        v.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string()
    };
    text.lines()
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            Some(Line {
                session: field(&v, "session"),
                event: field(&v, "event"),
                ts: field(&v, "ts"),
                task_id: field(&v, "task_id"),
            })
        })
        .collect()
}

/// The repository-relative form of a path under `root`, or the path as given.
fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

/// The episode id the pointer resolves to, and how it is laid out.
fn pointer(dir: &Path) -> (Pointer, String) {
    let path = dir.join(POINTER);
    let meta = std::fs::symlink_metadata(&path).ok();
    let layout = match &meta {
        None => PointerLayout::Absent,
        Some(m) if m.file_type().is_symlink() => PointerLayout::Pointer,
        Some(_) => PointerLayout::Inline,
    };
    let target = std::fs::read_link(&path)
        .map(|t| t.to_string_lossy().into_owned())
        .unwrap_or_default();
    let session_id = document(&path)
        .and_then(|f| f.get("session_id").cloned())
        .unwrap_or_default();
    let resolves = !session_id.is_empty();
    let note = match layout {
        PointerLayout::Absent => {
            "No episode is open in this checkout. The provider's start event opens one."
                .to_string()
        }
        PointerLayout::Inline => format!(
            "`{POINTER}` is a regular file, which is the layout that existed before an episode was keyed by its provider session. Under it a second start overwrites the first, and two windows of one provider share one record. `majordomus session start` rewrites it as a pointer."
        ),
        PointerLayout::Pointer if resolves => format!(
            "`{POINTER}` points into `{OPEN_DIR}/`, which is the per-provider layout: each window's start event keeps its own episode."
        ),
        PointerLayout::Pointer => format!(
            "`{POINTER}` points at `{target}`, and reading through it yields no episode. The episode it named was removed without the pointer being re-aimed; `majordomus session status` reports what is actually open."
        ),
    };
    (
        Pointer {
            layout,
            target,
            resolves,
            session_id: session_id.clone(),
            note,
        },
        session_id,
    )
}

/// Every open record in the store, as `(path, fields)`, sorted by file name so that the
/// answer does not depend on the order the filesystem happened to hand them back.
fn open_records(dir: &Path) -> (Vec<(PathBuf, BTreeMap<String, String>)>, Vec<String>) {
    let mut out = Vec::new();
    let mut findings = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (out, findings);
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    paths.sort();
    for p in paths {
        match document(&p) {
            Some(f) if f.contains_key("session_id") => out.push((p, f)),
            _ => findings.push(format!(
                "the open record {} does not parse as an episode and was skipped; nothing about it is counted below",
                p.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    (out, findings)
}

// --------------------------------------------------------------------- handlers

fn episodes(ctx: &Context, _: Empty) -> Result<Episodes, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = root.join(STATE_DIR);
    let store = dir.join(OPEN_DIR);

    let (p, current) = pointer(&dir);
    let (records, mut findings) = open_records(&store);
    if !p.resolves && p.layout != PointerLayout::Absent {
        findings.push(p.note.clone());
    }
    let ledger = ledger_lines(&dir.join(LEDGER));

    let here = root.to_string_lossy().into_owned();
    let mut episodes: Vec<Episode> = records
        .into_iter()
        .map(|(path, f)| {
            let get = |k: &str| f.get(k).cloned().unwrap_or_default();
            let session_id = get("session_id");
            let worktree = get("worktree");
            let mine = worktree.is_empty() || worktree == here;
            let exists = worktree.is_empty() || Path::new(&worktree).is_dir();
            let standing = if !exists {
                EpisodeStanding::Stranded
            } else if !mine {
                EpisodeStanding::Foreign
            } else if session_id == current {
                EpisodeStanding::Current
            } else {
                EpisodeStanding::Open
            };
            let note = match standing {
                EpisodeStanding::Current => format!(
                    "This checkout's `{POINTER}` resolves to this episode, so it is the one `continuity.state` and the briefing are about."
                ),
                EpisodeStanding::Open => "Open in this worktree, and the pointer names a different episode. Its ledger lines are stamped with its own id; only the briefing follows the pointer.".to_string(),
                EpisodeStanding::Foreign => format!(
                    "Opened in {worktree}, not here. It is listed because it shares this repository's store, and nothing about it is about the work in this checkout."
                ),
                EpisodeStanding::Stranded => format!(
                    "The worktree it opened in, {worktree}, is gone from disk. The close path composes its record from git there, so nothing can close it in place."
                ),
            };
            let mut events = 0usize;
            let mut last_activity = String::new();
            let mut last_event = String::new();
            let mut tasks: Vec<String> = Vec::new();
            for l in &ledger {
                if l.session == session_id {
                    events += 1;
                    if l.ts.as_str() >= last_activity.as_str() {
                        last_activity.clone_from(&l.ts);
                        last_event.clone_from(&l.event);
                    }
                    if !l.task_id.is_empty() && !tasks.contains(&l.task_id) {
                        tasks.push(l.task_id.clone());
                    }
                }
            }
            Episode {
                session_id,
                path: relative(&root, &path),
                provider: get("provider"),
                provider_session: get("provider_session"),
                owner: get("owner"),
                worker: get("worker"),
                worktree,
                branch: get("branch"),
                start_head: get("start_head"),
                started_at: get("started_at"),
                standing,
                events,
                last_activity,
                last_event,
                tasks,
                note,
            }
        })
        .collect();
    // Newest first by what the record asserts, never by file modification time: mtime does
    // not survive a clone and is not the time the record claims.
    episodes.sort_by(|a, b| {
        b.started_at
            .cmp(&a.started_at)
            .then_with(|| a.session_id.cmp(&b.session_id))
    });

    if episodes.len() > 1 {
        findings.push(format!(
            "{} episodes are open in this store. That is ordinary — one per provider window — and it is also the state in which `continuity.state` can name only the one the pointer follows.",
            episodes.len()
        ));
    }

    Ok(Episodes {
        present: store.is_dir(),
        store: relative(&root, &store),
        worktree: here,
        current,
        episodes,
        findings,
    })
}

fn recovery(ctx: &Context, _: Empty) -> Result<Recovery, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = root.join(STATE_DIR);
    let store = dir.join(OPEN_DIR);

    let (p, _) = pointer(&dir);
    let (records, mut findings) = open_records(&store);
    let ledger = ledger_lines(&dir.join(LEDGER));

    let started = ledger.iter().filter(|l| l.event == "session.started").count();
    let closed_events: Vec<&str> = ledger
        .iter()
        .filter(|l| l.event == "session.closed")
        .map(|l| l.session.as_str())
        .collect();

    let here = root.to_string_lossy().into_owned();
    let mut stranded = Vec::new();
    for (path, f) in &records {
        let session_id = f.get("session_id").cloned().unwrap_or_default();
        let worktree = f.get("worktree").cloned().unwrap_or_default();
        let rel = relative(&root, path);
        if !worktree.is_empty() && !Path::new(&worktree).is_dir() {
            stranded.push(Stranded {
                session_id: session_id.clone(),
                path: rel.clone(),
                reason: format!(
                    "the worktree it opened in, {worktree}, is gone from disk, and the close path composes its record from git there"
                ),
                remedy: format!("rm {rel}"),
            });
            continue;
        }
        if closed_events.contains(&session_id.as_str()) {
            stranded.push(Stranded {
                session_id: session_id.clone(),
                path: rel.clone(),
                reason: "the ledger records this episode as closed, and its open record is still here; the close wrote the durable record and did not remove the file it was composed from".into(),
                remedy: format!("rm {rel}"),
            });
            continue;
        }
        if !worktree.is_empty() && worktree != here && !Path::new(&worktree).join(".git").exists() {
            stranded.push(Stranded {
                session_id,
                path: rel.clone(),
                reason: format!("it names {worktree}, which is not a checkout of this repository"),
                remedy: format!("rm {rel}"),
            });
        }
    }

    // The temporary files of the tracked sessions section. The section's path is the
    // manifest's, never a constant here: a repository that files its sessions elsewhere is
    // still checked, and one that declares no sessions section has nothing to check. The
    // value is already repository-relative and already carries `.ai/` — `Repository::
    // section_path` puts it there — so joining `.ai` again would look under `.ai/.ai/`,
    // find nothing, and report a clean store in a checkout that has orphans.
    let mut orphans = Vec::new();
    if let Some(section) = ctx.index.repository.sections.get("sessions") {
        let sessions = root.join(section);
        if let Ok(entries) = std::fs::read_dir(&sessions) {
            let mut found: Vec<(String, u64)> = entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp."))
                .map(|e| {
                    (
                        relative(&root, &e.path()),
                        e.metadata().map(|m| m.len()).unwrap_or(0),
                    )
                })
                .collect();
            found.sort();
            orphans.extend(found.into_iter().map(|(path, bytes)| Orphan { path, bytes }));
        }
    }

    let open = records.len();
    let closed = closed_events.len();
    let unaccounted = started as i64 - closed as i64 - open as i64;
    let agrees = unaccounted == 0;
    let balance = Balance {
        started,
        closed,
        open,
        unaccounted,
        agrees,
        note: if agrees {
            format!("{started} started, {closed} closed, {open} still open: every episode the ledger has seen is accounted for.")
        } else if unaccounted > 0 {
            format!(
                "{started} started, {closed} closed, {open} still open: {unaccounted} episode(s) started and neither closed nor left a record here. An end event that reached the tool and produced nothing looks exactly like this (ADR 0041)."
            )
        } else {
            format!(
                "{started} started, {closed} closed, {open} still open: more closes and open records than starts, which is what a rotated ledger looks like — `history --rotate` moves the oldest lines into a dated archive and the starts went with them."
            )
        },
    };

    if !stranded.is_empty() {
        findings.push(format!(
            "{} open record(s) cannot be closed by the worker that opened them.",
            stranded.len()
        ));
    }
    if !orphans.is_empty() {
        findings.push(format!(
            "{} temporary file(s) are sitting in the tracked sessions section. They are untracked, no reader of the section sees them, and the next `git add .` would commit them.",
            orphans.len()
        ));
    }
    if p.layout == PointerLayout::Inline {
        findings.push(p.note.clone());
    }
    if !agrees && unaccounted > 0 {
        findings.push(balance.note.clone());
    }

    Ok(Recovery {
        present: dir.is_dir(),
        pointer: p,
        stranded,
        orphans,
        balance,
        findings,
    })
}

fn runtime(ctx: &Context, _: Empty) -> Result<RuntimeView, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let (served_branch, served_head, served_working_tree) = match &ctx.index.repository.git {
        GitState::Available(i) => (
            i.branch.clone().unwrap_or_else(|| "DETACHED".into()),
            i.head.clone().unwrap_or_default(),
            i.working_tree.clone(),
        ),
        GitState::Unavailable { .. } => (String::new(), String::new(), String::new()),
    };
    let (repository_branch, repository_head, repository_working_tree) = match git::inspect(&root) {
        GitState::Available(i) => (
            i.branch.clone().unwrap_or_else(|| "DETACHED".into()),
            i.head.clone().unwrap_or_default(),
            i.working_tree.clone(),
        ),
        GitState::Unavailable { .. } => (String::new(), String::new(), String::new()),
    };
    let agree = served_head == repository_head && served_branch == repository_branch;
    let note = if served_head.is_empty() || repository_head.is_empty() {
        "Git could not be asked on one side of this comparison, so this process says so rather than reporting agreement it did not establish.".to_string()
    } else if agree {
        format!(
            "This process is answering about {}, which is the commit the repository is on. A long-lived server that built its index once and held it would report a commit from whenever it started, from here, from the HTTP API and from MCP alike, with nothing saying the picture was old.",
            &repository_head[..7.min(repository_head.len())]
        )
    } else {
        format!(
            "This process is answering about {} while the repository is on {}. Everything it reports — the index, the objects, this page — is a picture of the older commit. Restart the shared server, or run the command against a fresh process.",
            &served_head[..7.min(served_head.len())],
            &repository_head[..7.min(repository_head.len())]
        )
    };
    Ok(RuntimeView {
        root: root.to_string_lossy().into_owned(),
        served_head,
        served_branch,
        served_working_tree,
        repository_head,
        repository_branch,
        repository_working_tree,
        agree,
        objects: ctx.index.objects.len(),
        index_state: match ctx.index.state {
            crate::index::State::Ok => "ok".into(),
            crate::index::State::Degraded => "degraded".into(),
        },
        note,
    })
}

fn providers(ctx: &Context, _: Empty) -> Result<ProviderLifecycles, CapabilityError> {
    // Which enforcement entries this repository wires to a provider hook. The policy is
    // read as the index holds it — an object of kind `policy` — because that is the reading
    // every other surface uses, and a second parse of the same file is a second answer.
    let policy = ctx.index.objects.iter().find(|o| o.kind == "policy");
    let mut wired: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(entries) = policy
        .and_then(|p| p.metadata.get("enforcement"))
        .and_then(|v| v.as_array())
    {
        for e in entries {
            let Some(w) = e.get("wired_by").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(rest) = w.strip_prefix("provider-hook:") else {
                continue;
            };
            let id = rest.split(':').next().unwrap_or(rest);
            if let Some(name) = e.get("name").and_then(|v| v.as_str()) {
                wired.entry(id.to_string()).or_default().push(name.into());
            }
        }
    }

    let mut findings = Vec::new();
    let providers: Vec<ProviderLifecycle> = ctx
        .index
        .providers
        .providers
        .iter()
        .map(|d| {
            let w = wired.get(&d.id).cloned().unwrap_or_default();
            let note = match (d.lifecycle.is_empty(), w.is_empty()) {
                (true, true) => format!(
                    "The distribution ships no lifecycle adapter for {}. A worker using it loses the automation and none of the model: every command is the same, and running them is again a matter of remembering.",
                    d.title
                ),
                (true, false) => format!(
                    "This repository wires {} to {}'s hook, and the distribution declares no lifecycle events for it. One of the two is wrong.",
                    w.join(", "),
                    d.title
                ),
                (false, true) => format!(
                    "{} declares {}, and this repository's policy wires none of it. The adapter exists; nothing here asks it to run.",
                    d.title,
                    d.lifecycle.join(", ")
                ),
                (false, false) => format!(
                    "{} declares {}, and this repository wires {} to its hook.",
                    d.title,
                    d.lifecycle.join(", "),
                    w.join(", ")
                ),
            };
            if d.lifecycle.is_empty() && !w.is_empty() {
                findings.push(note.clone());
            }
            ProviderLifecycle {
                id: d.id.clone(),
                title: d.title.clone(),
                lifecycle: d.lifecycle.clone(),
                prompt_capture: d.prompt_capture,
                client_config: d.client_config.clone().unwrap_or_default(),
                wired: w,
                note,
            }
        })
        .collect();

    let with_lifecycle = providers.iter().filter(|p| !p.lifecycle.is_empty()).count();
    Ok(ProviderLifecycles {
        providers,
        with_lifecycle,
        findings,
    })
}

fn closed(ctx: &Context, _: Empty) -> Result<ClosedSessions, CapabilityError> {
    let branch = match &ctx.index.repository.git {
        GitState::Available(i) => i.branch.clone().unwrap_or_default(),
        GitState::Unavailable { .. } => String::new(),
    };
    let field = |o: &crate::model::Object, k: &str| {
        o.metadata
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let mut all: Vec<ClosedSession> = ctx
        .index
        .objects
        .iter()
        .filter(|o| o.kind == "session")
        .map(|o| ClosedSession {
            session_id: field(o, "session_id"),
            uri: o.uri.clone(),
            path: o.provenance.path.clone(),
            created_at: field(o, "created_at"),
            branch: field(o, "branch"),
            head: field(o, "head"),
            outcome: field(o, "outcome"),
            title: o.title.clone().unwrap_or_default(),
        })
        .collect();
    all.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    let total = all.len();
    let on_this_branch = if branch.is_empty() {
        0
    } else {
        all.iter().filter(|s| s.branch == branch).count()
    };
    all.truncate(CLOSED_WINDOW);
    Ok(ClosedSessions {
        total,
        on_this_branch,
        window: all.len(),
        newest: all,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "lifecycle",
        title: "Session lifecycle",
        description: "The episode lifecycle as an operator sees it: every open episode of this checkout's store rather than only the one the pointer follows, the episodes that can no longer close themselves, the commit this process is answering about against the commit the repository is on, what each provider's adapter declares it can do, and the tracked records a clone receives. Read from the local half of the layer and from the index; written by nothing here.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "lifecycle.episodes",
                title: "Every open episode",
                description: "Every open episode in this checkout's store, with the provider session that owns it, the worktree and branch it opened on, where it stands, and what the ledger last saw it do. `continuity.state` reports the one episode the pointer resolves to, which is right for a briefing and blind to every other window open on the same worktree.",
                input: Empty,
                output: Episodes,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_lifecycle_episodes".into()),
                        resource: Some(McpResource { uri: EPISODES_URI.into(), name: "lifecycle-episodes".into() }),
                    }),
                    http: get("/api/v1/lifecycle/episodes"),
                    cli: None,
                },
                tags: ["continuity", "session", "episode"],
                // Short-lived for the same reason continuity.state is: another process owns
                // these files, and a reader that cached them for a minute would answer with
                // an episode that had already closed.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: episodes,
            },
            capability! {
                id: "lifecycle.recovery",
                title: "What the store needs somebody to do",
                description: "Open records that can no longer close themselves, temporary files a killed close left in the tracked sessions section, whether the pointer has been migrated to the per-provider layout, and the arithmetic that says whether every episode the ledger has seen start is accounted for. Every finding carries the command that clears it.",
                input: Empty,
                output: Recovery,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_lifecycle_recovery".into()),
                        resource: Some(McpResource { uri: RECOVERY_URI.into(), name: "lifecycle-recovery".into() }),
                    }),
                    http: get("/api/v1/lifecycle/recovery"),
                    cli: None,
                },
                tags: ["continuity", "session", "recovery"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: recovery,
            },
            capability! {
                id: "lifecycle.runtime",
                title: "The commit this process is answering about",
                description: "The commit the served index was built at, against the commit the repository is on, read on this call. A server that froze its index at start-up answers every question about a commit from hours ago with nothing in the answer saying so; this is the reading that can tell.",
                input: Empty,
                output: RuntimeView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure { tool: Some("majordomus_lifecycle_runtime".into()), resource: None }),
                    http: get("/api/v1/lifecycle/runtime"),
                    cli: None,
                },
                tags: ["continuity", "session", "server"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: runtime,
            },
            capability! {
                id: "lifecycle.providers",
                title: "What each provider's adapter declares",
                description: "Per provider: the lifecycle events its adapter declares in the provider's own vocabulary, whether it can archive prompts, and which of this repository's enforcement entries name its hook. Declared in share/providers.yaml and in the policy; never inferred from whether a shim happens to be on disk.",
                input: Empty,
                output: ProviderLifecycles,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure { tool: Some("majordomus_lifecycle_providers".into()), resource: None }),
                    http: get("/api/v1/lifecycle/providers"),
                    cli: None,
                },
                tags: ["continuity", "session", "provider"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: providers,
            },
            capability! {
                id: "lifecycle.closed",
                title: "The records a clone receives",
                description: "The tracked, durable projection of closed episodes: how many there are, how many closed on this branch, and the newest twenty. This is the only half of the subsystem that survives a clone; everything under .ai/local/ names this machine and travels nowhere.",
                input: Empty,
                output: ClosedSessions,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure { tool: Some("majordomus_lifecycle_closed".into()), resource: None }),
                    http: get("/api/v1/lifecycle/closed"),
                    cli: None,
                },
                tags: ["continuity", "session", "record"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(30) },
                handler: closed,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place the ids, the tool names, the resource URIs and the
    /// routes exist. A refactor that dropped one of them would still compile and every test
    /// of the reports themselves would still pass; this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_identities_and_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "lifecycle");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "lifecycle.episodes",
                "lifecycle.recovery",
                "lifecycle.runtime",
                "lifecycle.providers",
                "lifecycle.closed"
            ]
        );
        // every one of them is read-only, and the registry refuses an executable that is not
        for e in &m.capabilities {
            assert!(e.capability.kind.is_read_only() && e.capability.kind.is_executable());
            assert!(e.capability.id.as_str().starts_with("lifecycle."));
        }
        let episodes = &m.capabilities[0].capability;
        assert_eq!(
            episodes.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_lifecycle_episodes")
        );
        assert_eq!(
            episodes.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/lifecycle/episodes")
        );
    }

    /// A standing is a word a person reads, and the four are the whole vocabulary. A fifth
    /// added without a word would serialise as something no surface has a sentence for.
    #[test]
    fn every_standing_has_a_word() {
        for s in [
            EpisodeStanding::Current,
            EpisodeStanding::Open,
            EpisodeStanding::Foreign,
            EpisodeStanding::Stranded,
        ] {
            assert!(!s.as_str().is_empty());
        }
        assert_eq!(EpisodeStanding::Foreign.as_str(), "foreign");
        assert_eq!(PointerLayout::Pointer.as_str(), "pointer");
    }
}
