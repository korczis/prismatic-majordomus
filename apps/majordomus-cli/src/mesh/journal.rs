//! The cooperation journal: the one replicated record of what the runtimes of a repository
//! said to each other. Discovery ([`super::registry`]) answers "which runtimes exist";
//! the journal answers "what are they doing" — sessions, claims, handovers, reviews —
//! as signed events that every linked runtime stores, relays and folds into the same
//! state ([`super::state`]).
//!
//! The guarantees are exactly these, and the tests hold each one:
//!
//! - **Identity.** An event is written by one *stream* — one run of one runtime of one
//!   node ([`StreamId`]) — and numbered densely from 1 within it. `(stream, seq)` is the
//!   event's identity everywhere.
//! - **Authenticity end to end.** The origin signs every event with its node key; a relay
//!   forwards the bytes and vouches for nothing, so a relay cannot forge, alter or
//!   re-attribute an event.
//! - **At-least-once delivery, idempotent application.** An event received twice is a
//!   counted duplicate and changes nothing; the application order within a stream is its
//!   sequence, never the arrival order — an early arrival waits (bounded) for its gap.
//! - **No loops.** Replication is a comparison of high-water marks, not a flood: a
//!   runtime sends a peer only what the peer's marks say it lacks, so an event reaches
//!   every runtime once per link and stops.
//! - **Liveness without shared clocks.** Each stream carries a beat its origin raises on
//!   every heartbeat; a mark relays the beat with how long ago the sender saw it rise.
//!   A runtime computes freshness on its own monotonic clock — no two machines' wall
//!   clocks are ever compared.
//!
//! What it does not promise: exactly-once delivery, a global order across streams beyond
//! the Lamport stamp, or consistency under partition. Two sides of a partition may both
//! act; the fold makes the result deterministic and names the conflict (ADR 0065).
//!
//! ```
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo};
//! use std::sync::Arc;
//!
//! let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
//!     "repo".into(), None).unwrap();
//! let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
//!     "repo".into(), None).unwrap();
//! a.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "docs") }).unwrap();
//!
//! // replication: B tells A what it has, A answers with what B lacks, B ingests it
//! let missing = a.missing_for(&b.marks(), 1 << 20);
//! let report = b.ingest(&missing, &|_| Ok(()));
//! assert_eq!(report.accepted, 1);
//! // delivered again, it is a duplicate and nothing changes
//! assert_eq!(b.ingest(&missing, &|_| Ok(())).duplicate, 1);
//! ```

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::identity::{node_id_of_key, verify, NodeIdentity};
use super::MeshError;

/// The event format version this executable writes and reads.
pub const EVENT_VERSION: u32 = 1;

/// The largest event, serialized, in bytes. A handover body is the largest payload and
/// is bounded below this.
pub const MAX_EVENT_BYTES: usize = 48 * 1024;

/// The largest handover body an event carries, in bytes.
pub const MAX_HANDOVER_BYTES: usize = 32 * 1024;

/// The most events one journal holds before dead streams are compacted away.
pub const MAX_EVENTS: usize = 20_000;

/// The most streams one journal tracks.
pub const MAX_STREAMS: usize = 1024;

/// The most out-of-order events held per stream while their gap is open.
pub const MAX_PENDING: usize = 256;

/// The most paths one claim or review names.
pub const MAX_SCOPE_PATHS: usize = 64;

/// The domain separator of an event signature: a signature over an event can never be
/// replayed as a signature over an advertisement or a link message.
const SIGNING_DOMAIN: &[u8] = b"majordomus-mesh-event/v1\n";

/// One stream: a run of one runtime of one node. `<node 32 hex>-<runtime 16 hex>-<instance
/// 16 hex>`. The node is the machine's key, the runtime is one checkout's server on that
/// machine, the instance is one process run of that server — so two worktrees on one
/// machine are two streams, and a restart is a new stream of the same runtime.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct StreamId(String);

impl StreamId {
    /// A stream id from its three parts; `None` unless each is lowercase hex of its width.
    pub fn new(node: &str, runtime: &str, instance: &str) -> Option<Self> {
        Self::parse(&format!("{node}-{runtime}-{instance}"))
    }

    /// Parse a stream id, refusing anything that is not exactly the three hex parts.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    /// assert!(StreamId::parse("nope").is_none());
    /// let id = format!("{}-{}-{}", "a".repeat(32), "b".repeat(16), "c".repeat(16));
    /// assert_eq!(StreamId::parse(&id).unwrap().runtime_key(), format!("{}-{}", "a".repeat(32), "b".repeat(16)));
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split('-').collect();
        let ok = parts.len() == 3
            && parts[0].len() == 32
            && parts[1].len() == 16
            && parts[2].len() == 16
            && parts
                .iter()
                .all(|p| p.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        ok.then(|| StreamId(text.to_string()))
    }

    /// The node (machine key digest) part.
    pub fn node(&self) -> &str {
        &self.0[..32]
    }

    /// The runtime part.
    pub fn runtime(&self) -> &str {
        &self.0[33..49]
    }

    /// The instance part.
    pub fn instance(&self) -> &str {
        &self.0[50..66]
    }

