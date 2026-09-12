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
//!   migrated to the per-provider layout, and the arithmetic ADR 0052 asks for: how many
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
//! engine for it in this file would be the second source of truth ADR 0052 names. What this
//! module reports is what it can observe without an opinion: a timestamp as recorded, a
//! count, a path that exists or does not.
//!
//! # The shape of the module, and the constraint it is under
//!
//! Two of the paragraphs above are properties of the declaration rather than intentions,
//! so they can be read off it. Every one of the five capabilities is served — over HTTP and
//! over MCP — and **none of them declares a command line**, which is what "served, never
//! published" amounts to mechanically: a command line is how a value reaches a script, a
//! log and eventually a commit, and nothing under `.ai/local/` may take that path.
//!
//! ```
//! use majordomus_cli::capability::builtin::lifecycle;
//!
//! let m = lifecycle::module();
//! assert_eq!(m.id.as_str(), "lifecycle");
//!
//! // declaration order, which is the order the five are read in
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, [
//!     "lifecycle.episodes",
//!     "lifecycle.recovery",
//!     "lifecycle.runtime",
//!     "lifecycle.providers",
//!     "lifecycle.closed",
//! ]);
//!
//! // served everywhere a worker in front of this checkout can reach, and nowhere a
//! // script can: the local half of the layer never becomes a committed value
//! for e in &m.capabilities {
//!     let id = e.capability.id.as_str();
//!     assert!(e.capability.exposure.http.is_some(), "{id} is not served over HTTP");
//!     assert!(e.capability.exposure.mcp.is_some(), "{id} is not served over MCP");
//!     assert!(e.capability.exposure.cli.is_none(), "{id} declares a command line");
//! }
//! ```

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
///
/// Read as a scale, the four run from "this checkout's own work" to "nobody's work any
/// more", and only the last is a fault: `current` and `open` are both this worktree's live
/// episodes and differ only in which one the pointer happens to aim at, `foreign` belongs to
/// another worktree and is reported rather than adopted, and `stranded` is the one that
/// needs a person, because nothing can close it.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::EpisodeStanding;
///
/// // the standings that are somebody's live work, against the one that is nobody's
/// let live = [EpisodeStanding::Current, EpisodeStanding::Open, EpisodeStanding::Foreign];
/// assert!(live.iter().all(|s| *s != EpisodeStanding::Stranded));
///
/// // serialised as the word `as_str` gives, so the two renderings cannot drift apart
/// for s in [EpisodeStanding::Current, EpisodeStanding::Foreign, EpisodeStanding::Stranded] {
///     assert_eq!(serde_json::to_value(s).unwrap(), serde_json::json!(s.as_str()));
/// }
/// ```
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
    /// The word this standing is reported and serialised under. One spelling for the
    /// value, so that the Cockpit, the API and MCP name a stranded episode identically
    /// and a reader who learned the vocabulary once has learned it everywhere.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::lifecycle::EpisodeStanding;
    /// assert_eq!(EpisodeStanding::Stranded.as_str(), "stranded");
    /// // the serialised form is this word, not a second rendering of the same value
    /// assert_eq!(
    ///     serde_json::to_value(EpisodeStanding::Stranded).unwrap(),
    ///     serde_json::json!(EpisodeStanding::Stranded.as_str()),
    /// );
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
///
/// Everything here is recorded rather than derived, and almost everything is optional,
/// which is the point: an episode opened by hand has no provider, a worker that supplied no
/// identity has none, and an episode that opened and did nothing has no ledger line and so
/// no `last_activity`. Each of those is ordinary and none is a fault, so each is carried as
/// absence — the empty fields are skipped on the wire — and a reader must handle their
/// absence rather than expect a placeholder.
///
/// The two fields that are always there are `standing` and `note`: where the episode stands,
/// and why it stands there in words. A standing a reader cannot act on is the failure this
/// subsystem was corrected for, so `note` is never empty.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::{Episode, EpisodeStanding};
/// use serde_json::json;
///
/// // the smallest true record: an episode opened by hand that has written nothing yet
/// let e: Episode = serde_json::from_value(json!({
///     "session_id": "s-0001",
///     "path": ".ai/local/state/sessions-open/by-hand.yaml",
///     "standing": "open",
///     "events": 0,
///     "note": "open in this worktree; the pointer names another episode",
/// })).unwrap();
///
/// assert_eq!(e.standing, EpisodeStanding::Open);
/// assert!(e.provider.is_empty(), "no provider session owns a hand-opened episode");
/// assert!(e.last_activity.is_empty(), "it has written no ledger line");
/// assert!(e.tasks.is_empty(), "work outside a task is permitted");
/// assert!(!e.note.is_empty(), "a standing always says why");
///
/// // and absence stays absence: the empty fields are not serialised back as ""
/// let wire = serde_json::to_value(&e).unwrap();
/// assert!(wire.get("provider").is_none());
/// assert_eq!(wire["events"], 0, "a count of zero is a measurement, so it is kept");
/// ```
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
    /// to go unwritten, which is the premise ADR 0052 removes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    /// Why it stands where it does, in words. Never empty: a standing a reader cannot act
    /// on is the failure this subsystem is being corrected for.
    pub note: String,
}

