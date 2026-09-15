//! Cooperation: the runtime that turns discovered, trusted runtimes of one repository into
//! linked peers and keeps their journal converged. It owns the one peer-link table, the
//! one journal of this runtime, the link handlers the HTTP surface answers, the dialers,
//! the heartbeat and the expiry — and the operations (sessions, claims, handovers,
//! reviews) every surface performs through it.
//!
//! The sequence one link goes through, and where each step is decided:
//!
//! ```text
//! candidate  a trusted registry record of the same repository, or a declared seed
//!    ↓
//! dial       hello → welcome  (link::)  signature, freshness, nonce, protocol,
//!                                        repository, self, trust — both directions
//!    ↓
//! connected  sync every heartbeat: marks + missing events, both ways, signed
//!    ↓        a failed round: degraded → unreachable (backoff, capped at the expiry)
//! expired    no exchange within the expiry: the link is dropped, the runtime's
//!    ↓        streams stop beating everywhere, its claims stop excluding
//! reconnect  the dialer says hello again; an unknown link id or a new instance is a
//!             re-handshake, never a second peer — the table is keyed by runtime
//! ```
//!
//! StreamLiveness has two layers and they are deliberately separate. A *link* is this runtime's
//! connection to one peer (connected, degraded, unreachable, expired). A *stream* is
//! alive while its beat keeps rising anywhere in the mesh — a runtime linked only through
//! a relay is still alive, and a runtime that crashed stops beating for everyone at once.
//! Claims follow streams, not links.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::config::{CooperationConfig, TrustConfig};
use super::identity::{node_id_of_key, NodeIdentity};
use super::journal::{
    ClaimMode, EventBody, HandoverBody, Journal, JournalTallies, Marks, MeshEvent, Rejection,
    SessionInfo, StreamId, StreamLiveness,
};
use super::link::{
    fresh_token, negotiate, sign, verify_signed, Domain, Hello, LinkRefusal, LinkReply,
    LinkTransport, RefusalCode, RuntimeCard, Signed, SyncAnswer, SyncRequest, Welcome, FEATURES,
    HELLO_PATH, LINK_PROTOCOL_MAX, LINK_PROTOCOL_MIN, MAX_LINK_MESSAGE, MAX_LINK_SKEW_SECONDS,
    SYNC_EVENT_BUDGET, SYNC_PATH,
};
use super::registry::MeshRegistry;
use super::repository::MeshRepositoryIdentity;
use super::state::{admission_conflicts, fold, ClaimView, CooperationState, HandoverView};
use super::trust::{self, TrustState};

/// The most peers one runtime links to.
pub const MAX_PEERS: usize = 256;

/// How long an expired peer stays listed before it leaves the table.
pub const PEER_RETENTION: Duration = Duration::from_secs(15 * 60);

/// The most hello nonces remembered for replay refusal.
const NONCE_CACHE: usize = 4096;

/// The stop check's slice while sleeping.
const SLICE: Duration = Duration::from_millis(100);

/// Everything cooperation needs to start.
pub struct CooperationSetup {
    /// This node's identity.
    pub identity: Arc<NodeIdentity>,
    /// This runtime's slot, 16 hex.
    pub runtime: String,
    /// The repository's mesh identity.
    pub repository: MeshRepositoryIdentity,
    /// Where this runtime answers.
    pub endpoints: Vec<String>,
    /// The executable version.
    pub version: String,
    /// The declaration's cooperation section.
    pub config: CooperationConfig,
    /// The declaration's trust section.
    pub trust: TrustConfig,
    /// Where the journal persists, when anywhere.
    pub journal_path: Option<PathBuf>,
    /// The discovery registry candidates come from.
    pub registry: Arc<MeshRegistry>,
    /// How link messages travel.
    pub transport: Arc<dyn LinkTransport>,
    /// The peer board of this process, projected into the journal every heartbeat: each
    /// attached session becomes a mesh session and each of its announcements an advisory
    /// claim, so a board announcement is visible on every linked runtime.
    pub board: Option<Arc<crate::peers::PeerBoard>>,
    /// Facts about this checkout the projected sessions carry.
    pub checkout: CheckoutFacts,
}

/// What the projected sessions say about the checkout they work in. Computed once at
/// start — branch and head are refreshed when a board session changes, never on a
/// heartbeat, so liveness never rescans the repository.
#[derive(Debug, Clone, Default)]
pub struct CheckoutFacts {
    /// The checkout id (a digest).
    pub id: Option<String>,
    /// The checkout root, for reading the branch and the head when a session changes.
    pub root: Option<PathBuf>,
}

impl CheckoutFacts {
    fn git(&self, args: &[&str]) -> Option<String> {
        let root = self.root.as_ref()?;
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[derive(Default)]
struct Projected {
    info: SessionInfo,
    claims: BTreeMap<String, (String, Vec<String>, String)>,
}

/// A link's standing, from this runtime's side.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// Handshaking, or never yet exchanged.
    Connecting,
    /// An exchange within two heartbeats.
    Connected,
    /// Rounds are failing; the last exchange is under half the expiry old.
    Degraded,
    /// The last exchange is over half the expiry old; the link is about to expire.
    Unreachable,
    /// No exchange within the expiry: the link is dropped and will be re-dialed.
    Expired,
}

#[derive(Clone)]
struct Outbound {
    link: String,
    endpoint: String,
    counter: u64,
    remote_marks: Marks,
}

#[derive(Clone)]
struct Inbound {
    link: String,
    counter: u64,
}

struct PeerLink {
    card: RuntimeCard,
    trust: TrustState,
    proto: u32,
    outbound: Option<Outbound>,
    inbound: Option<Inbound>,
    last_exchange: Option<Instant>,
    linked_at: String,
    rtt: Option<Duration>,
    failures: u32,
    last_error: Option<String>,
    handshakes: u64,
    reconnects: u64,
    restarts: u64,
    expired: bool,
}

struct Refused {
    refusal: LinkRefusal,
    runtime: Option<String>,
    direction: &'static str,
    at: String,
}

#[derive(Default)]
struct Table {
    peers: BTreeMap<String, PeerLink>,
    inbound_links: BTreeMap<String, String>,
    nonces: BTreeMap<(String, String), u64>,
    refused: BTreeMap<String, Refused>,
    workers: BTreeMap<String, Arc<AtomicBool>>,
}

#[derive(Default)]
struct Counters {
    handshakes_out: AtomicU64,
    handshakes_in: AtomicU64,
    dial_failures: AtomicU64,
    syncs_out: AtomicU64,
    syncs_failed: AtomicU64,
    syncs_in: AtomicU64,
    events_sent: AtomicU64,
    events_served: AtomicU64,
    reconnects: AtomicU64,
    restarts: AtomicU64,
    peers_expired: AtomicU64,
    claims_refused: AtomicU64,
    threads: AtomicU64,
    refused_in: Mutex<BTreeMap<RefusalCode, u64>>,
    refused_out: Mutex<BTreeMap<RefusalCode, u64>>,
}

/// One peer as every surface shows it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerView {
    /// `<node>-<runtime>`: the peer's durable identity.
    pub runtime: String,
    /// The node (machine key digest).
    pub node: String,
    /// The current instance.
    pub instance: String,
    /// The node's display name.
    pub name: String,
    /// The executable version.
    pub version: String,
    /// Whether the peer runs on this machine (the same node key).
    pub local: bool,
    /// The trust verdict the link was admitted under.
    pub trust: TrustState,
    /// The negotiated link protocol.
    pub protocol: u32,
    /// The link features the peer carries.
    pub features: Vec<String>,
    /// Where the peer answers.
    pub endpoints: Vec<String>,
    /// The link's standing.
    pub state: LinkState,
    /// Whether this runtime dials the peer.
    pub outbound: bool,
    /// Whether the peer dials this runtime.
    pub inbound: bool,
    /// The endpoint this runtime dials, when it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Milliseconds since the last successful exchange in either direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_exchange_ms: Option<u64>,
    /// The last round trip, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u64>,
    /// Consecutive failed rounds.
    pub failures: u32,
    /// The last failure, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Handshakes completed with this peer.
    pub handshakes: u64,
    /// Handshakes after an expiry or a dropped link.
    pub reconnects: u64,
    /// New instances of the peer seen (its process restarted).
    pub restarts: u64,
    /// When the current link was established, RFC 3339.
    pub linked_at: String,
}

/// A candidate that was refused, with the rule that refused it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefusedView {
    /// The endpoint dialed, or `inbound:<runtime>` for a refused hello.
    pub endpoint: String,
    /// `outbound` (this runtime refused the peer's welcome, or the peer refused this
    /// hello) or `inbound` (this runtime refused the peer's hello).
    pub direction: String,
    /// The peer's runtime key, when its card said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The refusal.
    pub refusal: LinkRefusal,
    /// When, RFC 3339.
    pub at: String,
}

/// The link and replication counters since start.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CooperationCounters {
    /// Handshakes this runtime completed as the dialer.
    pub handshakes_out: u64,
    /// Handshakes this runtime accepted.
    pub handshakes_in: u64,
    /// Dials that reached no endpoint.
    pub dial_failures: u64,
    /// Sync rounds this runtime completed as the dialer.
    pub syncs_out: u64,
    /// Sync rounds that failed in transport.
    pub syncs_failed: u64,
    /// Sync rounds this runtime answered.
    pub syncs_in: u64,
    /// Events pushed in sync requests.
    pub events_sent: u64,
    /// Events served in sync answers.
    pub events_served: u64,
    /// Re-handshakes after an expiry or a dropped link.
    pub reconnects: u64,
    /// Peer restarts observed.
    pub restarts: u64,
    /// Links expired.
    pub peers_expired: u64,
    /// Claims refused for a conflict.
    pub claims_refused: u64,
    /// Cooperation threads alive now.
    pub threads: u64,
    /// Hellos and syncs this runtime refused, by code.
    pub refused_in: BTreeMap<RefusalCode, u64>,
    /// Refusals this runtime received or applied to a welcome, by code.
    pub refused_out: BTreeMap<RefusalCode, u64>,
}

/// The whole cooperation runtime, as every surface reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CooperationStatus {
    /// Whether cooperation runs in this process.
    pub active: bool,
    /// Why not, when it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// This runtime, `<node>-<runtime>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// This runtime's current stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamId>,
    /// The repository identity links are matched on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<MeshRepositoryIdentity>,
    /// The link protocol range spoken.
    pub protocol: (u32, u32),
    /// The link features carried.
    pub features: Vec<String>,
    /// Seconds between heartbeats.
    pub heartbeat_seconds: u64,
    /// Seconds of silence before expiry.
    pub expiry_seconds: u64,
    /// Where this runtime answers.
    pub endpoints: Vec<String>,
    /// The declared seeds.
    pub seeds: Vec<String>,
    /// Every linked peer, by runtime key.
    pub peers: Vec<PeerView>,
    /// Candidates refused, by endpoint.
    pub refused: Vec<RefusedView>,
    /// Counters.
    pub counters: CooperationCounters,
    /// The journal's tallies.
    pub journal: JournalTallies,
    /// When cooperation started, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