    /// `<node>-<runtime>`: the durable identity of a runtime across its restarts.
    pub fn runtime_key(&self) -> String {
        self.0[..49].to_string()
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a session tells the mesh about itself. Every field is a public fact about work in
/// progress; none is a path on the author's disk, a secret or a credential.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    /// The session's id within its stream.
    pub session: String,
    /// The client: `claude-code`, `codex`, `cli`, ...
    pub client: String,
    /// The worker's own name for itself, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// What the session says it is doing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The task id (`t-...`) the session works under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue (`I0001`, `#184`) the work belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone the work belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The checkout id (a digest, never a path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    /// The branch checked out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit checked out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Whether the working tree had uncommitted changes when this was said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// The context revision the session works from (a digest the context compiler names).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl SessionInfo {
    /// A session with only an id and a client: what a test or an example needs.
    pub fn named(session: &str, client: &str) -> Self {
        SessionInfo {
            session: session.into(),
            client: client.into(),
            ..SessionInfo::default()
        }
    }
}

/// Whether a claim excludes others or only tells them.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ClaimMode {
    /// Exclusive: a second exclusive claim meeting this scope is refused while this one
    /// lives, on every runtime that knows of it.
    #[default]
    Exclusive,
    /// Advisory: an announcement, like the peer board's. Overlaps are reported, never
    /// refused.
    Advisory,
}

/// A handover as the mesh carries it: the record's facts and its body, bounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HandoverBody {
    /// The content digest (32 hex): the same handover published twice is one handover.
    pub id: String,
    /// The task the handover closes or continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The branch it was written on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit it was written at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// When it was written, RFC 3339, as its author's clock said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// The record's file name at its origin, informational.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The Markdown body (`# Objective`, `# Current State`, `# Next Action`, ...).
    pub body: String,
}

impl HandoverBody {
    /// The content digest a handover body is identified by.
    pub fn digest_of(body: &str) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(body.as_bytes());
        digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }
}

/// What an event says. One vocabulary for every runtime; an event of a kind this
/// executable does not know is stored and relayed but not interpreted, so a newer peer
/// never breaks an older one's replication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventBody {
    /// A session began, or said something new about itself (the latest wins).
    SessionOpened {
        /// The session.
        info: SessionInfo,
    },
    /// A session ended. Its claims end with it.
    SessionClosed {
        /// The session id within the stream.
        session: String,
    },
    /// A session claimed a scope.
    ClaimAcquired {
        /// The claim id within the stream.
        claim: String,
        /// The holding session within the stream.
        session: String,
        /// Repository-relative paths.
        scope: Vec<String>,
        /// What the claim is for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        /// Exclusive or advisory.
        #[serde(default)]
        mode: ClaimMode,
        /// The issue the claim is for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issue: Option<String>,
    },
    /// A claim was released by its holder.
    ClaimReleased {
        /// The claim id within the stream.
        claim: String,
    },
    /// A handover was published for other runtimes to consume.
    HandoverPublished {
        /// The handover.
        handover: HandoverBody,
    },
    /// A session consumed a handover (on any runtime).
    HandoverConsumed {
        /// The handover's content digest.
        handover: String,
        /// The consuming session within the stream.
        session: String,
    },
    /// A session asked for a review.
    ReviewRequested {
        /// The review id within the stream.
        review: String,
        /// The requesting session within the stream.
        session: String,
        /// What is to be reviewed: a branch, a commit, a pull request.
        subject: String,
        /// Paths the review covers.
        #[serde(default)]
        scope: Vec<String>,
        /// The issue.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issue: Option<String>,
        /// The runtime asked (`<node>-<runtime>`), when one was.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reviewer: Option<String>,
    },
    /// A session answered a review request (on any runtime).
    ReviewAnswered {
        /// The request: `<stream>/<review>`.
        request: String,
        /// The answering session within the stream.
        session: String,
        /// `approved`, `changes_requested`, `commented`.
        verdict: String,
        /// The answer's note.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

/// The event kinds this executable interprets. An event naming another kind is stored and
/// relayed, never folded.
pub const KNOWN_KINDS: &[&str] = &[
    "session_opened",
    "session_closed",
    "claim_acquired",
    "claim_released",
    "handover_published",
    "handover_consumed",
    "review_requested",
    "review_answered",
];

impl EventBody {
    /// The kind's wire word.
    pub fn kind(&self) -> &'static str {
        match self {
            EventBody::SessionOpened { .. } => "session_opened",
            EventBody::SessionClosed { .. } => "session_closed",
            EventBody::ClaimAcquired { .. } => "claim_acquired",
            EventBody::ClaimReleased { .. } => "claim_released",
            EventBody::HandoverPublished { .. } => "handover_published",
            EventBody::HandoverConsumed { .. } => "handover_consumed",
            EventBody::ReviewRequested { .. } => "review_requested",
            EventBody::ReviewAnswered { .. } => "review_answered",
        }
    }

    /// Check the body's bounds: short identifiers, repository-relative scopes, a bounded
    /// handover. A body out of bounds is refused before it is signed or stored.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            EventBody::SessionOpened { info } => {
                ident("session", &info.session)?;
                text("client", &info.client, 64)?;
                for (name, value, max) in [
                    ("worker", &info.worker, 128),
                    ("intent", &info.intent, 512),
                    ("task", &info.task, 64),
                    ("issue", &info.issue, 64),
                    ("milestone", &info.milestone, 64),
                    ("checkout", &info.checkout, 64),
                    ("branch", &info.branch, 256),
                    ("head", &info.head, 64),
                    ("context", &info.context, 128),
                ] {
                    if let Some(v) = value {
                        text(name, v, max)?;
                    }
                }
                Ok(())
            }
            EventBody::SessionClosed { session } => ident("session", session),
            EventBody::ClaimAcquired {
                claim,
                session,
                scope,
                intent,
                issue,
                ..
            } => {
                ident("claim", claim)?;
                ident("session", session)?;
                paths(scope, true)?;
                if let Some(v) = intent {
                    text("intent", v, 512)?;
                }
                if let Some(v) = issue {
                    text("issue", v, 64)?;
                }
                Ok(())
            }
            EventBody::ClaimReleased { claim } => ident("claim", claim),
            EventBody::HandoverPublished { handover } => {
                if handover.id != HandoverBody::digest_of(&handover.body) {
                    return Err("the handover id is not the digest of its body".into());
                }
                if handover.body.len() > MAX_HANDOVER_BYTES {
                    return Err(format!(
                        "a handover body of {} bytes exceeds {MAX_HANDOVER_BYTES}",
                        handover.body.len()
                    ));
                }
                for (name, value, max) in [
                    ("task", &handover.task, 64),
                    ("issue", &handover.issue, 64),
                    ("milestone", &handover.milestone, 64),
                    ("branch", &handover.branch, 256),
                    ("head", &handover.head, 64),
                    ("created_at", &handover.created_at, 64),
                    ("name", &handover.name, 256),
                ] {
                    if let Some(v) = value {
                        text(name, v, max)?;
                    }
                }
                Ok(())
            }
            EventBody::HandoverConsumed { handover, session } => {
                ident("handover", handover)?;
                ident("session", session)
            }
            EventBody::ReviewRequested {
                review,
                session,
                subject,
                scope,
                issue,
                reviewer,
            } => {
                ident("review", review)?;
                ident("session", session)?;
                text("subject", subject, 256)?;
                paths(scope, false)?;
                if let Some(v) = issue {
                    text("issue", v, 64)?;
                }
                if let Some(v) = reviewer {
                    text("reviewer", v, 64)?;
                }
                Ok(())
            }
            EventBody::ReviewAnswered {
                request,
                session,
                verdict,
                note,
            } => {
                text("request", request, 128)?;
                ident("session", session)?;
                if !["approved", "changes_requested", "commented"].contains(&verdict.as_str()) {
                    return Err(format!("verdict '{verdict}' is not approved, changes_requested or commented"));
                }
                if let Some(v) = note {
                    text("note", v, 2048)?;
                }
                Ok(())
            }
        }
    }
}