/// Every open episode of this checkout's store — the answer `continuity.state` cannot give,
/// because it reads the pointer and this reads the store.
///
/// `current` is the one episode the pointer resolves to, and it is one row of `episodes`
/// rather than a separate thing: the whole correction this type carries is that the pointed-at
/// episode is *a* member of the set and not the set. On the day this was written the store
/// held five open episodes in one checkout and every surface could name one of them.
///
/// `present: false` is a fresh clone, which is not a fault — nobody has opened an episode
/// here yet — and is reported as itself rather than as an error.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::Episodes;
/// use serde_json::json;
///
/// // a clone in which no episode has ever been opened: absence, answered as absence
/// let fresh: Episodes = serde_json::from_value(json!({
///     "present": false,
///     "store": ".ai/local/state/sessions-open",
///     "worktree": "/tmp/a-fresh-clone",
/// })).unwrap();
/// assert!(!fresh.present);
/// assert!(fresh.episodes.is_empty() && fresh.current.is_empty());
/// assert!(fresh.findings.is_empty(), "nothing to warn about; it is simply new");
///
/// // and a store with work in it: `current` names one of the rows, never a sixth thing
/// let busy: Episodes = serde_json::from_value(json!({
///     "present": true,
///     "store": ".ai/local/state/sessions-open",
///     "worktree": "/tmp/wt",
///     "current": "s-0002",
///     "episodes": [
///         {"session_id": "s-0001", "path": "a.yaml", "standing": "open",
///          "events": 3, "note": "this worktree, not pointed at"},
///         {"session_id": "s-0002", "path": "b.yaml", "standing": "current",
///          "events": 9, "note": "the pointer resolves to it"},
///     ],
/// })).unwrap();
/// assert_eq!(busy.episodes.len(), 2, "the pointer is blind to the first of these");
/// assert!(busy.episodes.iter().any(|e| e.session_id == busy.current));
/// ```
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
///
/// Only one of the three is a finding, and it is the middle one. `Pointer` is the current
/// layout and `Absent` means no episode is open here — a fresh clone, or a worker between
/// episodes — while `Inline` is a checkout still on the pre-migration layout, where a
/// second concurrent session silently overwrites the first one's record. So the question a
/// reader asks of this value is not "which layout" but "is it `Inline`".
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::PointerLayout;
///
/// // the two that are fine, and the one that loses records
/// let loses_records = |l: PointerLayout| l == PointerLayout::Inline;
/// assert!(!loses_records(PointerLayout::Pointer));
/// assert!(!loses_records(PointerLayout::Absent), "no episode open is not a fault");
/// assert!(loses_records(PointerLayout::Inline));
///
/// // serialised as the word `as_str` gives
/// assert_eq!(
///     serde_json::to_value(PointerLayout::Absent).unwrap(),
///     serde_json::json!(PointerLayout::Absent.as_str()),
/// );
/// ```
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
    /// The word this layout is reported and serialised under. Named for what the pointer
    /// *is* rather than for the migration that produced it, so that the value still reads
    /// correctly once nobody remembers there was a migration.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::lifecycle::PointerLayout;
    /// assert_eq!(PointerLayout::Inline.as_str(), "inline");
    /// // the serialised form is this word, not a second rendering of the same value
    /// assert_eq!(
    ///     serde_json::to_value(PointerLayout::Inline).unwrap(),
    ///     serde_json::json!(PointerLayout::Inline.as_str()),
    /// );
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
///
/// `layout` and `resolves` are independent, and the interesting case is the pair
/// `Pointer` + `resolves: false`: a symlink that survived the episode it named, which is
/// what a killed close or a hand-removed record leaves behind. The layout is right, the
/// destination is gone, and `continuity.state` has nothing to report while the store may
/// still hold open episodes — so this is a dangling pointer, not an empty store.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::{Pointer, PointerLayout};
/// use serde_json::json;
///
/// // the healthy case: the current layout, and it leads to an episode
/// let live: Pointer = serde_json::from_value(json!({
///     "layout": "pointer",
///     "target": "sessions-open/claude-abc.yaml",
///     "resolves": true,
///     "session_id": "s-0002",
///     "note": "the pointer resolves to an open episode",
/// })).unwrap();
/// assert_eq!(live.layout, PointerLayout::Pointer);
/// assert!(live.resolves && !live.session_id.is_empty());
///
/// // and the dangling one: the right layout, aimed at a record that is no longer there
/// let dangling: Pointer = serde_json::from_value(json!({
///     "layout": "pointer",
///     "target": "sessions-open/claude-gone.yaml",
///     "resolves": false,
///     "note": "the symlink outlived the episode it named",
/// })).unwrap();
/// assert_eq!(dangling.layout, PointerLayout::Pointer, "the layout is not the fault");
/// assert!(!dangling.resolves);
/// assert!(dangling.session_id.is_empty(), "there is no episode to name");
/// ```
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
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::Stranded;
/// use serde_json::json;
///
/// let s: Stranded = serde_json::from_value(json!({
///     "session_id": "s-0007",
///     "path": ".ai/local/state/sessions-open/claude-7.yaml",
///     "reason": "the worktree it opened in is gone from disk",
///     "remedy": "rm .ai/local/state/sessions-open/claude-7.yaml",
/// })).unwrap();
///
/// // the reason is about the world and the remedy is a command: a finding with no
/// // remedy is a complaint, so both are required fields rather than optional ones
/// assert!(!s.reason.is_empty() && !s.remedy.is_empty());
/// assert!(s.remedy.contains(&s.path), "the remedy acts on the file it names");
/// ```
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
///
/// `bytes` is carried because it is the one thing that decides what to do with the file: a
/// zero-length temporary file is a close that died before it wrote anything and can simply
/// go, while a large one is a record that was written and never renamed, and is worth
/// reading before it is deleted.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::Orphan;
/// use majordomus_cli::order::canonical;
///
/// let mut orphans = vec![
///     Orphan { path: ".ai/repo/project/sessions/.tmp.b2".into(), bytes: 4_812 },
///     Orphan { path: ".ai/repo/project/sessions/.tmp.a1".into(), bytes: 0 },
/// ];
/// canonical(&mut orphans);
///
/// // filed under the path, which names one file and so makes the list total
/// let paths: Vec<&str> = orphans.iter().map(|o| o.path.as_str()).collect();
/// assert_eq!(paths, [
///     ".ai/repo/project/sessions/.tmp.a1",
///     ".ai/repo/project/sessions/.tmp.b2",
/// ]);
///
/// // and the size is what tells a dead stub from a record that was written
/// assert_eq!(orphans[0].bytes, 0, "nothing was written before the close died");
/// assert!(orphans[1].bytes > 0, "read this one before removing it");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Orphan {
    /// Repository-relative path.
    pub path: String,
    /// Its size in bytes, which is how a reader tells an empty stub from a lost record.
    pub bytes: u64,
}

