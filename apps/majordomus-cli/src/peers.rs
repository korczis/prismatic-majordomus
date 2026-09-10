//! Peers: the clients attached to one shared server. Every MCP session, the owner's
//! stdio one and every one that arrived over HTTP, is a peer from the moment it attaches;
//! `initialize` gives it a name (the client's own `clientInfo`), and `peers.announce`
//! lets it say what it is working on. The board lives in the server's memory and nowhere
//! else: it ends with the process, and nothing here touches the repository.
//!
//! # Why it is in memory, and what outlives the process anyway
//!
//! A peer board is about *now*: who is attached, what they said they are doing. Writing it
//! into the repository would make it a record of something that has already stopped being
//! true, and the layer's records are for what survives the session. The board ends with the
//! process, which is exactly right for a fact about the process.
//!
//! One kind of fact on it is not about the process. What a peer said it was *working on* is
//! a fact about the repository, and the board already keeps it past the socket that said it
//! (a departed peer stays listed with `attached: false`). The same argument reaches past the
//! process: on 2026-09-09 the shared server restarted at 23:18, nine sessions were re-issued
//! `p1`..`p9`, every announcement was gone, and the intents that were re-announced carried
//! the old numbers of peers that no longer existed. So each announcement is also appended to
//! a journal under the ignored half of the layer (`.ai/local/state/mcp/board.jsonl`), stamped
//! with the generation of the server that heard it, and the next server lists the *previous*
//! generation's last word per peer as peers of an earlier generation: not attached, named
//! `p4@<generation>` so they cannot be confused with the live `p4`, and defending the ground
//! they claimed exactly as a departed peer does. The live board stays the truth about now;
//! the journal is the truth about what was said. ADR 0034.
//!
//! # What a peer may say
//!
//! An announcement is informational and enforces nothing. Its whole purpose is that another
//! agent working in the same checkout can read it and choose not to collide — the
//! coordination is between the peers, not imposed by the server.
//!
//! ```
//! use majordomus_cli::peers::{ClientInfo, PeerBoard, Transport};
//!
//! let board = PeerBoard::new();
//! let id = board.attach(Transport::Stdio);
//! board.identify(&id, ClientInfo::unknown());
//!
//! let announced = board
//!     .announce(&id, "landing the quality gate", vec!["apps/majordomus-cli".into()])
//!     .expect("the peer is attached");
//! assert_eq!(
//!     announced.peer.announcement.as_ref().map(|a| a.intent.as_str()),
//!     Some("landing the quality gate")
//! );
//! assert_eq!(board.list().len(), 1);
//!
//! // a session that ends is no longer counted; what it announced stays readable, so the
//! // next worker can still see the claim and avoid it
//! board.detach(&id);
//! assert_eq!(board.len(), 0);
//! assert!(!board.list()[0].attached);
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A peer's identity for the life of the server: `p1`, `p2`, ... in attachment order.
/// `p1` is the session that started the server.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct PeerId(String);

impl PeerId {
    fn new(seq: u64) -> Self {
        PeerId(format!("p{seq}"))
    }