fn ident(name: &str, value: &str) -> Result<(), String> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-#".contains(&b));
    if ok {
        Ok(())
    } else {
        Err(format!("{name} '{value}' is not an identifier (1-64 of [A-Za-z0-9._:#-])"))
    }
}

fn text(name: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max || value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("{name} is over {max} bytes or carries control characters"));
    }
    Ok(())
}

fn paths(scope: &[String], required: bool) -> Result<(), String> {
    if required && scope.is_empty() {
        return Err("a claim names at least one path".into());
    }
    if scope.len() > MAX_SCOPE_PATHS {
        return Err(format!("more than {MAX_SCOPE_PATHS} paths"));
    }
    for path in scope {
        if path.is_empty()
            || path.len() > 256
            || path.starts_with('/')
            || path.split('/').any(|s| s == "..")
            || path.chars().any(|c| c.is_control())
        {
            return Err(format!(
                "'{path}' is not a repository-relative path (no leading '/', no '..')"
            ));
        }
    }
    Ok(())
}

/// One event as the wire and the store carry it. The body is kept as JSON so that an
/// event of an unknown kind survives storage and relay byte-for-byte; [`MeshEvent::body`]
/// interprets it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshEvent {
    /// The event format version.
    pub v: u32,
    /// The stream that wrote it.
    pub stream: StreamId,
    /// Dense from 1 within the stream.
    pub seq: u64,
    /// The Lamport stamp: above every stamp its writer had seen.
    pub lamport: u64,
    /// The writer's wall clock, seconds since the Unix epoch. Informational only: nothing
    /// is decided by comparing it with another machine's clock.
    pub at: u64,
    /// The mesh repository id the event belongs to.
    pub repo: String,
    /// The writer's Ed25519 public key, hex; its digest is the stream's node.
    pub pk: String,
    /// The body, `kind` tagged.
    pub body: Value,
    /// Ed25519 over the canonical bytes of everything above, hex.
    pub sig: String,
}

/// The canonical bytes of a JSON value: object keys sorted at every depth, no whitespace.
/// Signer and verifier compute it identically regardless of how either parser orders keys.
///
/// ```
/// use majordomus_cli::mesh::journal::canonical_json;
/// let a = serde_json::json!({"b": 1, "a": [{"d": 2, "c": 3}]});
/// assert_eq!(canonical_json(&a), br#"{"a":[{"c":3,"d":2}],"b":1}"#.to_vec());
/// ```
pub fn canonical_json(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend(serde_json::to_vec(key).unwrap_or_default());
                out.push(b':');
                write_canonical(&map[*key], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        other => out.extend(serde_json::to_vec(other).unwrap_or_default()),
    }
}

impl MeshEvent {
    /// The bytes the signature covers.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let core = serde_json::json!({
            "v": self.v,
            "stream": self.stream,
            "seq": self.seq,
            "lamport": self.lamport,
            "at": self.at,
            "repo": self.repo,
            "pk": self.pk,
            "body": self.body,
        });
        let mut bytes = SIGNING_DOMAIN.to_vec();
        bytes.extend(canonical_json(&core));
        bytes
    }

    /// `<stream>/<seq>`: the event's identity everywhere.
    pub fn id(&self) -> String {
        format!("{}/{}", self.stream, self.seq)
    }

    /// The interpreted body; `None` for a kind this executable does not know.
    pub fn body(&self) -> Option<EventBody> {
        serde_json::from_value(self.body.clone()).ok()
    }

    /// The body's `kind` word, known or not.
    pub fn kind(&self) -> &str {
        self.body.get("kind").and_then(Value::as_str).unwrap_or("")
    }
}

/// Why an event was not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// Over [`MAX_EVENT_BYTES`].
    Oversized,
    /// An event format version this executable does not read.
    Version,
    /// The stream id, the key, or the key-to-node binding does not hold.
    Identity,
    /// The signature does not verify.
    Signature,
    /// Another repository's event.
    Repository,
    /// The origin is not trusted by this runtime's policy.
    Untrusted,
    /// A known kind whose body is out of bounds.
    Bounds,
    /// The journal's stream or pending bound is full.
    Capacity,
}

