//! Peers: the clients attached to one shared server. Every MCP session, the owner's
//! stdio one and every one that arrived over HTTP, is a peer from the moment it attaches;
//! `initialize` gives it a name (the client's own `clientInfo`), and `peers.announce`
//! lets it say what it is working on. The board lives in the server's memory and nowhere
//! else: it ends with the process, and nothing here touches the repository.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Its announcement, when it made one.
    pub announcement: Option<Announcement>,
}

struct Slot {
    client: ClientInfo,
    transport: Transport,
    connected_at: SystemTime,
    last_seen: Instant,
    announcement: Option<Announcement>,
}

/// The peers of one server process.
#[derive(Default)]
pub struct PeerBoard {
    next: AtomicU64,
    slots: Mutex<BTreeMap<u64, Slot>>,
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

    /// Record what a peer is working on. `None` when the id is not attached.
    pub fn announce(&self, id: &PeerId, intent: &str, scope: Vec<String>) -> Option<Peer> {
        let mut slots = self.lock();
        let s = slots.get_mut(&id.seq())?;
        s.last_seen = Instant::now();
        s.announcement = Some(Announcement {
            intent: intent.to_string(),
            scope,
            at: rfc3339(SystemTime::now()),
        });
        Some(peer(id.seq(), s))
    }

    /// One peer, as listed.
    pub fn get(&self, id: &PeerId) -> Option<Peer> {
        self.lock().get(&id.seq()).map(|s| peer(id.seq(), s))
    }

    /// The session ended.
    pub fn detach(&self, id: &PeerId) {
        self.lock().remove(&id.seq());
    }

    /// Every peer, in attachment order.
    pub fn list(&self) -> Vec<Peer> {
        self.lock().iter().map(|(seq, s)| peer(*seq, s)).collect()
    }

    /// How many peers are attached.
    pub fn len(&self) -> usize {
        self.lock().len()
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
            .map(|p| match &p.announcement {
                Some(a) => format!(
                    "{} {} ({:?}: {})",
                    p.id, p.client.name, p.transport, a.intent
                ),
                None => format!("{} {} ({:?})", p.id, p.client.name, p.transport),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<u64, Slot>> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn peer(seq: u64, s: &Slot) -> Peer {
    Peer {
        id: PeerId::new(seq),
        client: s.client.clone(),
        transport: s.transport,
        connected_at: rfc3339(s.connected_at),
        last_seen_seconds_ago: s.last_seen.elapsed().as_secs(),
        announcement: s.announcement.clone(),
    }
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