    /// The id of a peer of an earlier server generation: `p4@2026-09-09T21:18:39Z`. It
    /// names the peer *and* the server that heard it, so it cannot be mistaken for the
    /// live `p4` of this process.
    fn earlier(id: &str, generation: &str) -> Self {
        PeerId(format!("{id}@{generation}"))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn seq(&self) -> u64 {
        self.0[1..].parse().unwrap_or(u64::MAX)
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a client said about itself in `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientInfo {
    /// `clientInfo.name`: `claude-code`, `codex`, `gemini-cli`, whatever the client sends.
    pub name: String,
    /// `clientInfo.version`.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `clientInfo.title`, when the client sends one.
    pub title: Option<String>,
}

impl ClientInfo {
    /// A session that has attached and not yet initialised.
    pub fn unknown() -> Self {
        ClientInfo {
            name: "(not initialized)".into(),
            version: String::new(),
            title: None,
        }
    }

    /// Read `clientInfo` out of `initialize` params; `unknown` when it is missing.
    pub fn from_initialize(params: &serde_json::Value) -> Self {
        let info = &params["clientInfo"];
        let name = info["name"].as_str().unwrap_or("").trim();
        if name.is_empty() {
            return Self::unknown();
        }
        ClientInfo {
            name: name.to_string(),
            version: info["version"].as_str().unwrap_or("").to_string(),
            title: info["title"].as_str().map(str::to_string),
        }
    }
}

/// How a peer reached the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// The owner's own client, on the process's stdin and stdout.
    Stdio,
    /// MCP over HTTP at `/mcp`: a `majordomus mcp` bridge, or a client speaking it directly.
    Http,
}

/// What a peer said it is working on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Announcement {
    /// One line: the task or intent, in the peer's words.
    pub intent: String,
    /// Repository-relative paths the peer expects to touch; informational, never enforced here.
    pub scope: Vec<String>,
    /// When it was announced, RFC 3339, UTC.
    pub at: String,
}

/// One peer as the board lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Peer {
    /// `p1`, `p2`, ...
    pub id: PeerId,
    /// The client behind it.
    pub client: ClientInfo,
    /// How it is attached.
    pub transport: Transport,
    /// When it attached, RFC 3339, UTC.
    pub connected_at: String,
    /// Seconds since its last message.
    pub last_seen_seconds_ago: u64,
    /// Whether the session is still attached. A peer that announced something and then
    /// went away is kept and listed with `attached: false`: what it said it was working on
    /// outlives the connection that said it, because the work does.
    pub attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Its announcement, when it made one.
    pub announcement: Option<Announcement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The `started_at` of the server that heard this peer, when that server is not this
    /// one: the peer belongs to the previous generation, read back from the journal, and
    /// its id carries the same value (`p4@<generation>`). Absent for a peer of this server.
    pub generation: Option<String>,
}

/// Two peers that claimed the same ground.
///
/// Reported when the second of them announces, so that a collision is known at the moment
/// it is created rather than discovered afterwards in the history of a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Overlap {
    /// The other peer.
    pub peer: PeerId,
    /// Whether that peer is still attached. An overlap with a departed peer is a weaker
    /// signal than one with a live session, and the reader is told which it is.
    pub attached: bool,
    /// What it said it was doing.
    pub intent: String,
    /// The claims that meet: one line per pair, `yours` and `theirs`.
    pub paths: Vec<OverlapPath>,
}

/// One pair of claims that contain one another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OverlapPath {
    /// The path the announcing peer claimed.
    pub yours: String,
    /// The path the other peer claimed.
    pub theirs: String,
}

/// What `announce` answers: the peer as recorded, and who else is on that ground.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Announced {
    /// The peer, with the announcement it just made.
    #[serde(flatten)]
    pub peer: Peer,
    /// Every other peer whose claimed scope meets this one. Empty is the ordinary case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlaps: Vec<Overlap>,
}

/// Do two claimed paths meet? Equal, or one inside the other.
///
/// Repository-relative, `/`-separated, trailing slashes ignored. `apps` contains
/// `apps/majordomus-cli`; `app` does not, because a claim is a path and not a prefix of a
/// string.
///
/// ```
/// use majordomus_cli::peers::claims_meet;
/// assert!(claims_meet("apps", "apps/majordomus-cli"));
/// assert!(claims_meet("apps/majordomus-cli", "apps"));
/// assert!(claims_meet("docs/", "docs"));
/// assert!(!claims_meet("app", "apps/majordomus-cli"));
/// assert!(!claims_meet("docs", "site"));
/// ```
pub fn claims_meet(a: &str, b: &str) -> bool {
    let (a, b) = (
        a.trim().trim_end_matches('/'),
        b.trim().trim_end_matches('/'),
    );
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/"))
}

/// The journal file's `schema`.
pub const JOURNAL_SCHEMA: &str = "majordomus-peer-board/v1";

/// The journal's file name, beside the lease under `.ai/local/state/mcp/`.
pub const JOURNAL_FILE: &str = "board.jsonl";

/// How many lines the journal keeps. Past this the oldest are dropped when the next server
/// opens it, for the reason [`RETAINED`] exists: a repository served for a month is not a
/// museum, and the previous generation's last word per peer is what a reader needs.
pub const JOURNAL_RETAINED: usize = 1000;

/// One line of the journal: an announcement, and which server heard it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JournalLine {
    /// [`JOURNAL_SCHEMA`].
    pub schema: String,
    /// The `started_at` of the server that recorded it: its generation.
    pub generation: String,
    /// The peer's id in that generation (`p4`).
    pub peer: String,
    /// The client behind it.
    pub client: ClientInfo,
    /// How it was attached.
    pub transport: Transport,
    /// When it attached, RFC 3339.
    pub connected_at: String,
    /// What it announced.
    #[serde(flatten)]
    pub announcement: Announcement,
}

/// What opening the journal found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JournalOpened {
    /// Peers of the previous generation now on the board.
    pub earlier: usize,
    /// The generation they belong to, when there is one.
    pub previous_generation: Option<String>,
    /// Lines that were not journal lines and were skipped.
    pub skipped: usize,
    /// Whether the file was rewritten to [`JOURNAL_RETAINED`] lines.
    pub compacted: bool,
}

struct Journal {
    path: PathBuf,
    generation: String,
}

/// A peer of the previous generation, as the journal's last word about it.
struct Earlier {
    id: PeerId,
    client: ClientInfo,
    transport: Transport,
    connected_at: String,
    announcement: Announcement,
    generation: String,
}

struct Slot {
    client: ClientInfo,
    transport: Transport,
    connected_at: SystemTime,
    last_seen: Instant,
    departed: bool,
    announcement: Option<Announcement>,
}

/// How many departed peers the board keeps. A retained announcement is the whole point —
/// a long session that reconnects must not vanish from the board — but a server that ran
/// all day must not accumulate every connection it ever saw either, so the oldest departed
/// slot is dropped past this. Attached peers are never dropped.
const RETAINED: usize = 32;

/// The peers of one server process, and the last word of the previous one's.
#[derive(Default)]
pub struct PeerBoard {
    next: AtomicU64,
    slots: Mutex<BTreeMap<u64, Slot>>,
    journal: Mutex<Option<Journal>>,
    earlier: Mutex<Vec<Earlier>>,
}

impl fmt::Debug for PeerBoard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerBoard")
            .field("peers", &self.len())
            .finish()
    }
}