impl CooperationStatus {
    /// The status of a runtime where cooperation is not running, and why.
    pub fn inactive(reason: &str) -> Self {
        CooperationStatus {
            active: false,
            reason: Some(reason.into()),
            runtime: None,
            stream: None,
            repository: None,
            protocol: (LINK_PROTOCOL_MIN, LINK_PROTOCOL_MAX),
            features: FEATURES.iter().map(|f| f.to_string()).collect(),
            heartbeat_seconds: 0,
            expiry_seconds: 0,
            endpoints: Vec::new(),
            seeds: Vec::new(),
            peers: Vec::new(),
            refused: Vec::new(),
            counters: CooperationCounters::default(),
            journal: JournalTallies::default(),
            started_at: None,
        }
    }
}

/// Why a cooperation operation did not happen.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CooperationError {
    /// An exclusive claim meets live exclusive claims of other sessions.
    #[error("the scope is claimed: {}", .0.iter().map(|c| format!("{} ({})", c.key, c.scope.join(", "))).collect::<Vec<_>>().join("; "))]
    Conflict(Vec<ClaimView>),
    /// The operation needs something this runtime wrote, and this was written elsewhere.
    #[error("{0}")]
    NotOwn(String),
    /// Nothing by that name.
    #[error("{0}")]
    NotFound(String),
    /// The peer lacks a feature the operation needs.
    #[error("{0}")]
    FeatureUnsupported(String),
    /// Out of bounds.
    #[error("{0}")]
    Invalid(String),
}

/// One verification check: what was checked, whether it holds, the evidence, and — when
/// something is wrong or limited — the impact and what to do about it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshVerifyCheck {
    /// What was checked.
    pub check: String,
    /// Whether it holds.
    pub ok: bool,
    /// The evidence.
    pub detail: String,
    /// What the failure or limitation means for cooperation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,
    /// What to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

/// One peer's verification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerVerdict {
    /// The peer's runtime key.
    pub runtime: String,
    /// The link's standing after the round.
    pub state: LinkState,
    /// Whether this runtime dials the peer (and so ran a round with it now).
    pub dialed: bool,
    /// Whether the round succeeded; `None` when the peer dials this runtime instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_trip: Option<bool>,
    /// The round trip, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u64>,
    /// Whether both sides hold the same high-water mark of every stream after the round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converged: Option<bool>,
    /// What happened.
    pub detail: String,
}

/// The answer of `mesh.verify`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshVerifyReport {
    /// Whether every check holds and every round succeeded.
    pub ok: bool,
    /// This runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The checks, in the order they ran.
    pub checks: Vec<MeshVerifyCheck>,
    /// Every linked peer's verdict.
    pub peers: Vec<PeerVerdict>,
    /// The state digest after the rounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl MeshVerifyReport {
    /// The report of a runtime where cooperation does not run.
    pub fn inactive(reason: &str) -> Self {
        MeshVerifyReport {
            ok: false,
            runtime: None,
            checks: vec![MeshVerifyCheck {
                check: "cooperation".into(),
                ok: false,
                detail: reason.into(),
                impact: Some("no session, claim, handover or review leaves or reaches this runtime".into()),
                remediation: Some("enable the mesh declaration (enabled: true), trust the peers' keys (trust.allow, from `majordomus mesh identity` on each machine), and restart the server: majordomus serve stop && majordomus serve ensure".into()),
            }],
            peers: Vec::new(),
            digest: None,
        }
    }
}

/// A written event, as an operation answers it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Written {
    /// `<stream>/<local id>` of what was created or changed.
    pub key: String,
    /// `<stream>/<seq>` of the event that says so.
    pub event: String,
    /// The event's Lamport stamp.
    pub lamport: u64,
}

/// What a dial or a sync round did not do.
#[derive(Debug, Clone)]
pub enum RoundError {
    /// No endpoint answered.
    Unreachable(String),
    /// The peer, or this runtime's check of the peer, refused.
    Refused(LinkRefusal),
    /// There is no outbound link to sync.
    NoLink,
}

impl std::fmt::Display for RoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoundError::Unreachable(e) => write!(f, "unreachable: {e}"),
            RoundError::Refused(r) => write!(f, "refused: {r}"),
            RoundError::NoLink => f.write_str("no link"),
        }
    }
}

struct Target {
    id: String,
    key: Option<String>,
    endpoints: Vec<String>,
}

/// The cooperation runtime.
pub struct Cooperation {
    identity: Arc<NodeIdentity>,
    card: RuntimeCard,
    own_key: String,
    repository: MeshRepositoryIdentity,
    config: CooperationConfig,
    trust: TrustConfig,
    registry: Arc<MeshRegistry>,
    transport: Arc<dyn LinkTransport>,
    journal: Arc<Journal>,
    table: Mutex<Table>,
    counters: Counters,
    stop: Arc<AtomicBool>,
    started_at: String,
    board: Option<Arc<crate::peers::PeerBoard>>,
    checkout: CheckoutFacts,
    projected: Mutex<BTreeMap<String, Projected>>,
    admission: Mutex<()>,
    rounds: Mutex<BTreeMap<String, Arc<Mutex<()>>>>,
}

/// The most refusals the table lists: a stream of hellos from invented keys must not grow
/// memory without bound. The oldest refusal goes first.
const MAX_REFUSED: usize = 128;

/// The most runtimes of one node this runtime dials.
pub const MAX_TARGETS_PER_NODE: usize = 8;