/// A path names one file, so it is both what a reader looks for and the identity that makes
/// the orphan list total.
impl crate::order::Ordered for Orphan {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.path, &self.path)
    }
}

/// What the ledger says about episode lifecycle against what the store holds.
///
/// The invariant ADR 0052 asks for, stated as arithmetic a person can check: every episode
/// that started either closed or is still open. When `started` exceeds `closed + open`, the
/// difference is episodes whose close was never recorded — the shape of the outage that ADR
/// describes, where events kept arriving and the records they should have produced did not.
///
/// The sign matters and the two directions mean opposite things. Positive `unaccounted` is
/// the outage: episodes that started, are not open, and produced no closed record — work
/// that happened and left nothing behind. Negative is ordinary bookkeeping: the ledger was
/// rotated, or a record was committed from a checkout whose ledger lines are not in this
/// one, so there are more records than started lines to account for them.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::Balance;
/// use serde_json::json;
///
/// let of = |started, closed, open, unaccounted, agrees| -> Balance {
///     serde_json::from_value(json!({
///         "started": started, "closed": closed, "open": open,
///         "unaccounted": unaccounted, "agrees": agrees, "note": "as measured",
///     })).unwrap()
/// };
///
/// // the good case: every episode that started either closed or is still open
/// let sound = of(40, 37, 3, 0, true);
/// assert_eq!(sound.started as i64 - sound.closed as i64 - sound.open as i64, sound.unaccounted);
/// assert!(sound.agrees);
///
/// // the outage ADR 0052 describes: four closes that were never recorded
/// let lost = of(40, 33, 3, 4, false);
/// assert!(lost.unaccounted > 0 && !lost.agrees);
///
/// // and a rotated ledger, which is not a fault in either half
/// let rotated = of(2, 37, 3, -38, false);
/// assert!(rotated.unaccounted < 0, "more records than lines to account for them");
/// ```
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
///
/// `findings` is the whole report said once, and an empty `findings` is the verdict: there
/// is nothing to recover. That is the field to branch on — the three detailed lists and the
/// arithmetic are what a reader consults *after* deciding there is something to look at,
/// and each of them can be empty for a good reason while another is not.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::Recovery;
/// use serde_json::json;
///
/// // a healthy checkout: the current layout, nothing stranded, the arithmetic agreeing
/// let clean: Recovery = serde_json::from_value(json!({
///     "present": true,
///     "pointer": {"layout": "pointer", "target": "sessions-open/c.yaml",
///                 "resolves": true, "session_id": "s-1", "note": "resolves"},
///     "balance": {"started": 8, "closed": 7, "open": 1,
///                 "unaccounted": 0, "agrees": true, "note": "they agree"},
/// })).unwrap();
///
/// assert!(clean.findings.is_empty(), "an empty findings list is the good verdict");
/// assert!(clean.stranded.is_empty() && clean.orphans.is_empty());
/// assert!(clean.balance.agrees);
///
/// // a fresh clone has no local half at all, which is also nothing to recover
/// let fresh: Recovery = serde_json::from_value(json!({
///     "present": false,
///     "pointer": {"layout": "absent", "resolves": false, "note": "no episode is open here"},
///     "balance": {"started": 0, "closed": 0, "open": 0,
///                 "unaccounted": 0, "agrees": true, "note": "nothing has happened yet"},
/// })).unwrap();
/// assert!(!fresh.present && fresh.findings.is_empty());
/// ```
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
///
/// `agree` is the field with the operational meaning, and what it says is *not* "the
/// repository is fine" but "the answers you are reading are about the tree you are looking
/// at". When it is false every other capability this process serves is answering about a
/// commit that is no longer checked out, and nothing else in the API would tell you.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::RuntimeView;
/// use serde_json::json;
///
/// let view = |served: &str, repo: &str, agree: bool| -> RuntimeView {
///     serde_json::from_value(json!({
///         "root": "/Users/x/repo",
///         "served_head": served, "served_branch": "master", "served_working_tree": "clean",
///         "repository_head": repo, "repository_branch": "master",
///         "repository_working_tree": "clean",
///         "agree": agree, "objects": 1319, "index_state": "ok",
///         "note": "as measured on this call",
///     })).unwrap()
/// };
///
/// // a server following the repository: one `git` invocation buys the proof
/// let current = view("a1b2c3d", "a1b2c3d", true);
/// assert!(current.agree);
/// assert_eq!(current.served_head, current.repository_head);
/// assert_eq!(current.index_state, "ok");
///
/// // and one that froze its index at start-up: every answer it serves is about the old commit
/// let frozen = view("a1b2c3d", "9f8e7d6", false);
/// assert!(!frozen.agree);
/// assert_ne!(frozen.served_head, frozen.repository_head);
/// ```
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
///
/// The two halves come from different places and must not be confused. `lifecycle` and
/// `prompt_capture` are the *distribution's* declaration — what the shipped adapter can do
/// for this provider at all — while `wired` is *this repository's* policy naming that
/// provider's hook in `wired_by`. Either can be empty with the other full, and that pairing
/// is the interesting finding: a provider this repository wires but the tool ships no
/// lifecycle for is a hook that will never fire.
///
/// Neither half is inferred from a file on disk. Whether a shim is actually installed is
/// `majordomus capture status`'s question, and this type deliberately does not answer it.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::ProviderLifecycle;
/// use serde_json::json;
///
/// // a provider the tool adapts and this repository wires: both halves present
/// let full: ProviderLifecycle = serde_json::from_value(json!({
///     "id": "claude",
///     "title": "Claude Code",
///     "lifecycle": ["SessionStart", "SessionEnd", "PreCompact"],
///     "prompt_capture": true,
///     "client_config": ".mcp.json",
///     "wired": ["session.start", "session.close"],
///     "note": "adapted and wired",
/// })).unwrap();
/// assert!(!full.lifecycle.is_empty() && !full.wired.is_empty());
/// assert!(full.prompt_capture);
///
/// // and one the tool ships no adapter for: it loses the automation and none of the model,
/// // because every command remains the same
/// let unadapted: ProviderLifecycle = serde_json::from_value(json!({
///     "id": "some-editor",
///     "title": "Some Editor",
///     "prompt_capture": false,
///     "note": "no lifecycle adapter ships for it",
/// })).unwrap();
/// assert!(unadapted.lifecycle.is_empty(), "nothing will fire for it");
/// assert!(unadapted.client_config.is_empty(), "and it reads no project-scoped config");
/// ```
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