impl PeerBoard {
    /// An empty board.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the journal this server writes and read the previous generation out of it.
    ///
    /// `generation` is this server's `started_at`. Every line of another generation is a
    /// candidate; the newest such generation is the previous one, and its last line per
    /// peer becomes a peer of an earlier generation on this board. A line that is not a
    /// journal line is skipped and counted, never fatal: a journal a person can corrupt
    /// must not stop a server from starting. Past [`JOURNAL_RETAINED`] lines the file is
    /// rewritten with the newest ones, atomically.
    ///
    /// ```
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let dir = tempfile::tempdir().unwrap();
    /// let path = dir.path().join("board.jsonl");
    ///
    /// let first = PeerBoard::new();
    /// first.open_journal(path.clone(), "2026-09-09T21:18:39Z".into());
    /// let a = first.attach(Transport::Http);
    /// first.announce(&a, "the ordering rule", vec!["apps".into()]);
    ///
    /// let second = PeerBoard::new();
    /// let opened = second.open_journal(path, "2026-09-09T23:18:39Z".into());
    /// assert_eq!(opened.earlier, 1);
    /// let listed = second.list();
    /// assert_eq!(listed[0].id.as_str(), "p1@2026-09-09T21:18:39Z");
    /// assert!(!listed[0].attached);
    /// assert_eq!(listed[0].generation.as_deref(), Some("2026-09-09T21:18:39Z"));
    /// assert_eq!(listed[0].announcement.as_ref().unwrap().intent, "the ordering rule");
    /// ```
    pub fn open_journal(&self, path: PathBuf, generation: String) -> JournalOpened {
        let (lines, skipped, total) = read_journal(&path);
        let previous = lines
            .iter()
            .filter(|l| l.generation != generation)
            .map(|l| l.generation.clone())
            .max();
        let mut earlier: Vec<Earlier> = Vec::new();
        if let Some(prev) = &previous {
            // the last word per peer: later lines replace earlier ones
            let mut last: BTreeMap<String, &JournalLine> = BTreeMap::new();
            for l in lines.iter().filter(|l| &l.generation == prev) {
                last.insert(l.peer.clone(), l);
            }
            earlier = last
                .into_values()
                .map(|l| Earlier {
                    id: PeerId::earlier(&l.peer, &l.generation),
                    client: l.client.clone(),
                    transport: l.transport,
                    connected_at: l.connected_at.clone(),
                    announcement: l.announcement.clone(),
                    generation: l.generation.clone(),
                })
                .collect();
            earlier.sort_by_key(|e| peer_seq(&e.id));
        }
        let compacted = total > JOURNAL_RETAINED && compact_journal(&path, &lines);
        let opened = JournalOpened {
            earlier: earlier.len(),
            previous_generation: previous,
            skipped,
            compacted,
        };
        *lock_of(&self.earlier) = earlier;
        *lock_of(&self.journal) = Some(Journal { path, generation });
        opened
    }

    /// Append one announcement to the journal, when one is open. Best effort: a journal
    /// that cannot be written is logged and the board goes on, because an announcement
    /// that reached the board must be answered whatever the disk says.
    fn journal(&self, line: JournalLine) {
        let guard = lock_of(&self.journal);
        let Some(j) = guard.as_ref() else {
            return;
        };
        if let Err(e) = append_line(&j.path, &line) {
            tracing::debug!(path = %j.path.display(), error = %e, "the peer journal could not be written");
        }
    }

    /// A session attached; it has no name until [`PeerBoard::identify`].
    ///
    /// ```
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let board = PeerBoard::new();
    /// let a = board.attach(Transport::Stdio);
    /// let b = board.attach(Transport::Http);
    /// assert_eq!((a.as_str(), b.as_str()), ("p1", "p2"));
    /// board.detach(&a);
    /// assert_eq!(board.list().len(), 1);
    /// ```
    pub fn attach(&self, transport: Transport) -> PeerId {
        let seq = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.lock().insert(
            seq,
            Slot {
                client: ClientInfo::unknown(),
                transport,
                connected_at: SystemTime::now(),
                last_seen: Instant::now(),
                departed: false,
                announcement: None,
            },
        );
        PeerId::new(seq)
    }

    /// The client behind a peer, from its `initialize`.
    pub fn identify(&self, id: &PeerId, client: ClientInfo) {
        if let Some(s) = self.lock().get_mut(&id.seq()) {
            s.client = client;
            s.last_seen = Instant::now();
        }
    }

    /// A message arrived from this peer.
    pub fn touch(&self, id: &PeerId) {
        if let Some(s) = self.lock().get_mut(&id.seq()) {
            s.last_seen = Instant::now();
        }
    }