/// What one ingest did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IngestReport {
    /// Newly stored and applied, in stream order.
    pub accepted: u64,
    /// Already held: at-least-once delivery, absorbed.
    pub duplicate: u64,
    /// Held back until the gap before them arrives.
    pub pending: u64,
    /// Refused, by reason.
    pub rejected: BTreeMap<Rejection, u64>,
}

impl IngestReport {
    fn reject(&mut self, why: Rejection) {
        *self.rejected.entry(why).or_default() += 1;
    }

    /// Every refused event, summed over reasons.
    pub fn rejected_total(&self) -> u64 {
        self.rejected.values().sum()
    }
}

/// What a runtime knows of one stream: the contiguous high-water sequence, the highest
/// beat, and how long ago that beat was seen to rise (`None` when never).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamMark {
    /// Every event up to and including this sequence is held.
    pub seq: u64,
    /// The highest beat heard.
    pub beat: u64,
    /// Milliseconds since that beat rose, on the sender's monotonic clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_ms: Option<u64>,
}

/// Every stream's mark: what a sync request and answer carry.
pub type Marks = BTreeMap<StreamId, StreamMark>;

/// Whether a stream is speaking now, on this runtime's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// This runtime's own current stream.
    Own,
    /// Its beat rose within the expiry.
    Live,
    /// Its beat has not risen within the expiry, or was never heard.
    Expired,
}

impl Liveness {
    /// Whether the stream counts as alive: its claims hold, its sessions are present.
    pub fn is_alive(self) -> bool {
        !matches!(self, Liveness::Expired)
    }
}

/// The journal's counters since start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JournalTallies {
    /// Streams tracked.
    pub streams: usize,
    /// Events held.
    pub events: usize,
    /// Events written by this runtime since start.
    pub written: u64,
    /// Events received and stored.
    pub received: u64,
    /// Duplicate deliveries absorbed.
    pub duplicates: u64,
    /// Events refused.
    pub rejected: u64,
    /// Events held for a gap right now.
    pub pending: usize,
    /// Events dropped by compaction since start.
    pub compacted: u64,
    /// Events of a kind this executable does not interpret, held and relayed.
    pub opaque: usize,
    /// The highest Lamport stamp seen.
    pub lamport: u64,
}

#[derive(Default)]
struct StreamLog {
    events: BTreeMap<u64, MeshEvent>,
    pending: BTreeMap<u64, MeshEvent>,
    beat: u64,
    fresh_at: Option<Instant>,
}

impl StreamLog {
    fn high_water(&self) -> u64 {
        self.events.keys().next_back().copied().unwrap_or(0)
    }
}

struct Inner {
    streams: BTreeMap<StreamId, StreamLog>,
    lamport: u64,
    tallies: JournalTallies,
}

/// The journal of one runtime. `Mutex<BTreeMap>`: a handful of events a second, and a
/// deterministic order falls out for free.
pub struct Journal {
    identity: Arc<NodeIdentity>,
    own: StreamId,
    repo: String,
    path: Option<PathBuf>,
    inner: Mutex<Inner>,
}

impl Journal {
    /// Open the journal of this runtime: the own stream is `<node>-<runtime>-<instance>` of
    /// the identity given, and `path`, when given, is the JSONL file events persist to and
    /// are reloaded from. Reloaded events keep their streams; none of them is fresh until
    /// a beat is heard again, so a restart never resurrects anybody's ownership.
    pub fn open(
        identity: Arc<NodeIdentity>,
        runtime: &str,
        repo: String,
        path: Option<PathBuf>,
    ) -> Result<Self, MeshError> {
        let own = StreamId::new(
            identity.public.node_id.as_str(),
            runtime,
            identity.public.instance_id.as_str(),
        )
        .ok_or_else(|| MeshError::Protocol(format!("runtime '{runtime}' is not 16 hex characters")))?;
        let mut streams = BTreeMap::new();
        streams.insert(own.clone(), StreamLog::default());
        let journal = Journal {
            identity,
            own,
            repo,
            path,
            inner: Mutex::new(Inner {
                streams,
                lamport: 0,
                tallies: JournalTallies::default(),
            }),
        };
        if let Some(path) = journal.path.clone() {
            journal.reload(&path);
        }
        Ok(journal)
    }