/// Every provider the distribution ships a bootstrap for, with the lifecycle half of each.
///
/// `with_lifecycle` is a count over `providers` rather than a second list, so the two
/// cannot disagree; it is there because "how many of the providers we support actually get
/// the automation" is the question this capability exists to answer, and a reader should not
/// have to fold the list to find out.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::ProviderLifecycles;
/// use serde_json::json;
///
/// let p: ProviderLifecycles = serde_json::from_value(json!({
///     "providers": [
///         {"id": "claude", "title": "Claude Code", "lifecycle": ["SessionStart"],
///          "prompt_capture": true, "note": "adapted"},
///         {"id": "plain", "title": "Plain", "prompt_capture": false, "note": "not adapted"},
///     ],
///     "with_lifecycle": 1,
/// })).unwrap();
///
/// // the count is over the list, so it is checkable against it
/// let counted = p.providers.iter().filter(|x| !x.lifecycle.is_empty()).count();
/// assert_eq!(p.with_lifecycle, counted);
/// assert_eq!(p.providers.len(), 2, "a provider with no adapter is still reported");
/// assert!(p.findings.is_empty());
/// ```
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

/// One closed episode of the tracked, durable projection — the half of this subsystem that
/// survives a clone.
///
/// It carries both a `path` and a `uri` because they answer different questions: the path
/// is where the record is in this checkout, and the `majordomus://session/<identity>`
/// identifier is what every other surface addresses the same record by, so a reader can
/// follow it to the object page without constructing one.
///
/// Everything but the identity, the identifier and the path is optional, because a record
/// written by an older close carries fewer fields and is still a record.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::ClosedSession;
/// use serde_json::json;
///
/// let s: ClosedSession = serde_json::from_value(json!({
///     "session_id": "s-0042",
///     "uri": "majordomus://session/2026-09-12-s-0042",
///     "path": ".ai/repo/project/sessions/2026-09-12-s-0042.md",
///     "created_at": "2026-09-12T15:44:00Z",
///     "branch": "master",
///     "head": "cd04012f1",
///     "outcome": "landed",
/// })).unwrap();
///
/// assert!(s.uri.starts_with("majordomus://session/"), "addressable as an object");
/// assert_eq!(s.outcome, "landed");
///
/// // an older record with less in it is still one, and says so by absence
/// let sparse: ClosedSession = serde_json::from_value(json!({
///     "session_id": "s-0001",
///     "uri": "majordomus://session/2026-08-01-s-0001",
///     "path": ".ai/repo/project/sessions/2026-08-01-s-0001.md",
/// })).unwrap();
/// assert!(sparse.outcome.is_empty() && sparse.title.is_empty());
/// ```
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
///
/// `total` is exact and `newest` is a window, and keeping them as two numbers is the whole
/// design of this type. The tracked directory grows without bound and a Cockpit card is not
/// a listing page, so the rows are capped — but a capped list that did not carry the true
/// total would quietly understate the repository, which is the shape of defect this
/// repository has been bitten by often enough to name. `window` says how many rows came
/// back, so a reader can see the cap rather than infer it.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::ClosedSessions;
/// use serde_json::json;
///
/// let row = |id: &str| json!({
///     "session_id": id,
///     "uri": format!("majordomus://session/{id}"),
///     "path": format!(".ai/repo/project/sessions/{id}.md"),
///     "branch": "master",
/// });
/// let c: ClosedSessions = serde_json::from_value(json!({
///     "total": 137,
///     "on_this_branch": 41,
///     "newest": [row("s-0137"), row("s-0136")],
///     "window": 2,
/// })).unwrap();
///
/// // the window is what came back; the total is what exists
/// assert_eq!(c.window, c.newest.len());
/// assert!(c.total > c.window, "the rows are capped and the total is not");
/// assert!(c.on_this_branch <= c.total);
/// ```
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
    /// The task it belonged to, when it belonged to one. This is the relation ADR 0052
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
type OpenRecord = (PathBuf, BTreeMap<String, String>);