    /// Record what a peer is working on, and answer with who else is on that ground.
    ///
    /// `None` when the id is not attached. The overlap is computed here, under the same
    /// lock as the write, so that two peers announcing at once cannot both be told they
    /// are alone.
    ///
    /// ```
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let board = PeerBoard::new();
    /// let a = board.attach(Transport::Stdio);
    /// let b = board.attach(Transport::Http);
    /// board.announce(&a, "the ordering rule", vec!["apps/majordomus-cli/src".into()]);
    /// let second = board.announce(&b, "the release model", vec!["apps/majordomus-cli/src/release".into()]).unwrap();
    /// assert_eq!(second.overlaps.len(), 1, "a claim inside another claim is an overlap");
    /// assert_eq!(second.overlaps[0].intent, "the ordering rule");
    /// ```
    pub fn announce(&self, id: &PeerId, intent: &str, scope: Vec<String>) -> Option<Announced> {
        let (announced, line) = {
            let mut slots = self.lock();
            if !slots.contains_key(&id.seq()) {
                return None;
            }
            let earlier = lock_of(&self.earlier);
            let overlaps = overlaps_against(&slots, &earlier, id.seq(), &scope);
            let s = slots.get_mut(&id.seq())?;
            s.last_seen = Instant::now();
            s.departed = false;
            let announcement = Announcement {
                intent: intent.to_string(),
                scope,
                at: rfc3339(SystemTime::now()),
            };
            s.announcement = Some(announcement.clone());
            let generation = lock_of(&self.journal)
                .as_ref()
                .map(|j| j.generation.clone());
            let line = generation.map(|generation| JournalLine {
                schema: JOURNAL_SCHEMA.into(),
                generation,
                peer: id.as_str().to_string(),
                client: s.client.clone(),
                transport: s.transport,
                connected_at: rfc3339(s.connected_at),
                announcement,
            });
            (
                Announced {
                    peer: peer(id.seq(), s),
                    overlaps,
                },
                line,
            )
        };
        // written after the locks are released: the disk is not on the board's critical path
        if let Some(line) = line {
            self.journal(line);
        }
        Some(announced)
    }

    /// Every pair of peers whose claims meet, each pair once, in peer order.
    pub fn overlaps(&self) -> Vec<Overlap> {
        let slots = self.lock();
        let earlier = lock_of(&self.earlier);
        let mut out = Vec::new();
        for (seq, s) in slots.iter() {
            let Some(mine) = &s.announcement else {
                continue;
            };
            for o in overlaps_against(&slots, &earlier, *seq, &mine.scope) {
                // each pair once: report it against the later peer only, and an earlier
                // generation's claim always against the live peer that meets it
                if o.peer.seq() < *seq || o.peer.as_str().contains('@') {
                    out.push(o);
                }
            }
        }
        out
    }

    /// One peer, as listed.
    pub fn get(&self, id: &PeerId) -> Option<Peer> {
        self.lock().get(&id.seq()).map(|s| peer(id.seq(), s))
    }

    /// The session ended.
    ///
    /// A peer that announced something is kept and marked departed rather than removed:
    /// what a session said it was working on is a fact about the repository, and it does
    /// not stop being true because a socket closed. A peer that never announced leaves
    /// nothing behind. The oldest departed slot past `RETAINED` is dropped.
    ///
    /// ```
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let board = PeerBoard::new();
    /// let quiet = board.attach(Transport::Http);
    /// let spoken = board.attach(Transport::Http);
    /// board.announce(&spoken, "the ordering rule", vec!["apps".into()]);
    /// board.detach(&quiet);
    /// board.detach(&spoken);
    /// assert_eq!(board.len(), 0, "neither is attached any more");
    /// let listed = board.list();
    /// assert_eq!(listed.len(), 1, "the one that said something is still on the board");
    /// assert!(!listed[0].attached);
    /// assert_eq!(listed[0].announcement.as_ref().unwrap().intent, "the ordering rule");
    /// ```
    pub fn detach(&self, id: &PeerId) {
        let mut slots = self.lock();
        match slots.get_mut(&id.seq()) {
            Some(s) if s.announcement.is_some() => s.departed = true,
            _ => {
                slots.remove(&id.seq());
                return;
            }
        }
        let departed: Vec<u64> = slots
            .iter()
            .filter(|(_, s)| s.departed)
            .map(|(seq, _)| *seq)
            .collect();
        for seq in departed
            .iter()
            .take(departed.len().saturating_sub(RETAINED))
        {
            slots.remove(seq);
        }
    }

    /// Every peer of this server in attachment order, then the previous generation's.
    pub fn list(&self) -> Vec<Peer> {
        let mut out: Vec<Peer> = self.lock().iter().map(|(seq, s)| peer(*seq, s)).collect();
        out.extend(lock_of(&self.earlier).iter().map(earlier_peer));
        out
    }

    /// How many peers are attached. A departed peer whose announcement the board kept is
    /// listed but not counted: the count is what the lifecycle asks, and the lifecycle
    /// asks who is here now.
    pub fn len(&self) -> usize {
        self.lock().values().filter(|s| !s.departed).count()
    }