/// `text` cut to at most `max` bytes on a character boundary, with control characters
/// removed: what the journal's single-line bounds accept, so a projection never fails
/// validation on a long or multi-byte title and silently retries forever.
fn clip(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars().filter(|c| !c.is_control()) {
        if out.len() + c.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    out
}

fn remember_refusal(table: &mut Table, key: String, refused: Refused) {
    if !table.refused.contains_key(&key) && table.refused.len() >= MAX_REFUSED {
        if let Some(oldest) = table
            .refused
            .iter()
            .min_by(|a, b| a.1.at.cmp(&b.1.at))
            .map(|(k, _)| k.clone())
        {
            table.refused.remove(&oldest);
        }
    }
    table.refused.insert(key, refused);
}

fn rfc3339_now() -> String {
    crate::peers::rfc3339(std::time::SystemTime::now())
}

impl Cooperation {
    /// Open the journal and build the runtime; nothing dials until [`Cooperation::start`].
    pub fn new(setup: CooperationSetup) -> Result<Arc<Self>, super::MeshError> {
        setup.config.validate().map_err(super::MeshError::Config)?;
        let journal = Arc::new(Journal::open(
            Arc::clone(&setup.identity),
            &setup.runtime,
            setup.repository.id.clone(),
            setup.journal_path,
        )?);
        let card = RuntimeCard {
            pk: setup.identity.public.public_key.clone(),
            runtime: setup.runtime.clone(),
            instance: setup.identity.public.instance_id.as_str().into(),
            repo: setup.repository.id.clone(),
            name: setup.identity.public.display_name.clone(),
            version: setup.version.chars().take(32).collect(),
            features: FEATURES.iter().map(|f| f.to_string()).collect(),
            endpoints: setup
                .endpoints
                .iter()
                .take(super::protocol::MAX_ENDPOINTS)
                .cloned()
                .collect(),
        };
        card.check().map_err(super::MeshError::Protocol)?;
        let own_key = journal.own_stream().runtime_key();
        Ok(Arc::new(Cooperation {
            identity: setup.identity,
            card,
            own_key,
            repository: setup.repository,
            config: setup.config,
            trust: setup.trust,
            registry: setup.registry,
            transport: setup.transport,
            journal,
            table: Mutex::new(Table::default()),
            counters: Counters::default(),
            stop: Arc::new(AtomicBool::new(false)),
            started_at: rfc3339_now(),
            board: setup.board,
            checkout: setup.checkout,
            projected: Mutex::new(BTreeMap::new()),
            admission: Mutex::new(()),
            rounds: Mutex::new(BTreeMap::new()),
        }))
    }

    /// Project the peer board into the journal: an attached session that is new opens a
    /// mesh session, a changed announcement releases its previous advisory claim and
    /// takes the new one, a withdrawn announcement is released, and a session that left
    /// the board is closed. One-way and idempotent — the board stays the local truth, the
    /// journal carries it to every linked runtime. Nothing happens when nothing changed.
    pub fn project_board(&self) {
        let Some(board) = &self.board else { return };
        let peers = board.list();
        let mut projected = self.projected.lock().expect("cooperation projection");
        let mut seen = Vec::new();
        for peer in peers.iter().filter(|p| p.attached) {
            let session = format!("board-{}", peer.id);
            seen.push(session.clone());
            let intent = peer.claims.first().map(|c| c.intent.clone());
            let entry = projected.entry(session.clone()).or_default();
            let fresh = entry.info.session.is_empty();
            if fresh || entry.info.intent != intent {
                let info = SessionInfo {
                    session: session.clone(),
                    client: clip(&peer.client.name, 64),
                    worker: peer
                        .client
                        .title
                        .clone()
                        .filter(|t| !t.is_empty())
                        .map(|t| clip(&t, 128)),
                    intent: intent.clone().map(|i| clip(&i, 512)),
                    checkout: self.checkout.id.clone(),
                    branch: self.checkout.git(&["symbolic-ref", "--short", "HEAD"]),
                    head: self.checkout.git(&["rev-parse", "--verify", "HEAD"]),
                    ..SessionInfo::default()
                };
                if self.open_session(info.clone()).is_ok() {
                    entry.info = info;
                }
            }
            let mut wanted: BTreeMap<String, (Vec<String>, String)> = BTreeMap::new();
            for claim in &peer.claims {
                let scope: Vec<String> = claim
                    .scope
                    .iter()
                    .map(|p| p.trim().trim_start_matches("./").to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                if !scope.is_empty() {
                    wanted.insert(
                        claim.name.clone().unwrap_or_default(),
                        (scope, claim.intent.clone()),
                    );
                }
            }
            let stale: Vec<String> = entry
                .claims
                .iter()
                .filter(|(name, (_, scope, intent))| {
                    wanted
                        .get(*name)
                        .is_none_or(|(s, i)| s != scope || i != intent)
                })
                .map(|(name, _)| name.clone())
                .collect();
            for name in stale {
                if let Some((key, _, _)) = entry.claims.remove(&name) {
                    let _ = self.release(&key);
                }
            }
            for (name, (scope, claim_intent)) in wanted {
                if entry.claims.contains_key(&name) {
                    continue;
                }
                match self.claim(
                    &entry.info,
                    scope.clone(),
                    Some(clip(&claim_intent, 512)).filter(|i| !i.is_empty()),
                    ClaimMode::Advisory,
                    None,
                ) {
                    Ok(written) => {
                        entry
                            .claims
                            .insert(name, (written.key, scope, claim_intent));
                    }
                    Err(e) => {
                        tracing::debug!(runtime_id = %self.own_key, session_id = %session, error = %e, "a board announcement was not projected")
                    }
                }
            }
        }
        let gone: Vec<String> = projected
            .keys()
            .filter(|s| !seen.contains(s))
            .cloned()
            .collect();
        for session in gone {
            if let Some(entry) = projected.remove(&session) {
                for (key, _, _) in entry.claims.values() {
                    let _ = self.release(key);
                }
            }
            let _ = self.close_session(&session);
        }
    }

    /// This runtime's key, `<node>-<runtime>`.
    pub fn runtime_key(&self) -> &str {
        &self.own_key
    }

    /// This runtime's journal.
    pub fn journal(&self) -> &Arc<Journal> {
        &self.journal
    }

    fn heartbeat(&self) -> Duration {
        Duration::from_secs(self.config.heartbeat_seconds)
    }

    fn expiry(&self) -> Duration {
        Duration::from_secs(self.config.expiry_seconds)
    }

    /// Start the supervisor: the heartbeat, the dialers and the expiry. Never blocks.
    pub fn start(self: &Arc<Self>) {
        if !self.config.enabled {
            return;
        }
        let me = Arc::clone(self);
        me.counters.threads.fetch_add(1, Ordering::SeqCst);
        let _ = std::thread::Builder::new()
            .name("majordomus-mesh-cooperation".into())
            .spawn(move || {
                tracing::info!(runtime_id = %me.own_key, repository_id = %me.repository.id, "mesh cooperation started");
                let mut tick: u64 = 0;
                while !me.stop.load(Ordering::SeqCst) {
                    me.journal.beat_own();
                    me.project_board();
                    me.reconcile_workers();
                    me.expire_links();
                    if tick % 60 == 59 {
                        me.journal.compact(me.expiry(), PEER_RETENTION, false);
                    }
                    tick += 1;
                    me.sleep(me.heartbeat());
                }
                me.counters.threads.fetch_sub(1, Ordering::SeqCst);
            });
    }

    /// Stop every cooperation thread at its next bounded wait and drop every link. The
    /// journal stays readable; this runtime's stream simply stops beating.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let mut table = self.table.lock().expect("cooperation table");
        for flag in table.workers.values() {
            flag.store(true, Ordering::SeqCst);
        }
        table.workers.clear();
        table.inbound_links.clear();
        for peer in table.peers.values_mut() {
            peer.outbound = None;
            peer.inbound = None;
        }
    }

    /// Whether [`Cooperation::stop`] was called.
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn sleep(&self, total: Duration) {
        let mut slept = Duration::ZERO;
        while slept < total && !self.stop.load(Ordering::SeqCst) {
            let slice = SLICE.min(total - slept);
            std::thread::sleep(slice);
            slept += slice;
        }
    }

    // ------------------------------------------------------------------ trust

    /// The trust verdict for a key: this machine's own key is trusted as itself; any
    /// other by the declared policy against what discovery knows of the node.
    pub fn trust_of_key(&self, public_key: &str) -> TrustState {
        if public_key == self.identity.public.public_key {
            return TrustState::Trusted("this machine's own key".into());
        }
        let Some(node) = node_id_of_key(public_key) else {
            return TrustState::Rejected("not a key".into());
        };
        trust::evaluate(
            &self.trust.policy,
            &self.trust.allow,
            public_key,
            self.registry.known_key(&node).as_deref(),
            self.registry.trust_of(&node).as_ref(),
        )
    }

    fn origin_accept(&self, event: &MeshEvent) -> Result<(), Rejection> {
        if self.trust_of_key(&event.pk).is_trusted() {
            Ok(())
        } else {
            Err(Rejection::Untrusted)
        }
    }

    fn count_refusal(map: &Mutex<BTreeMap<RefusalCode, u64>>, code: RefusalCode) {
        *map.lock()
            .expect("refusal counters")
            .entry(code)
            .or_default() += 1;
    }

    fn refuse_in(&self, code: RefusalCode, detail: String, runtime: Option<String>) -> LinkReply {
        Self::count_refusal(&self.counters.refused_in, code);
        tracing::warn!(runtime_id = %self.own_key, peer = runtime.as_deref().unwrap_or("?"), code = code.as_str(), detail = %detail, "mesh link refused");
        let reply = LinkReply::refused(code, detail);
        if let (Some(refusal), Some(runtime)) = (reply.refusal.clone(), runtime) {
            let mut table = self.table.lock().expect("cooperation table");
            remember_refusal(
                &mut table,
                format!("inbound:{runtime}"),
                Refused {
                    refusal,
                    runtime: Some(runtime),
                    direction: "inbound",
                    at: rfc3339_now(),
                },
            );
        }
        reply
    }

    // ------------------------------------------------------------------ answering

    /// Answer a hello: the whole admission, in order — shape, signature, freshness,
    /// replay, protocol, self, repository, trust, capacity — and a signed welcome.
    pub fn accept_hello(&self, message: &Signed) -> LinkReply {
        if self.stopped() {
            return LinkReply::refused(RefusalCode::NotActive, "cooperation is stopped");
        }
        if serde_json::to_vec(message)
            .map(|b| b.len())
            .unwrap_or(usize::MAX)
            > MAX_LINK_MESSAGE
        {
            return self.refuse_in(
                RefusalCode::Oversized,
                "the hello exceeds the message bound".into(),
                None,
            );
        }
        let hello: Hello = match serde_json::from_value(message.body.clone()) {
            Ok(h) => h,
            Err(e) => {
                return self.refuse_in(RefusalCode::Malformed, format!("not a hello: {e}"), None)
            }
        };
        if let Err(e) = hello.card.check() {
            return self.refuse_in(RefusalCode::Malformed, e, None);
        }
        let runtime = hello.card.runtime_key();
        if !verify_signed(&hello.card.pk, Domain::Hello, message) {
            return self.refuse_in(
                RefusalCode::Signature,
                "the hello's signature does not verify".into(),
                runtime,
            );
        }
        if hello.ts.abs_diff(super::protocol::now()) > MAX_LINK_SKEW_SECONDS {
            return self.refuse_in(
                RefusalCode::Stale,
                "the hello's clock is outside the skew window".into(),
                runtime,
            );
        }
        if hello.nonce.len() != 32 {
            return self.refuse_in(
                RefusalCode::Malformed,
                "the nonce is not 32 hex characters".into(),
                runtime,
            );
        }
        let Some(proto) = negotiate(hello.proto_min, hello.proto_max) else {
            return self.refuse_in(
                RefusalCode::ProtocolUnsupported,
                format!(
                    "this runtime speaks link protocol {LINK_PROTOCOL_MIN}..{LINK_PROTOCOL_MAX}; the peer speaks {}..{}",
                    hello.proto_min, hello.proto_max
                ),
                runtime,
            );
        };
        let runtime_key = runtime.clone().unwrap_or_default();
        if runtime_key == self.own_key {
            return self.refuse_in(
                RefusalCode::SelfLink,
                "that is this runtime".into(),
                runtime,
            );
        }
        if hello.card.repo != self.repository.id {
            return self.refuse_in(
                RefusalCode::RepositoryMismatch,
                format!(
                    "this runtime serves repository {}; the peer serves {}",
                    self.repository.id, hello.card.repo
                ),
                runtime,
            );
        }
        let verdict = self.trust_of_key(&hello.card.pk);
        if !verdict.is_trusted() {
            return self.refuse_in(
                RefusalCode::Untrusted,
                format!("the peer's key is not trusted here ({verdict:?}): discovery is not trust"),
                runtime,
            );
        }
        // The replay cache holds only nonces of trusted keys of this repository: a stranger's
        // hellos, however many, can neither fill it nor evict a real peer's nonce.
        {
            let mut table = self.table.lock().expect("cooperation table");
            let now = super::protocol::now();
            table
                .nonces
                .retain(|_, ts| now.saturating_sub(*ts) <= 2 * MAX_LINK_SKEW_SECONDS);
            let key = (hello.card.pk.clone(), hello.nonce.clone());
            if table.nonces.contains_key(&key) {
                drop(table);
                return self.refuse_in(
                    RefusalCode::Replay,
                    "this hello was already answered".into(),
                    runtime,
                );
            }
            if table.nonces.len() >= NONCE_CACHE {
                if let Some(oldest) = table
                    .nonces
                    .iter()
                    .min_by_key(|(_, ts)| **ts)
                    .map(|(k, _)| k.clone())
                {
                    table.nonces.remove(&oldest);
                }
            }
            table.nonces.insert(key, now);
        }
        let link = fresh_token();
        {
            let mut table = self.table.lock().expect("cooperation table");
            if !table.peers.contains_key(&runtime_key) && table.peers.len() >= MAX_PEERS {
                drop(table);
                return self.refuse_in(
                    RefusalCode::Capacity,
                    "the peer table is full".into(),
                    runtime,
                );
            }
            let restarts = &self.counters.restarts;
            let reconnects = &self.counters.reconnects;
            let peer = table
                .peers
                .entry(runtime_key.clone())
                .or_insert_with(|| PeerLink {
                    card: hello.card.clone(),
                    trust: verdict.clone(),
                    proto,
                    outbound: None,
                    inbound: None,
                    last_exchange: None,
                    linked_at: rfc3339_now(),
                    rtt: None,
                    failures: 0,
                    last_error: None,
                    handshakes: 0,
                    reconnects: 0,
                    restarts: 0,
                    expired: false,
                });
            if peer.card.instance != hello.card.instance {
                // A new run of the same runtime: its old links are dead on both sides.
                peer.restarts += 1;
                restarts.fetch_add(1, Ordering::Relaxed);
                peer.outbound = None;
                peer.linked_at = rfc3339_now();
            } else if peer.expired || (peer.handshakes > 0 && peer.inbound.is_some()) {
                peer.reconnects += 1;
                reconnects.fetch_add(1, Ordering::Relaxed);
            }
            if let Some(old) = peer.inbound.take() {
                table.inbound_links.remove(&old.link);
            }
            let peer = table.peers.get_mut(&runtime_key).expect("inserted above");
            peer.card = hello.card.clone();
            peer.trust = verdict;
            peer.proto = proto;
            peer.inbound = Some(Inbound {
                link: link.clone(),
                counter: 0,
            });
            peer.last_exchange = Some(Instant::now());
            peer.handshakes += 1;
            peer.expired = false;
            table
                .inbound_links
                .insert(link.clone(), runtime_key.clone());
            table.refused.remove(&format!("inbound:{runtime_key}"));
        }
        self.counters.handshakes_in.fetch_add(1, Ordering::Relaxed);
        tracing::info!(runtime_id = %self.own_key, peer = %runtime_key, link_id = %link, "mesh link accepted");
        let welcome = Welcome {
            proto,
            link,
            card: self.card.clone(),
            nonce: hello.nonce,
            ts: super::protocol::now(),
            marks: self.journal.marks(),
            heartbeat_seconds: self.config.heartbeat_seconds,
            expiry_seconds: self.config.expiry_seconds,
        };
        match serde_json::to_value(&welcome) {
            Ok(body) => LinkReply::accepted(sign(&self.identity, Domain::Welcome, body)),
            Err(e) => LinkReply::refused(RefusalCode::Malformed, e.to_string()),
        }
    }

    /// Answer a sync round: the link must be known and its peer's instance current, the
    /// signature must be the peer's and the counter must rise; then ingest, merge, and
    /// answer with what the peer lacks.
    pub fn accept_sync(&self, message: &Signed) -> LinkReply {
        if self.stopped() {
            return LinkReply::refused(RefusalCode::NotActive, "cooperation is stopped");
        }
        if serde_json::to_vec(message)
            .map(|b| b.len())
            .unwrap_or(usize::MAX)
            > MAX_LINK_MESSAGE
        {
            return self.refuse_in(
                RefusalCode::Oversized,
                "the sync exceeds the message bound".into(),
                None,
            );
        }
        let request: SyncRequest = match serde_json::from_value(message.body.clone()) {
            Ok(r) => r,
            Err(e) => {
                return self.refuse_in(
                    RefusalCode::Malformed,
                    format!("not a sync request: {e}"),
                    None,
                )
            }
        };
        let runtime_key = {
            let mut table = self.table.lock().expect("cooperation table");
            let Some(key) = table.inbound_links.get(&request.link).cloned() else {
                drop(table);
                Self::count_refusal(&self.counters.refused_in, RefusalCode::UnknownLink);
                return LinkReply::refused(
                    RefusalCode::UnknownLink,
                    "no such link here: say hello",
                );
            };
            let peer = table
                .peers
                .get_mut(&key)
                .expect("an indexed link has a peer");
            if peer.card.instance != request.instance {
                drop(table);
                Self::count_refusal(&self.counters.refused_in, RefusalCode::UnknownLink);
                return LinkReply::refused(
                    RefusalCode::UnknownLink,
                    "the link belongs to a previous instance: say hello",
                );
            }
            if !verify_signed(&peer.card.pk, Domain::SyncRequest, message) {
                drop(table);
                return self.refuse_in(
                    RefusalCode::Signature,
                    "the sync's signature is not the linked peer's".into(),
                    Some(key),
                );
            }
            let inbound = peer.inbound.as_mut().expect("an indexed link is inbound");
            if request.counter <= inbound.counter {
                drop(table);
                return self.refuse_in(
                    RefusalCode::Replay,
                    "the sync counter did not rise".into(),
                    Some(key),
                );
            }
            inbound.counter = request.counter;
            peer.last_exchange = Some(Instant::now());
            peer.expired = false;
            key
        };
        let report = self
            .journal
            .ingest(&request.events, &|e| self.origin_accept(e));
        self.journal.merge_marks(
            &request.marks,
            &|pk| self.trust_of_key(pk).is_trusted(),
            self.expiry(),
        );
        let events = self.journal.missing_for(&request.marks, SYNC_EVENT_BUDGET);
        self.counters.syncs_in.fetch_add(1, Ordering::Relaxed);
        self.counters
            .events_served
            .fetch_add(events.len() as u64, Ordering::Relaxed);
        if report.accepted > 0 || report.rejected_total() > 0 {
            tracing::debug!(runtime_id = %self.own_key, peer = %runtime_key, accepted = report.accepted, duplicate = report.duplicate, rejected = report.rejected_total(), "mesh sync ingested");
        }
        let answer = SyncAnswer {
            link: request.link,
            counter: request.counter,
            marks: self.journal.marks(),
            events,
            report,
        };
        match serde_json::to_value(&answer) {
            Ok(body) => LinkReply::accepted(sign(&self.identity, Domain::SyncAnswer, body)),
            Err(e) => LinkReply::refused(RefusalCode::Malformed, e.to_string()),
        }
    }

    // ------------------------------------------------------------------ dialing

    fn refuse_out(
        &self,
        endpoint: &str,
        refusal: LinkRefusal,
        runtime: Option<String>,
    ) -> RoundError {
        Self::count_refusal(&self.counters.refused_out, refusal.code);
        tracing::warn!(runtime_id = %self.own_key, endpoint = %endpoint, code = refusal.code.as_str(), detail = %refusal.detail, "mesh link not established");
        remember_refusal(
            &mut self.table.lock().expect("cooperation table"),
            endpoint.to_string(),
            Refused {
                refusal: refusal.clone(),
                runtime,
                direction: "outbound",
                at: rfc3339_now(),
            },
        );
        RoundError::Refused(refusal)
    }

    /// Say hello at each endpoint in turn until one welcomes this runtime; verify the
    /// welcome as strictly as a hello is verified. Returns the peer's runtime key.
    pub fn dial(&self, endpoints: &[String]) -> Result<String, RoundError> {
        let hello = Hello {
            proto_min: LINK_PROTOCOL_MIN,
            proto_max: LINK_PROTOCOL_MAX,
            card: self.card.clone(),
            nonce: fresh_token(),
            ts: super::protocol::now(),
        };
        let body =
            serde_json::to_value(&hello).map_err(|e| RoundError::Unreachable(e.to_string()))?;
        let message = sign(&self.identity, Domain::Hello, body);
        let mut last = "no endpoint".to_string();
        for endpoint in endpoints {
            let started = Instant::now();
            let reply = match self.transport.post(endpoint, HELLO_PATH, &message) {
                Ok(reply) => reply,
                Err(e) => {
                    last = e;
                    continue;
                }
            };
            if let Some(refusal) = reply.refusal {
                return Err(self.refuse_out(endpoint, refusal, None));
            }
            let Some(signed) = reply.signed else {
                return Err(self.refuse_out(
                    endpoint,
                    LinkRefusal::new(RefusalCode::Malformed, "an empty reply"),
                    None,
                ));
            };
            let welcome: Welcome = match serde_json::from_value(signed.body.clone()) {
                Ok(w) => w,
                Err(e) => {
                    return Err(self.refuse_out(
                        endpoint,
                        LinkRefusal::new(RefusalCode::Malformed, format!("not a welcome: {e}")),
                        None,
                    ))
                }
            };
            if let Err(e) = welcome.card.check() {
                return Err(self.refuse_out(
                    endpoint,
                    LinkRefusal::new(RefusalCode::Malformed, e),
                    None,
                ));
            }
            let runtime = welcome.card.runtime_key();
            let check = if !verify_signed(&welcome.card.pk, Domain::Welcome, &signed) {
                Some((
                    RefusalCode::Signature,
                    "the welcome's signature does not verify".to_string(),
                ))
            } else if welcome.nonce != hello.nonce {
                Some((
                    RefusalCode::Replay,
                    "the welcome answers another hello".into(),
                ))
            } else if welcome.ts.abs_diff(super::protocol::now()) > MAX_LINK_SKEW_SECONDS {
                Some((
                    RefusalCode::Stale,
                    "the welcome's clock is outside the skew window".into(),
                ))
            } else if negotiate(welcome.proto, welcome.proto).is_none() {
                Some((
                    RefusalCode::ProtocolUnsupported,
                    format!("the peer chose protocol {}", welcome.proto),
                ))
            } else if runtime.as_deref() == Some(self.own_key.as_str()) {
                Some((
                    RefusalCode::SelfLink,
                    "that endpoint is this runtime".into(),
                ))
            } else if welcome.card.repo != self.repository.id {
                Some((
                    RefusalCode::RepositoryMismatch,
                    format!("the peer serves repository {}", welcome.card.repo),
                ))
            } else if !self.trust_of_key(&welcome.card.pk).is_trusted() {
                Some((
                    RefusalCode::Untrusted,
                    "the peer's key is not trusted here: discovery is not trust".into(),
                ))
            } else {
                None
            };
            if let Some((code, detail)) = check {
                return Err(self.refuse_out(endpoint, LinkRefusal::new(code, detail), runtime));
            }
            let key = runtime.expect("a checked card has a runtime key");
            let verdict = self.trust_of_key(&welcome.card.pk);
            let mut table = self.table.lock().expect("cooperation table");
            if !table.peers.contains_key(&key) && table.peers.len() >= MAX_PEERS {
                drop(table);
                return Err(self.refuse_out(
                    endpoint,
                    LinkRefusal::new(RefusalCode::Capacity, "the peer table is full"),
                    Some(key),
                ));
            }
            let peer = table.peers.entry(key.clone()).or_insert_with(|| PeerLink {
                card: welcome.card.clone(),
                trust: verdict.clone(),
                proto: welcome.proto,
                outbound: None,
                inbound: None,
                last_exchange: None,
                linked_at: rfc3339_now(),
                rtt: None,
                failures: 0,
                last_error: None,
                handshakes: 0,
                reconnects: 0,
                restarts: 0,
                expired: false,
            });
            if peer.card.instance != welcome.card.instance {
                peer.restarts += 1;
                self.counters.restarts.fetch_add(1, Ordering::Relaxed);
                peer.linked_at = rfc3339_now();
            } else if peer.handshakes > 0 {
                peer.reconnects += 1;
                self.counters.reconnects.fetch_add(1, Ordering::Relaxed);
            }
            peer.card = welcome.card.clone();
            peer.trust = verdict;
            peer.proto = welcome.proto;
            peer.outbound = Some(Outbound {
                link: welcome.link.clone(),
                endpoint: endpoint.clone(),
                counter: 0,
                remote_marks: welcome.marks.clone(),
            });
            peer.last_exchange = Some(Instant::now());
            peer.rtt = Some(started.elapsed());
            peer.failures = 0;
            peer.last_error = None;
            peer.handshakes += 1;
            peer.expired = false;
            table.refused.remove(endpoint);
            drop(table);
            self.journal.merge_marks(
                &welcome.marks,
                &|pk| self.trust_of_key(pk).is_trusted(),
                self.expiry(),
            );
            self.counters.handshakes_out.fetch_add(1, Ordering::Relaxed);
            tracing::info!(runtime_id = %self.own_key, peer = %key, endpoint = %endpoint, link_id = %welcome.link, "mesh link established");
            return Ok(key);
        }
        self.counters.dial_failures.fetch_add(1, Ordering::Relaxed);
        Err(RoundError::Unreachable(last))
    }

    /// One sync round with a peer this runtime dials.
    pub fn sync_with(&self, runtime_key: &str) -> Result<(), RoundError> {
        // One round per link at a time: a verification and a worker (or two workers for one
        // peer) racing on the counter would refuse each other's rounds as replays.
        let round = {
            let mut rounds = self.rounds.lock().expect("sync rounds");
            Arc::clone(rounds.entry(runtime_key.to_string()).or_default())
        };
        let _round = round.lock().expect("sync round");
        let (out, pk) = {
            let mut table = self.table.lock().expect("cooperation table");
            let Some(peer) = table.peers.get_mut(runtime_key) else {
                return Err(RoundError::NoLink);
            };
            let Some(out) = peer.outbound.as_mut() else {
                return Err(RoundError::NoLink);
            };
            out.counter += 1;
            (out.clone(), peer.card.pk.clone())
        };
        let request = SyncRequest {
            link: out.link.clone(),
            counter: out.counter,
            instance: self.card.instance.clone(),
            marks: self.journal.marks(),
            events: self
                .journal
                .missing_for(&out.remote_marks, SYNC_EVENT_BUDGET),
        };
        let sent = request.events.len() as u64;
        let body =
            serde_json::to_value(&request).map_err(|e| RoundError::Unreachable(e.to_string()))?;
        let message = sign(&self.identity, Domain::SyncRequest, body);
        let started = Instant::now();
        let failed = |why: String| {
            self.counters.syncs_failed.fetch_add(1, Ordering::Relaxed);
            let mut table = self.table.lock().expect("cooperation table");
            if let Some(peer) = table.peers.get_mut(runtime_key) {
                peer.failures += 1;
                peer.last_error = Some(why.clone());
            }
            why
        };
        let reply = match self.transport.post(&out.endpoint, SYNC_PATH, &message) {
            Ok(reply) => reply,
            Err(e) => return Err(RoundError::Unreachable(failed(e))),
        };
        if let Some(refusal) = reply.refusal {
            failed(refusal.to_string());
            if refusal.code == RefusalCode::UnknownLink {
                let mut table = self.table.lock().expect("cooperation table");
                if let Some(peer) = table.peers.get_mut(runtime_key) {
                    peer.outbound = None;
                }
            }
            Self::count_refusal(&self.counters.refused_out, refusal.code);
            return Err(RoundError::Refused(refusal));
        }
        let Some(signed) = reply.signed else {
            return Err(RoundError::Unreachable(failed(
                "an empty sync reply".into(),
            )));
        };
        let answer: SyncAnswer = match serde_json::from_value(signed.body.clone()) {
            Ok(a) => a,
            Err(e) => {
                return Err(RoundError::Unreachable(failed(format!(
                    "not a sync answer: {e}"
                ))))
            }
        };
        if !verify_signed(&pk, Domain::SyncAnswer, &signed)
            || answer.link != out.link
            || answer.counter != out.counter
        {
            return Err(RoundError::Refused(LinkRefusal::new(
                RefusalCode::Signature,
                failed("the sync answer is not the linked peer's answer to this round".into()),
            )));
        }
        self.journal
            .ingest(&answer.events, &|e| self.origin_accept(e));
        self.journal.merge_marks(
            &answer.marks,
            &|pk| self.trust_of_key(pk).is_trusted(),
            self.expiry(),
        );
        {
            let mut table = self.table.lock().expect("cooperation table");
            if let Some(peer) = table.peers.get_mut(runtime_key) {
                if let Some(o) = peer.outbound.as_mut() {
                    o.remote_marks = answer.marks;
                }
                peer.last_exchange = Some(Instant::now());
                peer.rtt = Some(started.elapsed());
                peer.failures = 0;
                peer.last_error = None;
                peer.expired = false;
            }
        }
        self.counters.syncs_out.fetch_add(1, Ordering::Relaxed);
        self.counters.events_sent.fetch_add(sent, Ordering::Relaxed);
        Ok(())
    }

    // ------------------------------------------------------------------ supervision

    fn targets(&self) -> Vec<Target> {
        let mut targets: Vec<Target> = self
            .config
            .seeds
            .iter()
            .map(|seed| Target {
                id: format!("seed:{seed}"),
                key: None,
                endpoints: vec![seed.trim_start_matches("http://").to_string()],
            })
            .collect();
        // At most MAX_TARGETS_PER_NODE runtimes of one node are dialed: runtime slots are free
        // to invent, and a key must not turn this runtime into a dialer of arbitrary endpoints.
        let mut per_node: BTreeMap<String, usize> = BTreeMap::new();
        for record in self.registry.list() {
            if record.runtime.is_empty()
                || record.endpoints.is_empty()
                || !record.repositories.contains(&self.repository.id)
            {
                continue;
            }
            let key = format!("{}-{}", record.node_id, record.runtime);
            if key == self.own_key || !self.trust_of_key(&record.public_key).is_trusted() {
                continue;
            }
            let dialed = per_node.entry(record.node_id.to_string()).or_default();
            if *dialed >= MAX_TARGETS_PER_NODE {
                continue;
            }
            *dialed += 1;
            targets.push(Target {
                id: key.clone(),
                key: Some(key),
                endpoints: record.endpoints.clone(),
            });
        }
        targets
    }

    fn reconcile_workers(self: &Arc<Self>) {
        let targets = self.targets();
        let mut table = self.table.lock().expect("cooperation table");
        let wanted: Vec<String> = targets.iter().map(|t| t.id.clone()).collect();
        table.workers.retain(|id, flag| {
            let keep = wanted.contains(id);
            if !keep {
                flag.store(true, Ordering::SeqCst);
            }
            keep
        });
        for target in targets {
            if table.workers.contains_key(&target.id) {
                continue;
            }
            let flag = Arc::new(AtomicBool::new(false));
            table.workers.insert(target.id.clone(), Arc::clone(&flag));
            let me = Arc::clone(self);
            me.counters.threads.fetch_add(1, Ordering::SeqCst);
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-link".into())
                .spawn(move || {
                    me.work(target, flag);
                    me.counters.threads.fetch_sub(1, Ordering::SeqCst);
                });
        }
    }

    /// Whether this runtime should dial a known peer: the lower runtime key dials, and
    /// the higher one dials only when nothing has arrived from the peer lately — so a
    /// pair keeps one link when both can reach each other, and two when only one can.
    fn should_dial(&self, key: &str) -> bool {
        if self.own_key.as_str() < key {
            return true;
        }
        let table = self.table.lock().expect("cooperation table");
        let quiet = 3 * self.heartbeat();
        !table.peers.get(key).is_some_and(|p| {
            p.inbound.is_some() && p.last_exchange.is_some_and(|t| t.elapsed() <= quiet)
        })
    }

    fn outbound_key(&self, key: &str) -> bool {
        let table = self.table.lock().expect("cooperation table");
        table.peers.get(key).is_some_and(|p| p.outbound.is_some())
    }

    fn work(&self, target: Target, flag: Arc<AtomicBool>) {
        let mut learned = target.key.clone();
        let mut failures: u32 = 0;
        while !self.stopped() && !flag.load(Ordering::SeqCst) {
            let linked = learned
                .as_deref()
                .filter(|k| self.outbound_key(k))
                .map(str::to_string);
            let outcome = match linked {
                Some(key) => self.sync_with(&key),
                None => {
                    let may = target.key.as_deref().is_none_or(|k| self.should_dial(k));
                    if !may {
                        self.sleep(self.heartbeat());
                        continue;
                    }
                    match self.dial(&target.endpoints) {
                        Ok(key) => {
                            learned = Some(key.clone());
                            // A fresh link syncs at once: the peer should see this
                            // runtime's state now, not a heartbeat from now.
                            self.sync_with(&key)
                        }
                        Err(e) => Err(e),
                    }
                }
            };
            let delay = match outcome {
                Ok(()) => {
                    failures = 0;
                    self.heartbeat()
                }
                Err(RoundError::Refused(r)) if r.code == RefusalCode::UnknownLink => {
                    failures = 0;
                    Duration::ZERO
                }
                Err(RoundError::Refused(_)) => {
                    failures = failures.saturating_add(3).min(6);
                    self.backoff(failures)
                }
                Err(_) => {
                    failures = failures.saturating_add(1).min(6);
                    self.backoff(failures)
                }
            };
            self.sleep(delay);
        }
    }

    /// Heartbeat × 2^failures, never beyond the expiry: an unreachable peer is asked less
    /// and less, and still within one expiry of coming back.
    fn backoff(&self, failures: u32) -> Duration {
        let factor = 1u32 << failures.min(6);
        (self.heartbeat() * factor).min(self.expiry().max(self.heartbeat()))
    }

    fn expire_links(&self) {
        let expiry = self.expiry();
        let mut table = self.table.lock().expect("cooperation table");
        let mut dropped_links = Vec::new();
        let mut gone = Vec::new();
        for (key, peer) in table.peers.iter_mut() {
            let age = peer.last_exchange.map(|t| t.elapsed());
            if age.is_some_and(|a| a > expiry) && !peer.expired {
                peer.expired = true;
                if let Some(inbound) = peer.inbound.take() {
                    dropped_links.push(inbound.link);
                }
                peer.outbound = None;
                self.counters.peers_expired.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(runtime_id = %self.own_key, peer = %key, "mesh link expired: no exchange within the expiry");
            }
            if age.is_some_and(|a| a > expiry + PEER_RETENTION) {
                gone.push(key.clone());
            }
        }
        for link in dropped_links {
            table.inbound_links.remove(&link);
        }
        for key in gone {
            table.peers.remove(&key);
        }
    }

    fn state_of(&self, peer: &PeerLink) -> LinkState {
        let Some(age) = peer.last_exchange.map(|t| t.elapsed()) else {
            return LinkState::Connecting;
        };
        let expiry = self.expiry();
        if peer.expired || age > expiry {
            LinkState::Expired
        } else if age <= 2 * self.heartbeat() + Duration::from_secs(1) {
            LinkState::Connected
        } else if age <= expiry / 2 {
            LinkState::Degraded
        } else {
            LinkState::Unreachable
        }
    }

    // ------------------------------------------------------------------ reading

    /// Every peer, by runtime key.
    pub fn peers(&self) -> Vec<PeerView> {
        let table = self.table.lock().expect("cooperation table");
        let own_node = self.identity.public.node_id.as_str();
        table
            .peers
            .iter()
            .map(|(key, peer)| PeerView {
                runtime: key.clone(),
                node: key[..32].to_string(),
                instance: peer.card.instance.clone(),
                name: peer.card.name.clone(),
                version: peer.card.version.clone(),
                local: &key[..32] == own_node,
                trust: peer.trust.clone(),
                protocol: peer.proto,
                features: peer.card.features.clone(),
                endpoints: peer.card.endpoints.clone(),
                state: self.state_of(peer),
                outbound: peer.outbound.is_some(),
                inbound: peer.inbound.is_some(),
                endpoint: peer.outbound.as_ref().map(|o| o.endpoint.clone()),
                last_exchange_ms: peer.last_exchange.map(|t| t.elapsed().as_millis() as u64),
                rtt_ms: peer.rtt.map(|d| d.as_millis() as u64),
                failures: peer.failures,
                last_error: peer.last_error.clone(),
                handshakes: peer.handshakes,
                reconnects: peer.reconnects,
                restarts: peer.restarts,
                linked_at: peer.linked_at.clone(),
            })
            .collect()
    }

    /// The whole status.
    pub fn status(&self) -> CooperationStatus {
        let refused = {
            let table = self.table.lock().expect("cooperation table");
            table
                .refused
                .iter()
                .map(|(endpoint, r)| RefusedView {
                    endpoint: endpoint.clone(),
                    direction: r.direction.into(),
                    runtime: r.runtime.clone(),
                    refusal: r.refusal.clone(),
                    at: r.at.clone(),
                })
                .collect()
        };
        let c = &self.counters;
        let load = |a: &AtomicU64| a.load(Ordering::Relaxed);
        CooperationStatus {
            active: !self.stopped() && self.config.enabled,
            reason: if self.stopped() {
                Some("stopped".into())
            } else if !self.config.enabled {
                Some("cooperation is disabled in the mesh declaration".into())
            } else {
                None
            },
            runtime: Some(self.own_key.clone()),
            stream: Some(self.journal.own_stream().clone()),
            repository: Some(self.repository.clone()),
            protocol: (LINK_PROTOCOL_MIN, LINK_PROTOCOL_MAX),
            features: self.card.features.clone(),
            heartbeat_seconds: self.config.heartbeat_seconds,
            expiry_seconds: self.config.expiry_seconds,
            endpoints: self.card.endpoints.clone(),
            seeds: self.config.seeds.clone(),
            peers: self.peers(),
            refused,
            counters: CooperationCounters {
                handshakes_out: load(&c.handshakes_out),
                handshakes_in: load(&c.handshakes_in),
                dial_failures: load(&c.dial_failures),
                syncs_out: load(&c.syncs_out),
                syncs_failed: load(&c.syncs_failed),
                syncs_in: load(&c.syncs_in),
                events_sent: load(&c.events_sent),
                events_served: load(&c.events_served),
                reconnects: load(&c.reconnects),
                restarts: load(&c.restarts),
                peers_expired: load(&c.peers_expired),
                claims_refused: load(&c.claims_refused),
                threads: c.threads.load(Ordering::SeqCst),
                refused_in: c.refused_in.lock().expect("counters").clone(),
                refused_out: c.refused_out.lock().expect("counters").clone(),
            },
            journal: self.journal.tallies(),
            started_at: Some(self.started_at.clone()),
        }
    }

    /// The folded state, liveness judged on this runtime's clock now.
    pub fn state(&self) -> CooperationState {
        let live = self.journal.stream_liveness(self.expiry());
        fold(&self.journal.events(), &|s| {
            live.get(s)
                .map(|(l, _)| *l)
                .unwrap_or(StreamLiveness::Expired)
        })
    }

    /// Every stream's liveness and the milliseconds since its beat rose.
    pub fn streams(&self) -> Vec<(StreamId, StreamLiveness, Option<u64>)> {
        self.journal
            .stream_liveness(self.expiry())
            .into_iter()
            .map(|(id, (l, age))| (id, l, age.map(|a| a.as_millis() as u64)))
            .collect()
    }

    /// Verify cooperation now: the local checks, then one sync round with every peer this
    /// runtime dials, and whether both sides hold the same marks afterwards.
    pub fn verify(&self) -> MeshVerifyReport {
        let mut checks = Vec::new();
        let heartbeat = self.heartbeat();
        checks.push(MeshVerifyCheck {
            check: "cooperation".into(),
            ok: !self.stopped() && self.config.enabled,
            detail: format!(
                "runtime {} — heartbeat {}s, expiry {}s, link protocol {LINK_PROTOCOL_MIN}..{LINK_PROTOCOL_MAX}",
                self.own_key, self.config.heartbeat_seconds, self.config.expiry_seconds
            ),
            impact: None,
            remediation: None,
        });
        checks.push(MeshVerifyCheck {
            check: "repository".into(),
            ok: true,
            detail: format!(
                "{} from {:?}: {}",
                self.repository.id, self.repository.basis, self.repository.detail
            ),
            impact: None,
            remediation: None,
        });
        let loopback = super::address::loopback_only(&self.card.endpoints);
        checks.push(MeshVerifyCheck {
            check: "endpoints".into(),
            ok: true,
            detail: self.card.endpoints.join(", "),
            impact: loopback.then(|| {
                "only runtimes on this machine can dial this one; this runtime can still dial remote runtimes, and a link works in both directions once dialed".into()
            }),
            remediation: loopback.then(|| {
                "to be dialed from other machines, serve beyond loopback: majordomus serve --host 0.0.0.0 (or a deployment object), on a private network or an overlay".into()
            }),
        });
        let own_age = self
            .journal
            .stream_liveness(self.expiry())
            .get(self.journal.own_stream())
            .and_then(|(_, age)| *age);
        let beating = own_age.is_some_and(|a| a <= 2 * heartbeat + Duration::from_secs(1));
        checks.push(MeshVerifyCheck {
            check: "heartbeat".into(),
            ok: beating,
            detail: match own_age {
                Some(a) => format!("this runtime's beat rose {}ms ago", a.as_millis()),
                None => "this runtime has not beaten yet".into(),
            },
            impact: (!beating).then(|| "linked runtimes will expire this one and stop honouring its claims".into()),
            remediation: (!beating).then(|| "the cooperation thread is not running: restart the server (majordomus serve stop && majordomus serve ensure)".into()),
        });
        let tallies = self.journal.tallies();
        checks.push(MeshVerifyCheck {
            check: "journal".into(),
            ok: tallies.pending == 0,
            detail: format!(
                "{} events in {} streams; {} received, {} duplicates absorbed, {} rejected, {} pending, {} opaque",
                tallies.events, tallies.streams, tallies.received, tallies.duplicates, tallies.rejected, tallies.pending, tallies.opaque
            ),
            impact: (tallies.pending > 0).then(|| "events after a gap are held until the gap arrives; the state lags the stream they belong to".into()),
            remediation: (tallies.pending > 0).then(|| "a peer relays a partial stream: run mesh verify on the peers and compare their marks".into()),
        });
        let refused = self.status().refused;
        let suspicious: Vec<&RefusedView> = refused
            .iter()
            .filter(|r| {
                matches!(
                    r.refusal.code,
                    RefusalCode::Untrusted
                        | RefusalCode::ProtocolUnsupported
                        | RefusalCode::Signature
                )
            })
            .collect();
        checks.push(MeshVerifyCheck {
            check: "refusals".into(),
            // A runtime of this repository refused for trust, protocol or signature is a
            // misconfiguration or an attack, never a healthy mesh. A repository mismatch is
            // isolation working and does not fail the verification.
            ok: suspicious.is_empty(),
            detail: if refused.is_empty() {
                "no candidate refused".into()
            } else {
                refused
                    .iter()
                    .map(|r| format!("{} {}: {}", r.endpoint, r.refusal.code.as_str(), r.refusal.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            },
            impact: (!suspicious.is_empty()).then(|| {
                "a runtime of this repository may be refused for trust, protocol or signature: it cooperates with nobody here".into()
            }),
            remediation: (!suspicious.is_empty()).then(|| {
                "untrusted: add its key (majordomus mesh identity on that machine) to trust.allow; protocol: upgrade the older executable; signature: that peer's identity file changed or the traffic is tampered".into()
            }),
        });

        let mut peers = Vec::new();
        for peer in self.peers() {
            if peer.outbound {
                let outcome = self.sync_with(&peer.runtime);
                let after = self.peers().into_iter().find(|p| p.runtime == peer.runtime);
                let remote = {
                    let table = self.table.lock().expect("cooperation table");
                    table
                        .peers
                        .get(&peer.runtime)
                        .and_then(|p| p.outbound.as_ref())
                        .map(|o| o.remote_marks.clone())
                };
                let converged = remote.map(|remote| {
                    let local = self.journal.marks();
                    local.keys().chain(remote.keys()).all(|s| {
                        local.get(s).map(|m| m.seq).unwrap_or(0)
                            == remote.get(s).map(|m| m.seq).unwrap_or(0)
                    })
                });
                peers.push(PeerVerdict {
                    runtime: peer.runtime.clone(),
                    state: after
                        .as_ref()
                        .map(|p| p.state)
                        .unwrap_or(LinkState::Expired),
                    dialed: true,
                    round_trip: Some(outcome.is_ok()),
                    rtt_ms: after.and_then(|p| p.rtt_ms),
                    converged,
                    detail: match outcome {
                        Ok(()) => "one sync round, signed both ways".into(),
                        Err(e) => e.to_string(),
                    },
                });
            } else {
                let fresh = peer.state == LinkState::Connected;
                peers.push(PeerVerdict {
                    runtime: peer.runtime.clone(),
                    state: peer.state,
                    dialed: false,
                    round_trip: None,
                    rtt_ms: peer.rtt_ms,
                    converged: None,
                    detail: match peer.last_exchange_ms {
                        Some(ms) if fresh => {
                            format!("the peer dials this runtime; last exchange {ms}ms ago")
                        }
                        Some(ms) => {
                            format!("the peer dials this runtime and has been silent {ms}ms")
                        }
                        None => "the peer has not exchanged anything yet".into(),
                    },
                });
            }
        }
        // Declared seeds say that peers exist: a runtime with seeds and no connected peer has
        // verified nothing, however clean its local checks are.
        let connected = peers
            .iter()
            .filter(|p| p.state == LinkState::Connected)
            .count();
        let expected = self.config.seeds.len();
        let starved = expected > 0 && connected == 0;
        checks.push(MeshVerifyCheck {
            check: "links".into(),
            ok: !starved,
            detail: format!(
                "{connected} connected of {} linked peer(s); {expected} seed(s) declared",
                peers.len()
            ),
            impact: starved.then(|| {
                "the seeds this repository declares cooperate with nobody here: no claim, handover or review crosses".into()
            }),
            remediation: starved.then(|| {
                "read the refusals above; an unreachable seed is a network or bind problem — is the peer served beyond loopback?".into()
            }),
        });
        // An expired peer is listed with its state and does not fail the verification: a
        // peer that shut down cleanly is gone, not broken. A round that fails does.
        let ok = checks.iter().all(|c| c.ok)
            && peers
                .iter()
                .all(|p| p.state == LinkState::Expired || p.round_trip != Some(false));
        MeshVerifyReport {
            ok,
            runtime: Some(self.own_key.clone()),
            checks,
            peers,
            digest: Some(self.state().digest),
        }
    }

    // ------------------------------------------------------------------ operations

    fn write(&self, body: EventBody, key: String) -> Result<Written, CooperationError> {
        let event = self
            .journal
            .append_own(body)
            .map_err(|e| CooperationError::Invalid(e.to_string()))?;
        Ok(Written {
            key,
            event: event.id(),
            lamport: event.lamport,
        })
    }

    fn own(&self, local: &str) -> String {
        format!("{}/{local}", self.journal.own_stream())
    }

    fn session_exists(&self, state: &CooperationState, session: &str) -> bool {
        let key = self.own(session);
        state.sessions.iter().any(|s| s.key == key)
    }

    /// Open (or update) a session of this runtime.
    pub fn open_session(&self, info: SessionInfo) -> Result<Written, CooperationError> {
        let key = self.own(&info.session);
        self.write(EventBody::SessionOpened { info }, key)
    }

    /// Close a session of this runtime; its claims end with it.
    pub fn close_session(&self, session: &str) -> Result<Written, CooperationError> {
        let state = self.state();
        if !self.session_exists(&state, session) {
            return Err(CooperationError::NotFound(format!(
                "no session '{session}' on this runtime"
            )));
        }
        self.write(
            EventBody::SessionClosed {
                session: session.into(),
            },
            self.own(session),
        )
    }

    /// Claim a scope for a session of this runtime. An exclusive claim that meets a live
    /// exclusive claim of another session — on any runtime this one has heard — is
    /// refused with the claims it meets; the session is opened first if it is new.
    pub fn claim(
        &self,
        session: &SessionInfo,
        scope: Vec<String>,
        intent: Option<String>,
        mode: ClaimMode,
        issue: Option<String>,
    ) -> Result<Written, CooperationError> {
        // Check and append under one lock: two callers of this runtime must not both pass
        // admission for one scope and leave the fold to name a conflict it could prevent.
        let _admission = self.admission.lock().expect("claim admission");
        let state = self.state();
        let conflicts = admission_conflicts(&state, &self.own(&session.session), &scope, mode);
        if !conflicts.is_empty() {
            self.counters.claims_refused.fetch_add(1, Ordering::Relaxed);
            let conflicts: Vec<ClaimView> = conflicts.into_iter().cloned().collect();
            tracing::info!(runtime_id = %self.own_key, session_id = %session.session, conflicts = conflicts.len(), "mesh claim refused: the scope is claimed");
            return Err(CooperationError::Conflict(conflicts));
        }
        if !self.session_exists(&state, &session.session) {
            self.open_session(session.clone())?;
        }
        let claim = format!("c-{}", &fresh_token()[..12]);
        self.write(
            EventBody::ClaimAcquired {
                claim: claim.clone(),
                session: session.session.clone(),
                scope,
                intent,
                mode,
                issue,
            },
            self.own(&claim),
        )
    }

    /// Release a claim this runtime holds.
    pub fn release(&self, claim_key: &str) -> Result<Written, CooperationError> {
        let own_prefix = format!("{}/", self.journal.own_stream());
        let Some(local) = claim_key.strip_prefix(&own_prefix) else {
            return Err(CooperationError::NotOwn(format!(
                "{claim_key} was not claimed by this runtime's current run; only its holder releases a claim, and a dead holder's claim expires on its own"
            )));
        };
        let state = self.state();
        if !state.claims.iter().any(|c| c.key == claim_key) {
            return Err(CooperationError::NotFound(format!("no claim {claim_key}")));
        }
        self.write(
            EventBody::ClaimReleased {
                claim: local.into(),
            },
            claim_key.into(),
        )
    }

    /// Publish a handover for every linked runtime.
    pub fn publish_handover(&self, handover: HandoverBody) -> Result<Written, CooperationError> {
        let id = handover.id.clone();
        self.write(EventBody::HandoverPublished { handover }, id)
    }

    /// Record that a session of this runtime consumed a handover, and return it.
    pub fn consume_handover(
        &self,
        id: &str,
        session: &str,
    ) -> Result<(HandoverView, Written), CooperationError> {
        let state = self.state();
        let Some(view) = state.handovers.into_iter().find(|h| h.id == id) else {
            return Err(CooperationError::NotFound(format!(
                "no handover {id} in this runtime's journal"
            )));
        };
        let written = self.write(
            EventBody::HandoverConsumed {
                handover: id.into(),
                session: session.into(),
            },
            id.into(),
        )?;
        Ok((view, written))
    }

    /// Ask for a review; a named reviewer must be a linked peer carrying `reviews`.
    pub fn request_review(
        &self,
        session: &str,
        subject: String,
        scope: Vec<String>,
        issue: Option<String>,
        reviewer: Option<String>,
    ) -> Result<Written, CooperationError> {
        if let Some(target) = &reviewer {
            if *target != self.own_key {
                let table = self.table.lock().expect("cooperation table");
                match table.peers.get(target) {
                    None => {
                        return Err(CooperationError::NotFound(format!(
                            "no linked peer {target}"
                        )))
                    }
                    Some(peer) if !peer.card.supports("reviews") => {
                        return Err(CooperationError::FeatureUnsupported(format!(
                            "{target} does not carry the reviews feature (it carries: {})",
                            peer.card.features.join(", ")
                        )))
                    }
                    Some(_) => {}
                }
            }
        }
        let review = format!("r-{}", &fresh_token()[..12]);
        self.write(
            EventBody::ReviewRequested {
                review: review.clone(),
                session: session.into(),
                subject,
                scope,
                issue,
                reviewer,
            },
            self.own(&review),
        )
    }

    /// Answer a review request, from any runtime.
    pub fn answer_review(
        &self,
        request: &str,
        session: &str,
        verdict: String,
        note: Option<String>,
    ) -> Result<Written, CooperationError> {
        let state = self.state();
        let Some(review) = state.reviews.iter().find(|r| r.key == request) else {
            return Err(CooperationError::NotFound(format!(
                "no review request {request}"
            )));
        };
        if review.session == self.own(session) {
            return Err(CooperationError::Invalid(format!(
                "session {session} requested review {request} and cannot answer it: a review is another session's verdict"
            )));
        }
        self.write(
            EventBody::ReviewAnswered {
                request: request.into(),
                session: session.into(),
                verdict,
                note,
            },
            request.into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::repository::of_root_commits;
    use crate::mesh::trust::TrustPolicy;
    use std::sync::Weak;

    /// Two or more runtimes in one process, linked through this transport: the protocol
    /// under test is the real one; only the socket is replaced. Real sockets are the
    /// integration tests' (tests/mesh_cooperation.rs).
    #[derive(Default)]
    struct InProcess {
        runtimes: Mutex<BTreeMap<String, Weak<Cooperation>>>,
        cut: Mutex<Vec<String>>,
    }

    impl LinkTransport for InProcess {
        fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
            if self.cut.lock().unwrap().iter().any(|c| c == endpoint) {
                return Err(format!("{endpoint}: partitioned"));
            }
            let target = self
                .runtimes
                .lock()
                .unwrap()
                .get(endpoint)
                .and_then(Weak::upgrade)
                .ok_or_else(|| format!("{endpoint}: connection refused"))?;
            Ok(match path {
                HELLO_PATH => target.accept_hello(message),
                SYNC_PATH => target.accept_sync(message),
                other => return Err(format!("no route {other}")),
            })
        }
    }

    fn runtime(
        net: &Arc<InProcess>,
        endpoint: &str,
        repo: &str,
        policy: TrustPolicy,
    ) -> Arc<Cooperation> {
        let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        let c = Cooperation::new(CooperationSetup {
            identity,
            runtime: crate::mesh::repository::runtime_id(endpoint),
            repository: of_root_commits(&[repo.into()]),
            endpoints: vec![endpoint.into()],
            version: "test".into(),
            config: CooperationConfig {
                heartbeat_seconds: 1,
                expiry_seconds: 3,
                ..CooperationConfig::default()
            },
            trust: TrustConfig {
                policy,
                allow: vec![],
            },
            journal_path: None,
            registry: Arc::new(MeshRegistry::new()),
            transport: Arc::clone(net) as Arc<dyn LinkTransport>,
            board: None,
            checkout: CheckoutFacts::default(),
        })
        .unwrap();
        net.runtimes
            .lock()
            .unwrap()
            .insert(endpoint.into(), Arc::downgrade(&c));
        c
    }

    fn session(name: &str) -> SessionInfo {
        SessionInfo::named(name, "test")
    }

    #[test]
    fn a_hello_links_both_ways_and_a_claim_crosses_the_link() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let key = a.dial(&["b:1".into()]).expect("B welcomes A");
        assert_eq!(key, b.runtime_key());
        assert_eq!(
            b.peers()[0].runtime,
            a.runtime_key(),
            "B holds A as its inbound peer"
        );

        a.claim(
            &session("s1"),
            vec!["apps".into()],
            None,
            ClaimMode::Exclusive,
            Some("#184".into()),
        )
        .unwrap();
        a.sync_with(&key).unwrap();
        a.journal.beat_own();
        a.sync_with(&key).unwrap();
        let at_b = b.state();
        assert_eq!(at_b.claims.len(), 1, "B sees A's claim");
        let refused = b.claim(
            &session("s9"),
            vec!["apps/majordomus-cli".into()],
            None,
            ClaimMode::Exclusive,
            None,
        );
        assert!(
            matches!(refused, Err(CooperationError::Conflict(ref c)) if c.len() == 1),
            "{refused:?}"
        );
        assert_eq!(
            a.state().digest,
            b.state().digest,
            "one state, two runtimes"
        );
    }

    #[test]
    fn isolation_version_trust_and_replay_are_typed_refusals() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let other_repo = runtime(&net, "x:1", "another-root", TrustPolicy::Tofu);
        let strict = runtime(&net, "s:1", "root", TrustPolicy::DenyUnknown);

        let code = |r: Result<String, RoundError>| match r {
            Err(RoundError::Refused(refusal)) => refusal.code,
            other => panic!("expected a refusal, got {other:?}"),
        };
        assert_eq!(
            code(a.dial(&["x:1".into()])),
            RefusalCode::RepositoryMismatch
        );
        assert!(
            other_repo.peers().is_empty(),
            "no cooperative relationship formed"
        );
        assert_eq!(code(a.dial(&["s:1".into()])), RefusalCode::Untrusted);
        assert!(strict.peers().is_empty());
        assert_eq!(code(a.dial(&["a:1".into()])), RefusalCode::SelfLink);

        // A hello from a future protocol range, properly signed.
        let future = Hello {
            proto_min: LINK_PROTOCOL_MAX + 1,
            proto_max: LINK_PROTOCOL_MAX + 5,
            card: a.card.clone(),
            nonce: fresh_token(),
            ts: crate::mesh::protocol::now(),
        };
        let signed = sign(
            &a.identity,
            Domain::Hello,
            serde_json::to_value(&future).unwrap(),
        );
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let reply = b.accept_hello(&signed);
        let refusal = reply.refusal.expect("refused");
        assert_eq!(refusal.code, RefusalCode::ProtocolUnsupported);
        assert_eq!(
            (refusal.proto_min, refusal.proto_max),
            (LINK_PROTOCOL_MIN, LINK_PROTOCOL_MAX)
        );

        // A replayed hello. (A fresh nonce first: the refused future hello's nonce is
        // already spent, which is exactly the replay rule.)
        let hello = Hello {
            proto_min: 1,
            proto_max: 1,
            nonce: fresh_token(),
            ..future
        };
        let signed = sign(
            &a.identity,
            Domain::Hello,
            serde_json::to_value(&hello).unwrap(),
        );
        assert!(b.accept_hello(&signed).signed.is_some());
        assert_eq!(
            b.accept_hello(&signed).refusal.unwrap().code,
            RefusalCode::Replay
        );

        // Garbage, a forged signature, and an oversized message.
        let garbage = Signed {
            body: serde_json::json!({"nope": true}),
            sig: "00".into(),
        };
        assert_eq!(
            b.accept_hello(&garbage).refusal.unwrap().code,
            RefusalCode::Malformed
        );
        let mut forged = sign(
            &a.identity,
            Domain::Hello,
            serde_json::to_value(&Hello {
                nonce: fresh_token(),
                ..hello.clone()
            })
            .unwrap(),
        );
        forged.body["card"]["name"] = serde_json::json!("impostor");
        assert_eq!(
            b.accept_hello(&forged).refusal.unwrap().code,
            RefusalCode::Signature
        );
        let huge = Signed {
            body: serde_json::json!({"x": "y".repeat(MAX_LINK_MESSAGE)}),
            sig: String::new(),
        };
        assert_eq!(
            b.accept_hello(&huge).refusal.unwrap().code,
            RefusalCode::Oversized
        );
        assert!(b.status().counters.refused_in.len() >= 4);
    }

    #[test]
    fn a_sync_under_an_unknown_link_or_a_replayed_counter_is_refused() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let key = a.dial(&["b:1".into()]).unwrap();
        a.sync_with(&key).unwrap();
        let replay = {
            let table = a.table.lock().unwrap();
            let out = table.peers[&key].outbound.clone().unwrap();
            SyncRequest {
                link: out.link,
                counter: out.counter,
                instance: a.card.instance.clone(),
                marks: Marks::new(),
                events: vec![],
            }
        };
        let signed = sign(
            &a.identity,
            Domain::SyncRequest,
            serde_json::to_value(&replay).unwrap(),
        );
        assert_eq!(
            b.accept_sync(&signed).refusal.unwrap().code,
            RefusalCode::Replay
        );
        let unknown = SyncRequest {
            link: fresh_token(),
            counter: 99,
            ..replay
        };
        let signed = sign(
            &a.identity,
            Domain::SyncRequest,
            serde_json::to_value(&unknown).unwrap(),
        );
        assert_eq!(
            b.accept_sync(&signed).refusal.unwrap().code,
            RefusalCode::UnknownLink
        );
    }

    #[test]
    fn three_runtimes_in_a_line_converge_without_loops_or_duplicates() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let c = runtime(&net, "c:1", "root", TrustPolicy::Tofu);
        let ab = a.dial(&["b:1".into()]).unwrap();
        let cb = c.dial(&["b:1".into()]).unwrap();
        a.claim(
            &session("sa"),
            vec!["apps".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .unwrap();
        c.claim(
            &session("sc"),
            vec!["docs".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .unwrap();
        for _ in 0..3 {
            for j in [&a, &b, &c] {
                j.journal.beat_own();
            }
            a.sync_with(&ab).unwrap();
            c.sync_with(&cb).unwrap();
        }
        let digests: Vec<String> = [&a, &b, &c].iter().map(|r| r.state().digest).collect();
        assert!(
            digests.windows(2).all(|w| w[0] == w[1]),
            "A, B and C converge: {digests:?}"
        );
        let state = c.state();
        assert_eq!(state.claims.len(), 2);
        assert_eq!(
            c.journal.events().len(),
            4,
            "two sessions and two claims, each once"
        );
        // More rounds carry nothing: replication stopped, no event loops back.
        let before = b.status().counters.events_served;
        a.sync_with(&ab).unwrap();
        c.sync_with(&cb).unwrap();
        assert_eq!(
            b.status().counters.events_served,
            before,
            "nothing left to serve"
        );
        assert_eq!(
            a.status().journal.duplicates + c.status().journal.duplicates,
            0
        );
    }

    #[test]
    fn a_partition_expires_the_far_side_and_healing_reconciles_to_one_winner() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let key = a.dial(&["b:1".into()]).unwrap();
        a.journal.beat_own();
        b.journal.beat_own();
        a.sync_with(&key).unwrap();

        // Partition: both sides claim the same scope, as each is allowed to.
        net.cut.lock().unwrap().push("b:1".into());
        assert!(matches!(a.sync_with(&key), Err(RoundError::Unreachable(_))));
        assert!(a.peers()[0].failures >= 1);
        a.claim(
            &session("s1"),
            vec!["apps".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(3200));
        b.claim(
            &session("s2"),
            vec!["apps".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .expect("A's stream has expired at B, so nothing there refuses B");

        // Heal: one round each way, and both name the same single winner.
        net.cut.lock().unwrap().clear();
        a.journal.beat_own();
        b.journal.beat_own();
        match a.sync_with(&key) {
            Ok(()) => {}
            Err(RoundError::Refused(r)) if r.code == RefusalCode::UnknownLink => {
                let key = a.dial(&["b:1".into()]).unwrap();
                a.sync_with(&key).unwrap();
            }
            Err(e) => panic!("{e}"),
        }
        let (sa, sb) = (a.state(), b.state());
        assert_eq!(sa.digest, sb.digest, "the partition healed into one state");
        let held = sa
            .claims
            .iter()
            .filter(|c| c.state == crate::mesh::state::ClaimState::Held)
            .count();
        let conflicted = sa
            .claims
            .iter()
            .filter(|c| matches!(c.state, crate::mesh::state::ClaimState::Conflicted(_)))
            .count();
        assert_eq!(
            (held, conflicted),
            (1, 1),
            "one winner, one named conflict — never two holders"
        );
    }

    #[test]
    fn a_reviewer_must_carry_the_feature_and_stop_ends_every_thread() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let b = runtime(&net, "b:1", "root", TrustPolicy::Tofu);
        let key = a.dial(&["b:1".into()]).unwrap();
        a.table
            .lock()
            .unwrap()
            .peers
            .get_mut(&key)
            .unwrap()
            .card
            .features = vec!["claims".into()];
        let refused = a.request_review("s1", "feature/x".into(), vec![], None, Some(key.clone()));
        assert!(
            matches!(refused, Err(CooperationError::FeatureUnsupported(_))),
            "{refused:?}"
        );
        assert!(matches!(
            a.request_review(
                "s1",
                "feature/x".into(),
                vec![],
                None,
                Some("nobody".into())
            ),
            Err(CooperationError::NotFound(_))
        ));
        assert!(a
            .request_review("s1", "feature/x".into(), vec![], None, None)
            .is_ok());

        a.start();
        b.start();
        std::thread::sleep(Duration::from_millis(300));
        assert!(a.status().counters.threads >= 1);
        a.stop();
        b.stop();
        let deadline = Instant::now() + Duration::from_secs(3);
        while (a.status().counters.threads > 0 || b.status().counters.threads > 0)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            a.status().counters.threads,
            0,
            "stop leaves no cooperation thread behind"
        );
        assert_eq!(
            a.accept_hello(&Signed {
                body: serde_json::json!({}),
                sig: String::new()
            })
            .refusal
            .unwrap()
            .code,
            RefusalCode::NotActive
        );
    }

    #[test]
    fn the_board_is_projected_into_sessions_and_advisory_claims_and_withdrawn_with_it() {
        let net = Arc::new(InProcess::default());
        let board = Arc::new(crate::peers::PeerBoard::new());
        let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        let a = Cooperation::new(CooperationSetup {
            identity,
            runtime: "00000000000000a1".into(),
            repository: of_root_commits(&["root".into()]),
            endpoints: vec!["a:1".into()],
            version: "test".into(),
            config: CooperationConfig::default(),
            trust: TrustConfig::default(),
            journal_path: None,
            registry: Arc::new(MeshRegistry::new()),
            transport: Arc::clone(&net) as Arc<dyn LinkTransport>,
            board: Some(Arc::clone(&board)),
            checkout: CheckoutFacts::default(),
        })
        .unwrap();
        let p = board.attach(crate::peers::Transport::Http);
        board.announce(&p, "implement the mesh", vec!["apps/majordomus-cli".into()]);
        a.project_board();
        a.project_board();
        let state = a.state();
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(
            state.sessions[0].info.intent.as_deref(),
            Some("implement the mesh")
        );
        assert_eq!(state.claims.len(), 1, "projecting twice claims once");
        assert_eq!(state.claims[0].mode, ClaimMode::Advisory);

        board.announce(&p, "write the docs", vec!["docs".into()]);
        a.project_board();
        let state = a.state();
        let live: Vec<_> = state.claims.iter().filter(|c| c.state.is_live()).collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].scope, vec!["docs".to_string()]);

        board.detach(&p);
        a.project_board();
        let state = a.state();
        assert!(
            state.claims.iter().all(|c| !c.state.is_live()),
            "a departed session holds nothing"
        );
    }

    #[test]
    fn a_release_is_the_holders_and_a_session_close_ends_its_claims() {
        let net = Arc::new(InProcess::default());
        let a = runtime(&net, "a:1", "root", TrustPolicy::Tofu);
        let w = a
            .claim(
                &session("s1"),
                vec!["apps".into()],
                None,
                ClaimMode::Exclusive,
                None,
            )
            .unwrap();
        assert!(matches!(
            a.release("someone-else/c-1"),
            Err(CooperationError::NotOwn(_))
        ));
        a.release(&w.key).unwrap();
        let w2 = a
            .claim(
                &session("s1"),
                vec!["docs".into()],
                None,
                ClaimMode::Exclusive,
                None,
            )
            .unwrap();
        a.close_session("s1").unwrap();
        let state = a.state();
        let second = state.claims.iter().find(|c| c.key == w2.key).unwrap();
        assert!(matches!(
            second.state,
            crate::mesh::state::ClaimState::Expired(_)
        ));
    }
}
