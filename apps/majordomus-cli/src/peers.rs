//! Peers: the clients attached to one shared server. Every MCP session, the owner's
//! stdio one and every one that arrived over HTTP, is a peer from the moment it attaches;
//! `initialize` gives it a name (the client's own `clientInfo`), and `peers.announce`
//! lets it say what it is working on. The board lives in the server's memory and nowhere
//! else: it ends with the process, and nothing here touches the repository.
//!
//! # Why it is in memory and stays there
//!
//! A peer board is about *now*: who is attached, what they said they are doing. Writing it
//! into the repository would make it a record of something that has already stopped being
//! true, and the layer's records are for what survives the session. The board ends with the
//! process, which is exactly right for a fact about the process.
//!
//! # What a peer may say
//!
//! An announcement is informational and enforces nothing. Its whole purpose is that another
//! agent working in the same checkout can read it and choose not to collide — the
//! coordination is between the peers, not imposed by the server.
//!
//! A peer may hold **several claims at once**, each under a name of its own. One connection
//! is not always one piece of work: a session that fans work out to subagents shares its MCP
//! session with all of them, so they are one peer here while being several workers there.
//! Before named claims the board could hold only the last thing said, and each announcement
//! silently erased the one before it — the other peers stopped being told about scope that
//! was still very much being written. Announcing under the same name updates that claim;
//! announcing under a new one leaves the others standing; announcing without a name is the
//! peer's one unnamed claim, which is all a single session ever needs.
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
use std::path::PathBuf;
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
    // `default` as well as `skip_serializing_if`: a board is now read back off the wire
    // from a sibling checkout's server, and a field that is skipped when absent must also
    // be accepted when absent, or the peer that omitted it makes the whole board unreadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// One claim a peer has made: what it is working on, and where.
///
/// A peer may hold several. The `name` is what tells them apart and what makes announcing
/// again an update rather than a second claim: announce under the same name and the claim
/// is replaced, announce under a new one and the peer now holds both. A peer that never
/// names anything holds exactly one claim, the unnamed one, which is what a single session
/// announcing about itself has always had.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Announcement {
    /// What this claim is called, when the peer named it. Absent for a peer's unnamed claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// One line: the task or intent, in the peer's words.
    pub intent: String,
    /// Repository-relative paths the peer expects to touch; informational, never enforced here.
    pub scope: Vec<String>,
    /// When it was announced, RFC 3339, UTC.
    pub at: String,
}