    /// Nobody attached?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The names of every peer, one line, for a log or the `initialize` instructions.
    pub fn summary(&self) -> String {
        let peers = self.list();
        if peers.is_empty() {
            return "none".into();
        }
        peers
            .iter()
            .map(|p| match (&p.announcement, &p.generation) {
                (Some(a), Some(_)) => format!(
                    "{} {} (previous server generation: {})",
                    p.id, p.client.name, a.intent
                ),
                (Some(a), None) => format!(
                    "{} {} ({:?}: {})",
                    p.id, p.client.name, p.transport, a.intent
                ),
                (None, _) => format!("{} {} ({:?})", p.id, p.client.name, p.transport),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<u64, Slot>> {
        lock_of(&self.slots)
    }
}

fn lock_of<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The numeric part of a peer id, `p4` and `p4@<generation>` alike; `u64::MAX` for neither.
fn peer_seq(id: &PeerId) -> u64 {
    id.as_str()
        .trim_start_matches('p')
        .split('@')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(u64::MAX)
}

fn earlier_peer(e: &Earlier) -> Peer {
    Peer {
        id: e.id.clone(),
        client: e.client.clone(),
        transport: e.transport,
        connected_at: e.connected_at.clone(),
        last_seen_seconds_ago: seconds_since(&e.announcement.at),
        attached: false,
        announcement: Some(e.announcement.clone()),
        generation: Some(e.generation.clone()),
    }
}

/// Seconds between an RFC 3339 instant and now; zero when it cannot be read or is ahead.
fn seconds_since(at: &str) -> u64 {
    let Some(then) = parse_rfc3339(at) else {
        return 0;
    };
    SystemTime::now()
        .duration_since(UNIX_EPOCH + Duration::from_secs(then))
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Every well-formed line of the journal, how many were not, and how many lines there were.
fn read_journal(path: &Path) -> (Vec<JournalLine>, usize, usize) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (Vec::new(), 0, 0);
    };
    let mut lines = Vec::new();
    let mut skipped = 0;
    let mut total = 0;
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        total += 1;
        match serde_json::from_str::<JournalLine>(raw) {
            Ok(l) if l.schema == JOURNAL_SCHEMA => lines.push(l),
            _ => skipped += 1,
        }
    }
    (lines, skipped, total)
}

/// Rewrite the journal with its newest [`JOURNAL_RETAINED`] well-formed lines, atomically.
fn compact_journal(path: &Path, lines: &[JournalLine]) -> bool {
    let keep = &lines[lines.len().saturating_sub(JOURNAL_RETAINED)..];
    let mut text = String::new();
    for l in keep {
        if let Ok(s) = serde_json::to_string(l) {
            text.push_str(&s);
            text.push('\n');
        }
    }
    let tmp = path.with_extension("jsonl.tmp");
    if std::fs::write(&tmp, text).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

fn append_line(path: &Path, line: &JournalLine) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let text = serde_json::to_string(line).map_err(std::io::Error::other)?;
    f.write_all(text.as_bytes())?;
    f.write_all(b"\n")
}

fn peer(seq: u64, s: &Slot) -> Peer {
    Peer {
        id: PeerId::new(seq),
        client: s.client.clone(),
        transport: s.transport,
        connected_at: rfc3339(s.connected_at),
        last_seen_seconds_ago: s.last_seen.elapsed().as_secs(),
        attached: !s.departed,
        announcement: s.announcement.clone(),
        generation: None,
    }
}

/// Every peer other than `seq` whose announced scope meets `scope`: this server's, then
/// the previous generation's, which defend their ground as a departed peer does.
fn overlaps_against(
    slots: &BTreeMap<u64, Slot>,
    earlier: &[Earlier],
    seq: u64,
    scope: &[String],
) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (other, s) in slots.iter() {
        if *other == seq {
            continue;
        }
        let Some(theirs) = &s.announcement else {
            continue;
        };
        let paths = meeting_paths(scope, &theirs.scope);
        if !paths.is_empty() {
            out.push(Overlap {
                peer: PeerId::new(*other),
                attached: !s.departed,
                intent: theirs.intent.clone(),
                paths,
            });
        }
    }
    for e in earlier {
        let paths = meeting_paths(scope, &e.announcement.scope);
        if !paths.is_empty() {
            out.push(Overlap {
                peer: e.id.clone(),
                attached: false,
                intent: e.announcement.intent.clone(),
                paths,
            });
        }
    }
    out
}

fn meeting_paths(mine: &[String], theirs: &[String]) -> Vec<OverlapPath> {
    let mut paths = Vec::new();
    for m in mine {
        for t in theirs {
            if claims_meet(m, t) {
                paths.push(OverlapPath {
                    yours: m.clone(),
                    theirs: t.clone(),
                });
            }
        }
    }
    paths
}