    fn reload(&self, path: &Path) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let events: Vec<MeshEvent> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        // Reloaded events pass the same checks as received ones: a file is input too.
        let mut inner = self.inner.lock().expect("journal lock");
        let mut report = IngestReport::default();
        for event in events {
            self.ingest_locked(&mut inner, event, &|_| Ok(()), &mut report, false);
        }
        inner.tallies.received = 0;
        inner.tallies.duplicates = 0;
    }

    /// This runtime's own stream.
    pub fn own_stream(&self) -> &StreamId {
        &self.own
    }

    /// The mesh repository id this journal belongs to.
    pub fn repository(&self) -> &str {
        &self.repo
    }

    /// Sign, store and persist an event of this runtime. The body's bounds are checked
    /// first: nothing out of bounds is ever signed.
    pub fn append_own(&self, body: EventBody) -> Result<MeshEvent, MeshError> {
        body.validate().map_err(MeshError::Protocol)?;
        let body = serde_json::to_value(&body).map_err(|e| MeshError::Protocol(e.to_string()))?;
        let mut inner = self.inner.lock().expect("journal lock");
        let log = inner.streams.entry(self.own.clone()).or_default();
        let seq = log.high_water() + 1;
        let lamport = inner.lamport + 1;
        let mut event = MeshEvent {
            v: EVENT_VERSION,
            stream: self.own.clone(),
            seq,
            lamport,
            at: super::protocol::now(),
            repo: self.repo.clone(),
            pk: self.identity.public.public_key.clone(),
            body,
            sig: String::new(),
        };
        event.sig = self.identity.sign(&event.signing_bytes());
        let size = serde_json::to_vec(&event).map(|b| b.len()).unwrap_or(usize::MAX);
        if size > MAX_EVENT_BYTES {
            return Err(MeshError::Protocol(format!(
                "an event of {size} bytes exceeds the {MAX_EVENT_BYTES}-byte bound"
            )));
        }
        inner.lamport = lamport;
        inner
            .streams
            .get_mut(&self.own)
            .expect("own stream")
            .events
            .insert(seq, event.clone());
        inner.tallies.written += 1;
        self.persist(&[event.clone()]);
        Ok(event)
    }

    /// Raise this runtime's own beat: one heartbeat.
    pub fn beat_own(&self) {
        let mut inner = self.inner.lock().expect("journal lock");
        let log = inner.streams.entry(self.own.clone()).or_default();
        log.beat += 1;
        log.fresh_at = Some(Instant::now());
    }

    /// Every stream's mark, own included, ages measured now.
    pub fn marks(&self) -> Marks {
        let now = Instant::now();
        let inner = self.inner.lock().expect("journal lock");
        inner
            .streams
            .iter()
            .map(|(id, log)| {
                (
                    id.clone(),
                    StreamMark {
                        seq: log.high_water(),
                        beat: log.beat,
                        age_ms: log
                            .fresh_at
                            .map(|t| now.saturating_duration_since(t).as_millis() as u64),
                    },
                )
            })
            .collect()
    }

    /// Merge a peer's marks: a higher beat, or the same beat seen more recently, makes the
    /// stream fresher here. Sequences are not merged — only events move high-water marks.
    pub fn merge_marks(&self, marks: &Marks) {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("journal lock");
        for (id, mark) in marks {
            if *id == self.own {
                continue;
            }
            if !inner.streams.contains_key(id) && inner.streams.len() >= MAX_STREAMS {
                continue;
            }
            let log = inner.streams.entry(id.clone()).or_default();
            let seen_at = mark
                .age_ms
                .and_then(|age| now.checked_sub(Duration::from_millis(age)));
            if mark.beat > log.beat {
                log.beat = mark.beat;
                log.fresh_at = seen_at;
            } else if mark.beat == log.beat && mark.beat > 0 {
                if let Some(seen) = seen_at {
                    if log.fresh_at.is_none_or(|t| seen > t) {
                        log.fresh_at = Some(seen);
                    }
                }
            }
        }
    }

    /// The events a peer lacks by its marks, in stream-then-sequence order, at most
    /// `budget` serialized bytes (always at least one event when any is missing).
    pub fn missing_for(&self, peer: &Marks, budget: usize) -> Vec<MeshEvent> {
        let inner = self.inner.lock().expect("journal lock");
        let mut out = Vec::new();
        let mut spent = 0usize;
        for (id, log) in &inner.streams {
            let have = peer.get(id).map(|m| m.seq).unwrap_or(0);
            for (_, event) in log.events.range(have + 1..) {
                let size = serde_json::to_vec(event).map(|b| b.len()).unwrap_or(0);
                if !out.is_empty() && spent + size > budget {
                    return out;
                }
                spent += size;
                out.push(event.clone());
            }
        }
        out
    }

    /// Verify and store received events. `accept` is the trust decision for an event's
    /// origin, made above the journal; everything else — size, version, identity,
    /// signature, repository, bounds, order, duplicates — is decided here, once, for every
    /// transport and relay alike.
    pub fn ingest(
        &self,
        events: &[MeshEvent],
        accept: &dyn Fn(&MeshEvent) -> Result<(), Rejection>,
    ) -> IngestReport {
        let mut report = IngestReport::default();
        let mut stored = Vec::new();
        {
            let mut inner = self.inner.lock().expect("journal lock");
            for event in events {
                let before = inner.tallies.received;
                if let Some(applied) =
                    self.ingest_locked(&mut inner, event.clone(), accept, &mut report, true)
                {
                    stored.extend(applied);
                }
                debug_assert!(inner.tallies.received >= before);
            }
        }
        self.persist(&stored);
        report
    }

    /// One event under the lock. Returns the events it made contiguous (itself and any
    /// pending successors), which the caller persists.
    fn ingest_locked(
        &self,
        inner: &mut Inner,
        event: MeshEvent,
        accept: &dyn Fn(&MeshEvent) -> Result<(), Rejection>,
        report: &mut IngestReport,
        count: bool,
    ) -> Option<Vec<MeshEvent>> {
        let refuse = |inner: &mut Inner, report: &mut IngestReport, why: Rejection| {
            report.reject(why);
            if count {
                inner.tallies.rejected += 1;
            }
            None
        };
        if serde_json::to_vec(&event).map(|b| b.len()).unwrap_or(usize::MAX) > MAX_EVENT_BYTES {
            return refuse(inner, report, Rejection::Oversized);
        }
        if event.v != EVENT_VERSION {
            return refuse(inner, report, Rejection::Version);
        }
        if StreamId::parse(event.stream.as_str()).is_none()
            || node_id_of_key(&event.pk).map(|n| n.as_str().to_string())
                != Some(event.stream.node().to_string())
            || event.seq == 0
        {
            return refuse(inner, report, Rejection::Identity);
        }
        if !verify(&event.pk, &event.signing_bytes(), &event.sig) {
            return refuse(inner, report, Rejection::Signature);
        }
        if event.repo != self.repo {
            return refuse(inner, report, Rejection::Repository);
        }
        if let Some(body) = event.body() {
            if body.validate().is_err() {
                return refuse(inner, report, Rejection::Bounds);
            }
        } else if KNOWN_KINDS.contains(&event.kind()) || event.kind().is_empty() {
            // A kind this executable knows, in a shape it does not: out of bounds, not new.
            return refuse(inner, report, Rejection::Bounds);
        }
        if let Err(why) = accept(&event) {
            return refuse(inner, report, why);
        }
        if event.stream == self.own {
            // Our own events coming back through a relay: we hold them already, or they
            // are from a future we did not write — either way, nothing to store.
            report.duplicate += 1;
            if count {
                inner.tallies.duplicates += 1;
            }
            return None;
        }
        if !inner.streams.contains_key(&event.stream) && inner.streams.len() >= MAX_STREAMS {
            return refuse(inner, report, Rejection::Capacity);
        }
        let lamport = event.lamport;
        let log = inner.streams.entry(event.stream.clone()).or_default();
        let high = log.high_water();
        if event.seq <= high || log.pending.contains_key(&event.seq) {
            report.duplicate += 1;
            if count {
                inner.tallies.duplicates += 1;
            }
            return None;
        }
        if event.seq > high + 1 {
            if log.pending.len() >= MAX_PENDING {
                return refuse(inner, report, Rejection::Capacity);
            }
            log.pending.insert(event.seq, event);
            report.pending += 1;
            return None;
        }
        let mut applied = vec![event.clone()];
        log.events.insert(event.seq, event);
        let mut next = high + 2;
        while let Some(successor) = log.pending.remove(&next) {
            applied.push(successor.clone());
            log.events.insert(next, successor);
            next += 1;
        }
        let max_lamport = applied.iter().map(|e| e.lamport).max().unwrap_or(lamport);
        inner.lamport = inner.lamport.max(max_lamport);
        report.accepted += applied.len() as u64;
        if count {
            inner.tallies.received += applied.len() as u64;
        }
        Some(applied)
    }

    fn persist(&self, events: &[MeshEvent]) {
        let Some(path) = &self.path else { return };
        if events.is_empty() {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
            tracing::warn!(path = %path.display(), "mesh journal: cannot open for append");
            return;
        };
        let mut text = String::new();
        for event in events {
            if let Ok(line) = serde_json::to_string(event) {
                text.push_str(&line);
                text.push('\n');
            }
        }
        let _ = file.write_all(text.as_bytes());
    }

    /// A stream's liveness on this runtime's clock, against `expiry`.
    pub fn liveness(&self, stream: &StreamId, expiry: Duration) -> Liveness {
        if *stream == self.own {
            return Liveness::Own;
        }
        let now = Instant::now();
        let inner = self.inner.lock().expect("journal lock");
        match inner.streams.get(stream).and_then(|l| l.fresh_at) {
            Some(t) if now.saturating_duration_since(t) <= expiry => Liveness::Live,
            _ => Liveness::Expired,
        }
    }

    /// Every stream's liveness and time since its beat rose, in stream order.
    pub fn stream_liveness(&self, expiry: Duration) -> BTreeMap<StreamId, (Liveness, Option<Duration>)> {
        let now = Instant::now();
        let inner = self.inner.lock().expect("journal lock");
        inner
            .streams
            .iter()
            .map(|(id, log)| {
                let age = log.fresh_at.map(|t| now.saturating_duration_since(t));
                let liveness = if *id == self.own {
                    Liveness::Own
                } else if age.is_some_and(|a| a <= expiry) {
                    Liveness::Live
                } else {
                    Liveness::Expired
                };
                (id.clone(), (liveness, age))
            })
            .collect()
    }

    /// Every held event, in stream-then-sequence order.
    pub fn events(&self) -> Vec<MeshEvent> {
        let inner = self.inner.lock().expect("journal lock");
        inner
            .streams
            .values()
            .flat_map(|log| log.events.values().cloned())
            .collect()
    }

    /// Held events whose Lamport stamp is above `after`, in Lamport order, at most `limit`.
    pub fn events_after(&self, after: u64, limit: usize) -> Vec<MeshEvent> {
        let mut events: Vec<MeshEvent> = self
            .events()
            .into_iter()
            .filter(|e| e.lamport > after)
            .collect();
        events.sort_by(|a, b| (a.lamport, &a.stream, a.seq).cmp(&(b.lamport, &b.stream, b.seq)));
        events.truncate(limit);
        events
    }

    /// Drop every stream that has been expired for longer than `retention` and holds no
    /// handover, when the journal is over its bound or `force` is set; the file is
    /// rewritten to what remains. Only dead streams go, so no live claim can lose its
    /// release to compaction.
    pub fn compact(&self, expiry: Duration, retention: Duration, force: bool) -> u64 {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("journal lock");
        let total: usize = inner.streams.values().map(|l| l.events.len()).sum();
        if !force && total <= MAX_EVENTS {
            return 0;
        }
        let dead: Vec<StreamId> = inner
            .streams
            .iter()
            .filter(|(id, log)| {
                **id != self.own
                    && log
                        .fresh_at
                        .is_none_or(|t| now.saturating_duration_since(t) > expiry + retention)
                    && !log.events.values().any(|e| e.kind() == "handover_published")
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut dropped = 0u64;
        for id in dead {
            if let Some(log) = inner.streams.remove(&id) {
                dropped += log.events.len() as u64;
            }
        }
        inner.tallies.compacted += dropped;
        if dropped > 0 {
            if let Some(path) = &self.path {
                let mut text = String::new();
                for event in inner.streams.values().flat_map(|l| l.events.values()) {
                    if let Ok(line) = serde_json::to_string(event) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                let tmp = path.with_extension("jsonl.tmp");
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(&tmp, path);
                }
            }
        }
        dropped
    }

    /// The tallies now.
    pub fn tallies(&self) -> JournalTallies {
        let inner = self.inner.lock().expect("journal lock");
        let mut tallies = inner.tallies;
        tallies.streams = inner.streams.len();
        tallies.events = inner.streams.values().map(|l| l.events.len()).sum();
        tallies.pending = inner.streams.values().map(|l| l.pending.len()).sum();
        tallies.opaque = inner
            .streams
            .values()
            .flat_map(|l| l.events.values())
            .filter(|e| e.body().is_none())
            .count();
        tallies.lamport = inner.lamport;
        tallies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal(runtime: &str) -> Journal {
        Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            runtime,
            "repo".into(),
            None,
        )
        .unwrap()
    }

    fn opened(j: &Journal, session: &str) -> MeshEvent {
        j.append_own(EventBody::SessionOpened {
            info: SessionInfo::named(session, "test"),
        })
        .unwrap()
    }

    fn accept_all(_: &MeshEvent) -> Result<(), Rejection> {
        Ok(())
    }

    #[test]
    fn own_events_are_dense_signed_and_lamport_stamped() {
        let j = journal("0000000000000001");
        let a = opened(&j, "s1");
        let b = opened(&j, "s2");
        assert_eq!((a.seq, b.seq), (1, 2));
        assert!(b.lamport > a.lamport);
        assert!(verify(&a.pk, &a.signing_bytes(), &a.sig));
        assert_eq!(j.marks()[j.own_stream()].seq, 2);
    }

    #[test]
    fn replication_sends_only_what_the_peer_lacks() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        opened(&a, "s1");
        opened(&a, "s2");
        let first = a.missing_for(&b.marks(), usize::MAX);
        assert_eq!(first.len(), 2);
        assert_eq!(b.ingest(&first, &accept_all).accepted, 2);
        assert!(
            a.missing_for(&b.marks(), usize::MAX).is_empty(),
            "nothing left to send: marks converge, so replication stops"
        );
    }

    #[test]
    fn a_relay_carries_events_it_cannot_forge() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let c = journal("0000000000000003");
        opened(&a, "s1");
        b.ingest(&a.missing_for(&b.marks(), usize::MAX), &accept_all);
        // B relays A's event to C: C verifies A's signature, not B's word.
        let relayed = b.missing_for(&c.marks(), usize::MAX);
        assert_eq!(relayed.len(), 1);
        assert_eq!(relayed[0].stream, *a.own_stream());
        assert_eq!(c.ingest(&relayed, &accept_all).accepted, 1);
        // A tampering relay is caught at the consumer.
        let mut forged = relayed[0].clone();
        forged.body = serde_json::json!({"kind": "session_closed", "session": "s1"});
        let d = journal("0000000000000004");
        let report = d.ingest(&[forged], &accept_all);
        assert_eq!(report.rejected.get(&Rejection::Signature), Some(&1));
    }

    #[test]
    fn out_of_order_delivery_waits_for_the_gap() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let e1 = opened(&a, "s1");
        let e2 = opened(&a, "s2");
        let e3 = opened(&a, "s3");
        let report = b.ingest(&[e3.clone(), e2.clone()], &accept_all);
        assert_eq!((report.accepted, report.pending), (0, 2));
        assert_eq!(b.marks()[a.own_stream()].seq, 0, "a gap holds the mark");
        let report = b.ingest(&[e1], &accept_all);
        assert_eq!(report.accepted, 3, "the gap closes and its successors apply");
        assert_eq!(b.marks()[a.own_stream()].seq, 3);
    }

    #[test]
    fn hostile_events_are_refused_by_reason() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let good = opened(&a, "s1");

        let mut other_repo = good.clone();
        other_repo.repo = "elsewhere".into();
        other_repo.sig = String::new();
        let wrong_version = MeshEvent { v: 2, ..good.clone() };
        let mut stolen_stream = good.clone();
        stolen_stream.stream = b.own_stream().clone();
        let mut huge = good.clone();
        huge.body = serde_json::json!({"kind": "future", "blob": "x".repeat(MAX_EVENT_BYTES)});

        let r = b.ingest(&[other_repo, wrong_version, stolen_stream, huge], &accept_all);
        assert_eq!(r.rejected.get(&Rejection::Signature), Some(&1));
        assert_eq!(r.rejected.get(&Rejection::Version), Some(&1));
        assert_eq!(r.rejected.get(&Rejection::Identity), Some(&1));
        assert_eq!(r.rejected.get(&Rejection::Oversized), Some(&1));
        let untrusted = b.ingest(&[good], &|_| Err(Rejection::Untrusted));
        assert_eq!(untrusted.rejected.get(&Rejection::Untrusted), Some(&1));
        assert!(b.events().is_empty());
    }

    #[test]
    fn a_repository_mismatch_is_refused_even_when_signed() {
        let foreign = Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            "0000000000000001",
            "another-repository".into(),
            None,
        )
        .unwrap();
        let e = opened(&foreign, "s1");
        let ours = journal("0000000000000002");
        let r = ours.ingest(&[e], &accept_all);
        assert_eq!(r.rejected.get(&Rejection::Repository), Some(&1));
    }

    #[test]
    fn an_unknown_kind_is_held_and_relayed_but_a_bad_known_kind_is_refused() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        // A future executable's event, signed properly.
        let mut future = opened(&a, "s1");
        future.body = serde_json::json!({"kind": "vote_cast", "ballot": 7});
        future.seq = 1;
        future.sig = a.identity.sign(&future.signing_bytes());
        assert_eq!(b.ingest(&[future], &accept_all).accepted, 1);
        assert_eq!(b.tallies().opaque, 1);

        let mut bad = opened(&a, "s2");
        bad.body = serde_json::json!({"kind": "claim_acquired", "claim": "c", "session": "s", "scope": ["/etc/passwd"]});
        bad.sig = a.identity.sign(&bad.signing_bytes());
        let r = b.ingest(&[bad], &accept_all);
        assert_eq!(r.rejected.get(&Rejection::Bounds), Some(&1));
    }

    #[test]
    fn bounds_are_checked_before_signing() {
        let j = journal("0000000000000001");
        let err = j.append_own(EventBody::ClaimAcquired {
            claim: "c1".into(),
            session: "s1".into(),
            scope: vec!["../outside".into()],
            intent: None,
            mode: ClaimMode::Exclusive,
            issue: None,
        });
        assert!(err.is_err());
        assert!(j.events().is_empty());
    }

    #[test]
    fn beats_relay_freshness_without_comparing_clocks() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let c = journal("0000000000000003");
        let expiry = Duration::from_secs(30);
        assert_eq!(b.liveness(a.own_stream(), expiry), Liveness::Expired, "never heard");
        a.beat_own();
        b.merge_marks(&a.marks());
        assert_eq!(b.liveness(a.own_stream(), expiry), Liveness::Live);
        // C hears A only through B.
        c.merge_marks(&b.marks());
        assert_eq!(c.liveness(a.own_stream(), expiry), Liveness::Live);
        // An old beat relayed with its age does not make a dead stream fresh.
        let d = journal("0000000000000004");
        let mut stale = b.marks();
        stale.get_mut(a.own_stream()).unwrap().age_ms = Some(60_000);
        d.merge_marks(&stale);
        assert_eq!(d.liveness(a.own_stream(), expiry), Liveness::Expired);
        assert_eq!(d.liveness(d.own_stream(), expiry), Liveness::Own);
    }

    #[test]
    fn a_reloaded_journal_keeps_events_and_resurrects_no_liveness() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let key = dir.path().join("node.json");
        let a = journal("0000000000000001");
        a.beat_own();
        opened(&a, "s1");
        let b = Journal::open(
            Arc::new(NodeIdentity::load_or_create(&key).unwrap()),
            "0000000000000002",
            "repo".into(),
            Some(path.clone()),
        )
        .unwrap();
        b.merge_marks(&a.marks());
        b.ingest(&a.missing_for(&b.marks(), usize::MAX), &accept_all);
        let own_before = b.own_stream().clone();
        b.append_own(EventBody::SessionOpened { info: SessionInfo::named("mine", "test") })
            .unwrap();
        drop(b);

        // The same node restarts: a new instance, so a new stream; the old events reload.
        let restarted = Journal::open(
            Arc::new(NodeIdentity::load_or_create(&key).unwrap()),
            "0000000000000002",
            "repo".into(),
            Some(path),
        )
        .unwrap();
        assert_ne!(restarted.own_stream(), &own_before);
        assert_eq!(restarted.events().len(), 2);
        let expiry = Duration::from_secs(30);
        assert_eq!(restarted.liveness(a.own_stream(), expiry), Liveness::Expired);
        assert_eq!(
            restarted.liveness(&own_before, expiry),
            Liveness::Expired,
            "the previous run of this very runtime is dead, not resurrected"
        );
    }

    #[test]
    fn compaction_drops_only_dead_streams_without_handovers() {
        let a = journal("0000000000000001");
        let h = journal("0000000000000003");
        let b = journal("0000000000000002");
        opened(&a, "s1");
        let body = "# Objective\nx\n".to_string();
        h.append_own(EventBody::HandoverPublished {
            handover: HandoverBody {
                id: HandoverBody::digest_of(&body),
                task: None,
                issue: None,
                milestone: None,
                branch: None,
                head: None,
                created_at: None,
                name: None,
                body,
            },
        })
        .unwrap();
        b.ingest(&a.missing_for(&b.marks(), usize::MAX), &accept_all);
        b.ingest(&h.missing_for(&b.marks(), usize::MAX), &accept_all);
        let dropped = b.compact(Duration::ZERO, Duration::ZERO, true);
        assert_eq!(dropped, 1, "A's session event goes; the handover stays");
        assert_eq!(b.events().len(), 1);
    }

    proptest::proptest! {
        /// Any delivery order and any duplication of one stream's events converges on the
        /// same held set as in-order delivery.
        #[test]
        fn any_permutation_with_duplicates_converges(order in proptest::collection::vec(0usize..6, 1..24)) {
            let a = journal("0000000000000001");
            let events: Vec<MeshEvent> = (0..6).map(|i| opened(&a, &format!("s{i}"))).collect();
            let b = journal("0000000000000002");
            let shuffled: Vec<MeshEvent> = order.iter().map(|i| events[*i].clone()).collect();
            b.ingest(&shuffled, &accept_all);
            b.ingest(&events, &accept_all);
            proptest::prop_assert_eq!(b.events(), events.clone());
            proptest::prop_assert_eq!(b.tallies().pending, 0);
        }

        /// Arbitrary JSON posing as an event never panics the ingest path.
        #[test]
        fn hostile_json_never_panics(seq in 0u64..5, lamport in proptest::prelude::any::<u64>(), kind in "[a-z_]{0,20}", sig in "[0-9a-f]{0,130}") {
            let b = journal("0000000000000002");
            let event = MeshEvent {
                v: 1,
                stream: StreamId(format!("{}-{}-{}", "a".repeat(32), "b".repeat(16), "c".repeat(16))),
                seq,
                lamport,
                at: 0,
                repo: "repo".into(),
                pk: "00".repeat(32),
                body: serde_json::json!({"kind": kind}),
                sig,
            };
            let r = b.ingest(&[event], &accept_all);
            proptest::prop_assert_eq!(r.accepted, 0);
        }
    }
}