/// The key a claim is held under: its name, or the empty string for the unnamed claim.
///
/// Private, because the emptiness is an implementation of "unnamed" and not something a
/// reader of the board should ever see: [`Announcement::name`] is `None` there.
fn claim_key(name: Option<&str>) -> String {
    name.map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Which checkout of the repository a peer is attached to.
///
/// A board is one process's memory and a server serves one checkout, so a peer read from
/// a sibling checkout's server and a peer of this one are otherwise indistinguishable —
/// and worse than indistinguishable, because the ids collide: `p1` is the first session
/// of *every* board, so a repository-wide listing without this field would hold several
/// peers called `p1` and no way to tell them apart. This is the qualification. The
/// worktree path is the durable half of a worker's identity: the peer id is a position on
/// one board and is handed out again after a reconnect (`p3` this morning, `p1` after the
/// bridge re-attached), while the checkout a worker is working in outlives its connection.
///
/// Absent on a peer a server reports about its own board without being asked which
/// checkout that is — `peers.announce` answers about the announcing session and the
/// answer is this server's by construction. The repository-wide listing stamps every
/// peer it gathers, its own included, so that no entry of that answer is unqualified.
///
/// ```
/// use majordomus_cli::peers::PeerCheckout;
/// let c = PeerCheckout {
///     id: "b8293f11".into(),
///     worktree: "/repo-wt/feature/x".into(),
///     branch: Some("feature/x".into()),
///     this_checkout: false,
/// };
/// let v = serde_json::to_value(&c).unwrap();
/// assert_eq!(v["branch"], "feature/x");
/// assert_eq!(v["this_checkout"], false);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PeerCheckout {
    /// The checkout's identity digest, the same value `server.status` reports as
    /// `checkout_id`. One per checkout, never a path, and stable across a restart.
    pub id: String,
    /// Where the checkout is. The one thing on this board that a human can act on
    /// directly, and the one thing that survives a reconnect.
    pub worktree: PathBuf,
    /// The branch checked out there, when it could be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Whether this is the checkout of the server answering the call.
    pub this_checkout: bool,
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
    /// Every claim this peer holds, in name order, the unnamed one first. Empty for a peer
    /// that has announced nothing.
    ///
    /// One connection may be doing several things at once — a session that fans work out to
    /// subagents shares its MCP session with all of them — and before this field the board
    /// could hold only the last thing said, so each announcement silently erased the one
    /// before it and the other peers stopped seeing the rest of the scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<Announcement>,
    // `default` as well as `skip_serializing_if`, for the reason on `ClientInfo::title`:
    // a peer that has attached and not yet announced serializes without this field, and
    // before the board was gathered off the wire nothing ever read one back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The most recently made of [`Peer::claims`], for a reader that wants one line rather
    /// than all of them. Derived from `claims` and never a second account of it: a peer
    /// holding one claim reports the same thing in both, which is what every peer that does
    /// not name its claims reports.
    pub announcement: Option<Announcement>,
    /// Which checkout of the repository this peer is attached to; see [`PeerCheckout`].
    ///
    /// The board itself never sets it — a board does not know where it is — and it is
    /// stamped by the reader that gathered this peer, which is the one thing that knows
    /// which checkout's server it asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<PeerCheckout>,
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
    /// Which checkout the other peer is attached to, when the reader knew.
    ///
    /// The overlap that matters most is the one nobody could see before: two workers in
    /// two worktrees of one repository, each reading a board that held only itself, both
    /// claiming `apps/majordomus-cli/src`. Naming the checkout is what turns "p1 is on
    /// your ground" into something a reader can act on when `p1` is not on this board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<PeerCheckout>,
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
    /// Paths the claim this one replaced had, and this one does not.
    ///
    /// Announcing under a name a peer already used replaces that claim, which is what an
    /// update is. When the replacement is narrower, the peer has just stopped claiming
    /// ground it held a moment ago, and the other peers stop being warned off it. That is
    /// occasionally what was meant and is otherwise a mistake nobody would ever see, so it
    /// is said here, at the moment it happens, rather than left to be discovered in a
    /// collision later. Empty is the ordinary case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub released: Vec<String>,
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

struct Slot {
    client: ClientInfo,
    transport: Transport,
    connected_at: SystemTime,
    last_seen: Instant,
    departed: bool,
    /// The peer's claims, by [`claim_key`]. A `BTreeMap` rather than a `Vec` because
    /// announcing under a name a peer already used must replace that claim and not append
    /// a second one — idempotence is the map, not a search.
    claims: BTreeMap<String, Announcement>,
}

impl Slot {
    /// The claims, in name order, the unnamed one first.
    fn claims(&self) -> Vec<Announcement> {
        self.claims.values().cloned().collect()
    }

    /// The most recently made claim, which is what a one-line reader wants.
    fn latest(&self) -> Option<Announcement> {
        self.claims.values().max_by(|a, b| a.at.cmp(&b.at)).cloned()
    }
}

/// How many departed peers the board keeps. A retained announcement is the whole point —
/// a long session that reconnects must not vanish from the board — but a server that ran
/// all day must not accumulate every connection it ever saw either, so the oldest departed
/// slot is dropped past this. Attached peers are never dropped.
const RETAINED: usize = 32;

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
                departed: false,
                claims: BTreeMap::new(),
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

    /// Record a peer's unnamed claim, and answer with who else is on that ground.
    ///
    /// This is the whole of what a session announcing about itself needs, and it is what
    /// announcing has always meant: the peer holds one claim and each announcement replaces
    /// it. A connection doing several things at once wants [`PeerBoard::announce_claim`].
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
    /// let second = board
    ///     .announce(&b, "the release model", vec!["apps/majordomus-cli/src/release".into()])
    ///     .unwrap();
    /// assert_eq!(second.overlaps.len(), 1, "a claim inside another claim is an overlap");
    /// assert_eq!(second.overlaps[0].intent, "the ordering rule");
    ///
    /// // announcing again, narrower: the peer is told what it stopped claiming
    /// let again = board.announce(&b, "the release model", vec![]).unwrap();
    /// assert_eq!(again.released, vec!["apps/majordomus-cli/src/release".to_string()]);
    /// ```
    pub fn announce(&self, id: &PeerId, intent: &str, scope: Vec<String>) -> Option<Announced> {
        self.announce_claim(id, None, intent, scope)
    }

    /// The same, naming which of the peer's claims this is.
    ///
    /// `None` is [`PeerBoard::announce`]: the peer's one unnamed claim. Every other value
    /// is a claim of its own, held beside the rest until it is announced again under the
    /// same name.
    ///
    /// ```
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let board = PeerBoard::new();
    /// let fleet = board.attach(Transport::Http);
    /// board.announce(&fleet, "the whole mandate", vec!["docs".into()]);
    /// let named = board
    ///     .announce_claim(&fleet, Some("worker-a"), "the guard", vec![".claude".into()])
    ///     .unwrap();
    /// assert_eq!(named.peer.claims.len(), 2, "the unnamed claim still stands");
    /// assert_eq!(named.peer.claims[1].name.as_deref(), Some("worker-a"));
    /// ```
    pub fn announce_claim(
        &self,
        id: &PeerId,
        claim: Option<&str>,
        intent: &str,
        scope: Vec<String>,
    ) -> Option<Announced> {
        let mut slots = self.lock();
        if !slots.contains_key(&id.seq()) {
            return None;
        }
        let overlaps = overlaps_against(&slots, id.seq(), &scope);
        let key = claim_key(claim);
        let s = slots.get_mut(&id.seq())?;
        s.last_seen = Instant::now();
        s.departed = false;
        let released = s
            .claims
            .get(&key)
            .map(|old| {
                old.scope
                    .iter()
                    .filter(|held| !scope.iter().any(|now| claims_meet(held, now)))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        s.claims.insert(
            key.clone(),
            Announcement {
                name: (!key.is_empty()).then(|| key.clone()),
                intent: intent.to_string(),
                scope,
                at: rfc3339(SystemTime::now()),
            },
        );
        Some(Announced {
            peer: peer(id.seq(), s),
            overlaps,
            released,
        })
    }

    /// Every pair of claims that meet, each pair once, in peer order.
    ///
    /// A peer holding several claims is asked about each of them, so two connections that
    /// collide on two different claims are reported twice, with the intent of each. That is
    /// the answer a reader wants: it is not "these two sessions overlap" but "these two
    /// pieces of work overlap", and the intent says which.
    pub fn overlaps(&self) -> Vec<Overlap> {
        overlaps_among(&self.list())
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
            Some(s) if !s.claims.is_empty() => s.departed = true,
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

    /// Every peer, in attachment order.
    pub fn list(&self) -> Vec<Peer> {
        self.lock().iter().map(|(seq, s)| peer(*seq, s)).collect()
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
            .map(|p| {
                if p.claims.is_empty() {
                    return format!("{} {} ({:?})", p.id, p.client.name, p.transport);
                }
                let claims = p
                    .claims
                    .iter()
                    .map(|a| match &a.name {
                        Some(n) => format!("{n}: {}", a.intent),
                        None => a.intent.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                format!("{} {} ({:?}: {claims})", p.id, p.client.name, p.transport)
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
        attached: !s.departed,
        claims: s.claims(),
        announcement: s.latest(),
        // a board does not know which checkout it is the board of; whoever gathered this
        // peer stamps it (`peers.list`), because that is the reader that asked a server
        checkout: None,
    }
}

/// Whether this peer is attached and has claimed nothing.
///
/// The one definition of *silent* in this crate. A silent peer is the board's blind spot:
/// it is a worker, it is here now, and the board can say nothing about what it will touch.
/// Every reader that wants to talk about silence — the `initialize` instructions, the
/// answer of `peers.list` — asks this function rather than re-deriving the predicate, so
/// that two surfaces can never come to disagree about what "announced nothing" means
/// (ADR 0004: a repeated semantic definition across projections is a design defect).
///
/// A *departed* peer is not silent, whatever it announced: it is not here, and the point
/// of the number is how much of the ground being worked right now is unclaimed. A departed
/// peer that did announce is the opposite problem — a claim standing over ground nobody
/// holds — and it is counted separately, where it is reported.
///
/// ```
/// use majordomus_cli::peers::{is_silent, silent_among, PeerBoard, Transport};
/// let board = PeerBoard::new();
/// let quiet = board.attach(Transport::Stdio);
/// let spoken = board.attach(Transport::Http);
/// board.announce(&spoken, "the ordering rule", vec!["apps/majordomus-cli/src".into()]);
/// let peers = board.list();
/// assert!(is_silent(&peers[0]), "attached and claiming nothing");
/// assert!(!is_silent(&peers[1]), "it said what it is doing");
/// // A silent peer that leaves stops being a blind spot by leaving the board outright:
/// // `detach` retains a peer only for the claim it made, and this one made none. So the
/// // `attached` term of the predicate is NOT what is exercised here — no board this type
/// // builds can list a peer that is both departed and silent. That term is reachable only
/// // where a sibling server's board is parsed off the wire, and it is proven there, in
/// // `a_departed_peer_is_not_silent_however_it_arrives`.
/// board.detach(&quiet);
/// assert_eq!(board.list().len(), 1, "the silent peer is gone, not retained");
/// assert_eq!(silent_among(&board.list()), 0, "and nobody left here is quiet");
/// ```
pub fn is_silent(peer: &Peer) -> bool {
    peer.attached && peer.claims.is_empty()
}

/// How many of an arbitrary set of peers are attached and have claimed nothing.
///
/// Computed over the union by whoever gathered the peers, exactly like [`overlaps_among`]
/// and for the same reason: silence is a property of the set that was actually read, and
/// no single board knows what the others hold.
///
/// This is the number that separates two answers a reader must never confuse. A board of
/// `count: 0` means nobody is working in this repository. A board of `count: 5, silent: 5`
/// means five workers are here and not one of them has said what it is touching — the
/// same absence of claims, the opposite situation, and only the second is a reason to stop
/// and ask before allocating an identifier or opening a mandate. Without this number a
/// reader sees an empty `overlaps` and concludes there is no collision, when the truth is
/// that the board was never in a position to find one.
///
/// ```
/// use majordomus_cli::peers::{silent_among, PeerBoard, Transport};
/// let board = PeerBoard::new();
/// board.attach(Transport::Stdio);
/// board.attach(Transport::Http);
/// let spoken = board.attach(Transport::Http);
/// board.announce(&spoken, "the release model", vec!["apps/majordomus-cli/src/release".into()]);
/// assert_eq!(silent_among(&board.list()), 2, "three here, one of them speaking");
/// ```
pub fn silent_among(peers: &[Peer]) -> usize {
    peers.iter().filter(|p| is_silent(p)).count()
}

/// Every pair of claims that meet among an arbitrary set of peers, each pair once.
///
/// The listing side of the overlap question, and the only account of it: [`PeerBoard::overlaps`]
/// is this function over its own peers, and the repository-wide listing is this function
/// over the peers of every checkout's board. They cannot drift into two answers, and a
/// collision between two worktrees is found by exactly the algorithm that found a
/// collision between two sessions of one — which is the whole point, because until the
/// board became repository-wide the second kind was the only kind anyone could see.
///
/// A pair is reported once, against the later peer of the two, naming the earlier one:
/// `yours` is the later peer's path and `theirs` the earlier's, and [`Overlap::peer`]
/// with [`Overlap::checkout`] says who and where the earlier one is. Peers are paired by
/// position, never by [`PeerId`], because ids are unique to one board and a gathered list
/// holds a `p1` per checkout.
///
/// ```
/// use majordomus_cli::peers::{overlaps_among, PeerBoard, Transport};
/// let board = PeerBoard::new();
/// let a = board.attach(Transport::Stdio);
/// let b = board.attach(Transport::Http);
/// board.announce(&a, "the ordering rule", vec!["apps/majordomus-cli/src".into()]);
/// board.announce(&b, "the release model", vec!["apps/majordomus-cli/src/release".into()]);
/// let found = overlaps_among(&board.list());
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].peer.as_str(), "p1", "reported against the later peer, naming the earlier");
/// assert_eq!(found[0].paths[0].yours, "apps/majordomus-cli/src/release");
/// assert_eq!(found[0].paths[0].theirs, "apps/majordomus-cli/src");
/// // and it is the same answer the board gives about itself
/// assert_eq!(board.overlaps(), found);
/// ```
pub fn overlaps_among(peers: &[Peer]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (i, later) in peers.iter().enumerate() {
        for mine in &later.claims {
            for earlier in peers.iter().take(i) {
                for theirs in &earlier.claims {
                    let mut paths: Vec<OverlapPath> = Vec::new();
                    for yours in &mine.scope {
                        for their in &theirs.scope {
                            if claims_meet(yours, their) {
                                paths.push(OverlapPath {
                                    yours: yours.clone(),
                                    theirs: their.clone(),
                                });
                            }
                        }
                    }
                    if !paths.is_empty() {
                        out.push(Overlap {
                            peer: earlier.id.clone(),
                            attached: earlier.attached,
                            intent: theirs.intent.clone(),
                            paths,
                            checkout: earlier.checkout.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// Every claim of every peer other than `seq` whose scope meets `scope`.
///
/// One entry per claim, not per peer: a connection holding two claims that both meet this
/// scope is two entries, because they are two pieces of work and the reader needs the
/// intent of each to know which one to talk about.
fn overlaps_against(slots: &BTreeMap<u64, Slot>, seq: u64, scope: &[String]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (other, s) in slots.iter() {
        if *other == seq {
            continue;
        }
        for theirs in s.claims.values() {
            let mut paths: Vec<OverlapPath> = Vec::new();
            for mine in scope {
                for path in &theirs.scope {
                    if claims_meet(mine, path) {
                        paths.push(OverlapPath {
                            yours: mine.clone(),
                            theirs: path.clone(),
                        });
                    }
                }
            }
            if !paths.is_empty() {
                out.push(Overlap {
                    peer: PeerId::new(*other),
                    attached: !s.departed,
                    intent: theirs.intent.clone(),
                    paths,
                    // this board's own peers: the caller is on this checkout too, and the
                    // one answer that gathers several boards stamps them all itself
                    checkout: None,
                });
            }
        }
    }
    out
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

/// The inverse of [`rfc3339`]: `2026-08-29T10:40:00Z` back to Unix seconds.
///
/// It lives beside the formatter because the two are one thing, and a repository that had
/// the one without the other grew a second parser every time somebody needed to compare two
/// recorded instants. Strict about the shape it accepts — exactly `YYYY-MM-DDTHH:MM:SSZ`,
/// which is the only shape this tool writes — because a lenient parser that guessed at a
/// half-recognised string would answer `None` for a *malformed* timestamp and a plausible
/// number for a *different* one, and only the first of those is safe.
///
/// `None` is the honest answer for anything else, and the caller's job is to report it as
/// unjudgeable rather than as fresh: an episode whose evidence cannot be read as a time is
/// one a recovery sweep must skip and count, never one it may sweep.
///
/// ```
/// use majordomus_cli::peers::{epoch_seconds, rfc3339};
/// use std::time::{Duration, UNIX_EPOCH};
///
/// assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
/// assert_eq!(epoch_seconds("2026-08-29T10:40:00Z"), Some(1_788_000_000));
/// assert_eq!(epoch_seconds("not a timestamp"), None);
/// assert_eq!(epoch_seconds("2026-08-29 10:40:00Z"), None, "the T is not optional");
/// assert_eq!(epoch_seconds("2026-08-29T10:40:00+02:00"), None, "UTC or nothing");
///
/// // and it round-trips with the formatter it is the inverse of
/// for secs in [0u64, 1, 1_788_000_000, 2_000_000_000] {
///     let text = rfc3339(UNIX_EPOCH + Duration::from_secs(secs));
///     assert_eq!(epoch_seconds(&text), Some(secs as i64), "{text}");
/// }
/// ```
pub fn epoch_seconds(ts: &str) -> Option<i64> {
    let b = ts.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let n = |from: usize, to: usize| -> Option<i64> {
        let part = ts.get(from..to)?;
        part.bytes().all(|c| c.is_ascii_digit()).then_some(())?;
        part.parse::<i64>().ok()
    };
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hh, mm, ss) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // days-from-civil, Howard Hinnant's algorithm — the inverse of the civil-from-days
    // above, so the two agree by construction rather than by two tables being kept level.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `is_silent` asks whether the peer is attached as well as whether it claimed
    /// anything, and through a [`PeerBoard`] that question can only ever be answered one
    /// way: [`PeerBoard::detach`] *removes* a peer holding no claims outright and only
    /// retains one that announced, so no board this crate builds can list a peer that is
    /// both departed and silent. The behavioural case cannot reach the state either, and a
    /// mutation that dropped the `attached` term survived the whole suite because of it.
    ///
    /// It is reachable over the wire, which is the only place it matters. A gathered board
    /// is `serde`-parsed from a sibling server's answer, that sibling is a separate process
    /// of a possibly different version, and `silent_among` is documented as taking an
    /// arbitrary set of peers. So the guard is tested where the input actually comes from:
    /// the JSON a sibling sends, not a board this test builds.
    ///
    /// What it protects is the meaning of the number. `silent` counts ground being worked
    /// *now* with nobody saying what they will touch. A departed peer that claimed nothing
    /// is not that — it is nothing at all — and counting it would inflate the one number a
    /// worker reads to decide whether the board can be trusted, in the direction that makes
    /// a quiet board look busier than it is.
    #[test]
    fn a_departed_peer_is_not_silent_however_it_arrives() {
        let wire = serde_json::json!([
            {
                "id": "p1",
                "client": { "name": "worker-gone", "version": "0" },
                "transport": "http",
                "connected_at": "2026-09-12T21:00:00Z",
                "last_seen_seconds_ago": 900,
                "attached": false
            },
            {
                "id": "p2",
                "client": { "name": "worker-here", "version": "0" },
                "transport": "http",
                "connected_at": "2026-09-12T21:05:00Z",
                "last_seen_seconds_ago": 1,
                "attached": true
            }
        ]);
        let peers: Vec<Peer> = serde_json::from_value(wire).expect("a sibling board parses");
        assert!(
            !is_silent(&peers[0]),
            "a peer that is not here is not a blind spot, whatever it claimed"
        );
        assert!(is_silent(&peers[1]), "attached and claiming nothing");
        assert_eq!(
            silent_among(&peers),
            1,
            "only the worker that is actually here counts"
        );
    }

    /// A board is now read back off the wire: `peers.list` gathers a sibling checkout's
    /// board from that checkout's own server, and `serde` parses the answer into these
    /// types. Every shape a board can serialise must therefore parse again — including the
    /// two nothing had ever read back before the gather existed: a peer that has attached
    /// and not yet announced (no `announcement`), and a client that sent no `title`. Both
    /// are skipped when absent, and a field that is skipped must also be defaulted, or one
    /// silent peer makes an entire sibling board unreadable and the answer `complete:
    /// false` — which is the failure this decision exists to remove, arriving by another
    /// door. Caught in review of ADR 0044, before it ever ran.
    #[test]
    fn a_board_survives_the_wire_including_a_peer_that_said_nothing() {
        let board = PeerBoard::new();
        let quiet = board.attach(Transport::Http);
        let listed = board.list();
        let text = serde_json::to_string(&listed[0]).expect("a peer serialises");
        assert!(
            !text.contains("announcement"),
            "a peer that said nothing carries no announcement: {text}"
        );
        assert!(
            !text.contains("title"),
            "and no title, because its client sent none: {text}"
        );
        let back: Peer = serde_json::from_str(&text).expect("a silent peer must read back");
        assert_eq!(back, listed[0]);
        assert!(
            back.checkout.is_none(),
            "a board does not know which checkout it is the board of"
        );

        // and the shape a peer that has said everything has
        board.announce(&quiet, "the ordering rule", vec!["apps".into()]);
        let spoken = board.list().remove(0);
        let text = serde_json::to_string(&spoken).unwrap();
        assert_eq!(serde_json::from_str::<Peer>(&text).unwrap(), spoken);

        // the whole answer, as a sibling would send it and the gather would read it
        let whole = serde_json::json!({ "count": 1, "peers": board.list() });
        let peers: Vec<Peer> = serde_json::from_value(whole["peers"].clone())
            .expect("the array the gather reads out of a sibling's answer");
        assert_eq!(peers, board.list());
    }

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

    /// One MCP session is not always one piece of work. A client that fans work out to
    /// subagents shares its session with them, so they arrive here as one peer; before named
    /// claims each announcement replaced the last, and the board ended up describing
    /// whichever worker spoke most recently while the rest of the scope silently stopped
    /// being claimed. This is that failure, and its absence.
    #[test]
    fn a_peer_holds_every_claim_it_names() {
        let board = PeerBoard::new();
        let fleet = board.attach(Transport::Http);

        board.announce(&fleet, "the whole mandate", vec!["docs".into()]);
        board.announce_claim(
            &fleet,
            Some("worker-a"),
            "the guard",
            vec![".claude".into()],
        );
        let third = board
            .announce_claim(&fleet, Some("worker-b"), "the cockpit", vec!["site".into()])
            .unwrap();

        assert_eq!(
            third.peer.claims.len(),
            3,
            "three claims, not the newest one"
        );
        let intents: Vec<&str> = third
            .peer
            .claims
            .iter()
            .map(|c| c.intent.as_str())
            .collect();
        assert_eq!(
            intents,
            vec!["the whole mandate", "the guard", "the cockpit"],
            "the unnamed claim first, then the named ones in name order"
        );
        assert_eq!(
            third.peer.claims[0].name, None,
            "the unnamed claim is unnamed"
        );
        assert_eq!(third.peer.claims[1].name.as_deref(), Some("worker-a"));

        // and every one of them is still ground another peer is warned off
        let other = board.attach(Transport::Http);
        let answer = board
            .announce(
                &other,
                "unrelated",
                vec![".claude/hooks".into(), "site".into()],
            )
            .unwrap();
        let met: Vec<&str> = answer.overlaps.iter().map(|o| o.intent.as_str()).collect();
        assert_eq!(
            met,
            vec!["the guard", "the cockpit"],
            "a claim of a peer is an overlap whichever of its claims it is"
        );
    }

    /// Announcing under a name the peer already used is an update, not a second claim — and
    /// when the update is narrower the peer is told which ground it just stopped holding,
    /// because nobody would otherwise ever see it happen.
    #[test]
    fn re_announcing_a_name_updates_it_and_says_what_it_released() {
        let board = PeerBoard::new();
        let peer = board.attach(Transport::Http);
        board.announce_claim(
            &peer,
            Some("worker-a"),
            "the guard",
            vec![".claude".into(), "scripts/ci".into()],
        );
        let again = board
            .announce_claim(
                &peer,
                Some("worker-a"),
                "the guard, narrowed",
                vec![".claude".into()],
            )
            .unwrap();

        assert_eq!(
            again.peer.claims.len(),
            1,
            "the same name is the same claim"
        );
        assert_eq!(again.peer.claims[0].intent, "the guard, narrowed");
        assert_eq!(
            again.released,
            vec!["scripts/ci".to_string()],
            "the path it stopped claiming is named at the moment it stops"
        );

        // widening releases nothing, and neither does re-stating the same scope
        let wider = board
            .announce_claim(
                &peer,
                Some("worker-a"),
                "the guard, wider",
                vec![".claude".into(), "scripts/ci".into()],
            )
            .unwrap();
        assert!(wider.released.is_empty());
        let same = board
            .announce_claim(
                &peer,
                Some("worker-a"),
                "the guard, wider",
                vec![".claude".into(), "scripts/ci".into()],
            )
            .unwrap();
        assert!(
            same.released.is_empty(),
            "announcing the same thing releases nothing"
        );

        // a claim inside the one it replaces is not a release: the ground is still held
        let inside = board
            .announce_claim(
                &peer,
                Some("worker-a"),
                "deeper",
                vec![".claude/hooks".into()],
            )
            .unwrap();
        assert_eq!(
            inside.released,
            vec!["scripts/ci".to_string()],
            ".claude is still covered by .claude/hooks; scripts/ci is not covered by anything"
        );
    }

    /// A peer that announced anything at all is kept on the board when its session ends —
    /// including one whose only claims are named ones, which the departure test's unnamed
    /// announcement would not have caught.
    #[test]
    fn a_peer_with_only_named_claims_outlives_its_session() {
        let board = PeerBoard::new();
        let peer = board.attach(Transport::Http);
        board.announce_claim(&peer, Some("worker-a"), "the guard", vec![".claude".into()]);
        board.detach(&peer);
        let listed = board.list();
        assert_eq!(
            listed.len(),
            1,
            "what it said it was working on outlives the socket"
        );
        assert!(!listed[0].attached);
        assert_eq!(listed[0].claims.len(), 1);
        assert_eq!(listed[0].claims[0].name.as_deref(), Some("worker-a"));
    }

    /// The one-line summary an `initialize` hands a new client names every claim, so a client
    /// arriving into a fleet of fanned-out workers sees all of them and not just the last.
    #[test]
    fn the_summary_names_every_claim() {
        let board = PeerBoard::new();
        let peer = board.attach(Transport::Http);
        board.announce(&peer, "the whole mandate", vec![]);
        board.announce_claim(&peer, Some("worker-a"), "the guard", vec![]);
        let line = board.summary();
        assert!(line.contains("the whole mandate"), "{line}");
        assert!(line.contains("worker-a: the guard"), "{line}");
    }
}