/// A `SystemTime` as RFC 3339 in UTC, to the second (`2026-09-05T12:34:56Z`).
///
/// ```
/// use std::time::{Duration, UNIX_EPOCH};
/// use majordomus_cli::peers::rfc3339;
/// assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
/// assert_eq!(rfc3339(UNIX_EPOCH + Duration::from_secs(1_788_000_000)), "2026-08-29T10:40:00Z");
/// ```
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil-from-days, Howard Hinnant's algorithm
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}-{mth:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// The inverse of [`rfc3339`]: seconds since the epoch, for exactly that form.
///
/// ```
/// use majordomus_cli::peers::{parse_rfc3339, rfc3339};
/// use std::time::{Duration, UNIX_EPOCH};
/// assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
/// assert_eq!(parse_rfc3339("2026-08-29T10:40:00Z"), Some(1_788_000_000));
/// assert_eq!(parse_rfc3339(&rfc3339(UNIX_EPOCH + Duration::from_secs(1_757_500_000))), Some(1_757_500_000));
/// assert_eq!(parse_rfc3339("yesterday"), None);
/// ```
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let t = text.strip_suffix('Z')?;
    let (date, time) = t.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (y, mth, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let mut c = time.split(':').map(|p| p.parse::<i64>());
    let (h, m, s) = (c.next()?.ok()?, c.next()?.ok()?, c.next()?.ok()?);
    if !(1..=12).contains(&mth) || !(1..=31).contains(&day) || h > 23 || m > 59 || s > 60 {
        return None;
    }
    // days-from-civil, Howard Hinnant's algorithm
    let y = if mth <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if mth > 2 { mth - 3 } else { mth + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + m * 60 + s;
    u64::try_from(secs).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board_with(claims: &[(&str, &[&str])]) -> (PeerBoard, Vec<PeerId>) {
        let board = PeerBoard::new();
        let mut ids = Vec::new();
        for (intent, scope) in claims {
            let id = board.attach(Transport::Http);
            board.announce(&id, intent, scope.iter().map(|s| s.to_string()).collect());
            ids.push(id);
        }
        (board, ids)
    }

    #[test]
    fn a_claim_meets_one_that_contains_it_and_not_one_that_merely_starts_the_same() {
        assert!(claims_meet("apps", "apps/majordomus-cli/src/order.rs"));
        assert!(claims_meet("apps/majordomus-cli/src/order.rs", "apps"));
        assert!(claims_meet("docs", "docs"));
        assert!(
            !claims_meet("app", "apps"),
            "a claim is a path, not a string prefix"
        );
        assert!(!claims_meet(
            "apps/majordomus-cli/src/order.rs",
            "apps/majordomus-cli/src/peers.rs"
        ));
        assert!(!claims_meet("", "apps"), "nothing claimed meets nothing");
        assert!(!claims_meet("  ", "apps"));
    }

    #[test]
    fn a_trailing_slash_is_the_same_claim() {
        assert!(claims_meet("docs/", "docs"));
        assert!(claims_meet("docs", "docs/"));
        assert!(claims_meet("apps/", "apps/majordomus-cli"));
    }

    #[test]
    fn announcing_over_another_peers_ground_says_whose_it_is() {
        let (board, ids) = board_with(&[("the ordering rule", &["apps/majordomus-cli/src"])]);
        let mine = board.attach(Transport::Http);
        let answer = board
            .announce(
                &mine,
                "the release model",
                vec!["apps/majordomus-cli/src/release".into()],
            )
            .expect("the peer is attached");

        assert_eq!(answer.overlaps.len(), 1);
        let hit = &answer.overlaps[0];
        assert_eq!(hit.peer, ids[0]);
        assert_eq!(hit.intent, "the ordering rule");
        assert!(hit.attached);
        assert_eq!(hit.paths.len(), 1);
        assert_eq!(hit.paths[0].yours, "apps/majordomus-cli/src/release");
        assert_eq!(hit.paths[0].theirs, "apps/majordomus-cli/src");
    }

    #[test]
    fn a_peer_that_claims_ground_nobody_holds_is_told_it_is_alone() {
        let (board, _) = board_with(&[("the ordering rule", &["apps/majordomus-cli/src"])]);
        let mine = board.attach(Transport::Http);
        let answer = board
            .announce(&mine, "the site", vec!["site/templates".into()])
            .unwrap();
        assert!(answer.overlaps.is_empty(), "{:?}", answer.overlaps);
    }

    #[test]
    fn an_overlap_names_every_pair_of_paths_that_meet_and_no_others() {
        let (board, _) = board_with(&[("theirs", &["docs", "site", "test/cases"])]);
        let mine = board.attach(Transport::Http);
        let answer = board
            .announce(
                &mine,
                "mine",
                vec!["docs/MCP.md".into(), "apps".into(), "site".into()],
            )
            .unwrap();
        let pairs: Vec<(String, String)> = answer.overlaps[0]
            .paths
            .iter()
            .map(|p| (p.yours.clone(), p.theirs.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("docs/MCP.md".to_string(), "docs".to_string()),
                ("site".to_string(), "site".to_string()),
            ],
            "apps meets nothing they claimed, and test/cases meets nothing I claimed"
        );
    }

    #[test]
    fn an_announcement_outlives_the_connection_that_made_it() {
        // The defect this exists for: a session that reconnects used to vanish from the
        // board with everything it had said, and nobody — including the session itself —
        // was told. What it said it was working on is a fact about the repository.
        let board = PeerBoard::new();
        let first = board.attach(Transport::Http);
        board.announce(&first, "the ordering rule", vec!["apps".into()]);
        board.detach(&first);

        let listed = board.list();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].attached, "it is listed as gone, not as present");
        assert_eq!(
            listed[0].announcement.as_ref().unwrap().intent,
            "the ordering rule"
        );
        assert_eq!(board.len(), 0, "the lifecycle counts who is here now");

        // and the ground it claimed is still defended
        let second = board.attach(Transport::Http);
        let answer = board
            .announce(
                &second,
                "something else",
                vec!["apps/majordomus-cli".into()],
            )
            .unwrap();
        assert_eq!(answer.overlaps.len(), 1);
        assert!(
            !answer.overlaps[0].attached,
            "the reader is told the holder is gone"
        );
    }

    #[test]
    fn a_peer_that_said_nothing_leaves_nothing_behind() {
        let board = PeerBoard::new();
        let quiet = board.attach(Transport::Http);
        board.detach(&quiet);
        assert!(board.list().is_empty(), "a connection is not news");
    }

    #[test]
    fn a_returning_peer_that_announces_again_is_attached_once_more() {
        let board = PeerBoard::new();
        let id = board.attach(Transport::Http);
        board.announce(&id, "first", vec!["apps".into()]);
        board.detach(&id);
        assert!(!board.list()[0].attached);
        board.announce(&id, "second", vec!["apps".into()]);
        assert!(board.list()[0].attached, "it spoke, so it is here");
        assert_eq!(board.len(), 1);
    }

    #[test]
    fn the_board_keeps_a_bounded_number_of_departed_peers() {
        let board = PeerBoard::new();
        for i in 0..RETAINED + 8 {
            let id = board.attach(Transport::Http);
            board.announce(&id, &format!("intent {i}"), vec!["apps".into()]);
            board.detach(&id);
        }
        assert_eq!(
            board.list().len(),
            RETAINED,
            "a server that ran all day is not a museum"
        );
        let kept: Vec<String> = board
            .list()
            .iter()
            .filter_map(|p| p.announcement.as_ref().map(|a| a.intent.clone()))
            .collect();
        assert!(
            kept.contains(&format!("intent {}", RETAINED + 7)),
            "the newest is kept"
        );
        assert!(
            !kept.contains(&"intent 0".to_string()),
            "the oldest is dropped"
        );
    }

    #[test]
    fn an_attached_peer_is_never_dropped_to_make_room() {
        let board = PeerBoard::new();
        let live = board.attach(Transport::Http);
        board.announce(&live, "still working", vec!["apps".into()]);
        for i in 0..RETAINED + 4 {
            let id = board.attach(Transport::Http);
            board.announce(&id, &format!("gone {i}"), vec!["site".into()]);
            board.detach(&id);
        }
        assert!(
            board.list().iter().any(|p| p.id == live && p.attached),
            "the live session was evicted by departed ones"
        );
        assert_eq!(board.len(), 1);
    }

    #[test]
    fn every_colliding_pair_is_reported_once_by_the_board() {
        let (board, _) = board_with(&[
            ("a", &["apps/majordomus-cli/src"]),
            ("b", &["apps/majordomus-cli/src/release"]),
            ("c", &["site"]),
        ]);
        let overlaps = board.overlaps();
        assert_eq!(
            overlaps.len(),
            1,
            "one pair meets, and it is named once: {overlaps:?}"
        );
        assert_eq!(
            overlaps[0].intent, "a",
            "the pair is reported against the later peer"
        );
    }

    #[test]
    fn announcing_as_a_peer_that_is_not_on_the_board_answers_nothing() {
        let board = PeerBoard::new();
        assert!(board.announce(&PeerId::new(99), "hello", vec![]).is_none());
    }

    // ------------------------------------------------------------ the journal

    const GEN_A: &str = "2026-09-09T21:18:39Z";
    const GEN_B: &str = "2026-09-09T23:18:39Z";
    const GEN_C: &str = "2026-09-10T04:00:00Z";

    fn line(generation: &str, peer: &str, intent: &str, scope: &[&str]) -> String {
        serde_json::to_string(&JournalLine {
            schema: JOURNAL_SCHEMA.into(),
            generation: generation.into(),
            peer: peer.into(),
            client: ClientInfo {
                name: "claude-code".into(),
                version: "2".into(),
                title: None,
            },
            transport: Transport::Http,
            connected_at: generation.into(),
            announcement: Announcement {
                intent: intent.into(),
                scope: scope.iter().map(|s| s.to_string()).collect(),
                at: generation.into(),
            },
        })
        .unwrap()
    }

    #[test]
    fn an_announcement_is_journalled_and_the_next_server_reads_it_back() {
        // The defect this exists for: the server restarted at 23:18, every announcement
        // was gone, and nine sessions were re-issued p1..p9 as if nobody had said anything.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp").join(JOURNAL_FILE);

        let first = PeerBoard::new();
        let opened = first.open_journal(path.clone(), GEN_A.into());
        assert_eq!(opened, JournalOpened::default(), "an absent journal is empty");
        let a = first.attach(Transport::Http);
        first.identify(
            &a,
            ClientInfo {
                name: "claude-code".into(),
                version: "2.1".into(),
                title: Some("Claude Code".into()),
            },
        );
        first.announce(&a, "the ordering rule", vec!["apps".into()]);
        first.announce(&a, "the ordering rule, second word", vec!["apps/x".into()]);

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2, "one line per announce, appended");
        let last: JournalLine = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last.schema, JOURNAL_SCHEMA);
        assert_eq!(last.generation, GEN_A);
        assert_eq!(last.peer, "p1");
        assert_eq!(last.client.name, "claude-code");
        assert_eq!(last.announcement.intent, "the ordering rule, second word");

        let second = PeerBoard::new();
        let opened = second.open_journal(path, GEN_B.into());
        assert_eq!(opened.earlier, 1, "one peer, its last word");
        assert_eq!(opened.previous_generation.as_deref(), Some(GEN_A));
        assert_eq!(opened.skipped, 0);
        let listed = second.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.as_str(), "p1@2026-09-09T21:18:39Z");
        assert!(!listed[0].attached);
        assert_eq!(listed[0].generation.as_deref(), Some(GEN_A));
        assert_eq!(
            listed[0].announcement.as_ref().unwrap().intent,
            "the ordering rule, second word"
        );
        assert_eq!(second.len(), 0, "the lifecycle counts who is here now");

        // and the ground it claimed is still defended, against a live peer
        let live = second.attach(Transport::Http);
        let answer = second
            .announce(&live, "something else", vec!["apps/x/y".into()])
            .unwrap();
        assert_eq!(answer.overlaps.len(), 1);
        assert_eq!(answer.overlaps[0].peer.as_str(), "p1@2026-09-09T21:18:39Z");
        assert!(!answer.overlaps[0].attached);
        let board_wide = second.overlaps();
        assert_eq!(board_wide.len(), 1, "{board_wide:?}");
        assert!(second.summary().contains("previous server generation"));
    }

    #[test]
    fn only_the_previous_generation_is_shown_and_only_its_last_word_per_peer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(JOURNAL_FILE);
        std::fs::write(
            &path,
            [
                line(GEN_A, "p1", "old and gone", &["docs"]),
                line(GEN_B, "p1", "first word", &["apps"]),
                line(GEN_B, "p2", "another peer", &["site"]),
                line(GEN_B, "p1", "last word", &["apps/x"]),
            ]
            .join("\n"),
        )
        .unwrap();
        let board = PeerBoard::new();
        let opened = board.open_journal(path, GEN_C.into());
        assert_eq!(opened.previous_generation.as_deref(), Some(GEN_B));
        let listed = board.list();
        let ids: Vec<&str> = listed.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["p1@2026-09-09T23:18:39Z", "p2@2026-09-09T23:18:39Z"],
            "generation A is history, not the board"
        );
        assert_eq!(
            listed[0].announcement.as_ref().unwrap().intent,
            "last word",
            "the last line per peer wins"
        );
    }

    #[test]
    fn this_generations_own_lines_are_not_its_previous_generation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(JOURNAL_FILE);
        std::fs::write(&path, line(GEN_B, "p1", "mine", &["apps"])).unwrap();
        let board = PeerBoard::new();
        let opened = board.open_journal(path, GEN_B.into());
        assert_eq!(opened.earlier, 0);
        assert!(opened.previous_generation.is_none());
        assert!(board.list().is_empty());
    }

    #[test]
    fn a_corrupt_line_is_skipped_and_counted_never_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(JOURNAL_FILE);
        std::fs::write(
            &path,
            format!(
                "{{not json\n{}\n{{\"schema\":\"something-else/v9\"}}\n\n",
                line(GEN_A, "p3", "survives", &["lib"])
            ),
        )
        .unwrap();
        let board = PeerBoard::new();
        let opened = board.open_journal(path, GEN_B.into());
        assert_eq!(opened.skipped, 2);
        assert_eq!(opened.earlier, 1);
        assert_eq!(
            board.list()[0].announcement.as_ref().unwrap().intent,
            "survives"
        );
    }

    #[test]
    fn the_journal_is_compacted_to_the_cap_when_the_next_server_opens_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(JOURNAL_FILE);
        let lines: Vec<String> = (0..JOURNAL_RETAINED + 50)
            .map(|i| line(GEN_A, "p1", &format!("word {i}"), &["apps"]))
            .collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        let board = PeerBoard::new();
        let opened = board.open_journal(path.clone(), GEN_B.into());
        assert!(opened.compacted);
        let kept = std::fs::read_to_string(&path).unwrap();
        assert_eq!(kept.lines().count(), JOURNAL_RETAINED);
        assert!(kept.lines().next().unwrap().contains("word 50"), "the oldest went");
        assert!(kept.lines().last().unwrap().contains(&format!("word {}", JOURNAL_RETAINED + 49)));
        assert_eq!(
            board.list()[0].announcement.as_ref().unwrap().intent,
            format!("word {}", JOURNAL_RETAINED + 49)
        );
    }

    #[test]
    fn a_board_without_a_journal_writes_nothing_anywhere() {
        let board = PeerBoard::new();
        let a = board.attach(Transport::Stdio);
        assert!(board.announce(&a, "standalone", vec![]).is_some());
        assert!(lock_of(&board.journal).is_none());
    }

    #[test]
    fn the_seq_of_an_earlier_id_is_read_through_the_generation_suffix() {
        assert_eq!(peer_seq(&PeerId::earlier("p4", GEN_A)), 4);
        assert_eq!(peer_seq(&PeerId::new(7)), 7);
        assert_eq!(peer_seq(&PeerId("nonsense".into())), u64::MAX);
    }
}