fn open_records(dir: &Path) -> (Vec<OpenRecord>, Vec<String>) {
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

    let started = ledger
        .iter()
        .filter(|l| l.event == "session.started")
        .count();
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
            let mut found: Vec<Orphan> = entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp."))
                .map(|e| Orphan {
                    path: relative(&root, &e.path()),
                    bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .collect();
            crate::order::canonical(&mut found);
            orphans.extend(found);
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
                "{started} started, {closed} closed, {open} still open: {unaccounted} episode(s) started and neither closed nor left a record here. An end event that reached the tool and produced nothing looks exactly like this (ADR 0052)."
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

/// The module's one declaration: the five capabilities, their schemas and every projection
/// of them.
///
/// This function is the canonical definition and not a registration of one made elsewhere.
/// The HTTP routes, the MCP tools and resources, the OpenAPI components and the Cockpit
/// cards are all derived from what is declared here, which is why none of them is editable
/// on its own — a semantic definition repeated across projections is the design defect
/// ADR 0004 names.
///
/// ```
/// use majordomus_cli::capability::builtin::lifecycle::{self, EPISODES_URI, RECOVERY_URI};
///
/// let m = lifecycle::module();
/// assert_eq!(m.id.as_str(), "lifecycle");
/// assert_eq!(m.capabilities.len(), 5);
///
/// // the routes are derived from the declaration and written down nowhere else
/// let routes: Vec<&str> = m
///     .capabilities
///     .iter()
///     .filter_map(|e| e.capability.exposure.http.as_ref().map(|h| h.path.as_str()))
///     .collect();
/// assert_eq!(routes, [
///     "/api/v1/lifecycle/episodes",
///     "/api/v1/lifecycle/recovery",
///     "/api/v1/lifecycle/runtime",
///     "/api/v1/lifecycle/providers",
///     "/api/v1/lifecycle/closed",
/// ]);
///
/// // and the two readable resources are addressed by the constants this module exports,
/// // so a caller never spells a URI that the declaration does not carry
/// let resources: Vec<&str> = m
///     .capabilities
///     .iter()
///     .filter_map(|e| e.capability.exposure.mcp.as_ref())
///     .filter_map(|x| x.resource.as_ref())
///     .map(|r| r.uri.as_str())
///     .collect();
/// assert!(resources.contains(&EPISODES_URI));
/// assert!(resources.contains(&RECOVERY_URI));
/// ```
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
            episodes
                .exposure
                .mcp
                .as_ref()
                .and_then(|m| m.tool.as_deref()),
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
