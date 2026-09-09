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
    /// Whether the session is still attached. A peer that announced something and then
    /// went away is kept and listed with `attached: false`: what it said it was working on
    /// outlives the connection that said it, because the work does.
    pub attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Its announcement, when it made one.
    pub announcement: Option<Announcement>,
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
        let mut slots = self.lock();
        if !slots.contains_key(&id.seq()) {
            return None;
        }
        let overlaps = overlaps_against(&slots, id.seq(), &scope);
        let s = slots.get_mut(&id.seq())?;
        s.last_seen = Instant::now();
        s.departed = false;
        s.announcement = Some(Announcement {
            intent: intent.to_string(),
            scope,
            at: rfc3339(SystemTime::now()),
        });
        Some(Announced {
            peer: peer(id.seq(), s),
            overlaps,
        })
    }

    /// Every pair of peers whose claims meet, each pair once, in peer order.
    pub fn overlaps(&self) -> Vec<Overlap> {
        let slots = self.lock();
        let mut out = Vec::new();
        for (seq, s) in slots.iter() {
            let Some(mine) = &s.announcement else {
                continue;
            };
            for o in overlaps_against(&slots, *seq, &mine.scope) {
                // each pair once: report it against the later peer only
                if o.peer.seq() < *seq {
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
    /// nothing behind. The oldest departed slot past [`RETAINED`] is dropped.
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
        attached: !s.departed,
        announcement: s.announcement.clone(),
    }
}

/// Every peer other than `seq` whose announced scope meets `scope`.
fn overlaps_against(slots: &BTreeMap<u64, Slot>, seq: u64, scope: &[String]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (other, s) in slots.iter() {
        if *other == seq {
            continue;
        }
        let Some(theirs) = &s.announcement else {
            continue;
        };
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
            });
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
}
