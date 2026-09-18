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
//! - **StreamLiveness without shared clocks.** Each stream carries a beat its origin raises on
//!   every heartbeat; a mark relays the beat with how long ago the sender saw it rise.
//!   A runtime computes freshness on its own monotonic clock — no two machines' wall
//!   clocks are ever compared.
//!
//! What it does not promise: exactly-once delivery, a global order across streams beyond
//! the Lamport stamp, or consistency under partition. Two sides of a partition may both
//! act; the fold makes the result deterministic and names the conflict (ADR 0067).
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

/// The most streams one node may occupy: runtime and instance ids are free to invent, so a
/// single trusted key must not be able to fill the stream table.
pub const MAX_STREAMS_PER_NODE: usize = 64;

/// The most events (held and pending) one node may occupy.
pub const MAX_EVENTS_PER_NODE: usize = 20_000;

/// The most out-of-order events the whole journal holds at once.
pub const MAX_PENDING_TOTAL: usize = 4096;

/// The domain separator of an event signature: a signature over an event can never be
/// replayed as a signature over an advertisement or a link message.
const SIGNING_DOMAIN: &[u8] = b"majordomus-mesh-event/v1\n";

/// One stream: a run of one runtime of one node. `<node 32 hex>-<runtime 16 hex>-<instance
/// 16 hex>`. The node is the machine's key, the runtime is one checkout's server on that
/// machine, the instance is one process run of that server — so two worktrees on one
/// machine are two streams, and a restart is a new stream of the same runtime.
///
/// Numbering events within a stream rather than globally is what makes replication cheap
/// and unambiguous: a gap in one stream is visible without coordinating with anyone, and
/// `(stream, seq)` identifies an event on every machine that holds it.
///
/// ```
/// use majordomus_cli::mesh::journal::StreamId;
///
/// let id = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap();
/// assert_eq!(id.node(), "a".repeat(32));
/// assert_eq!(id.runtime(), "b".repeat(16));
///
/// // a restart is a new stream, and the same runtime
/// let after = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"d".repeat(16)).unwrap();
/// assert_ne!(after, id);
/// assert_eq!(after.runtime_key(), id.runtime_key());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct StreamId(String);

/// A stream id off the wire is parsed, never trusted: every accessor slices at fixed
/// widths, so an unparsed id would be a panic waiting in a peer's marks.
impl TryFrom<String> for StreamId {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        StreamId::parse(&text).ok_or_else(|| format!("'{text}' is not a stream id"))
    }
}

impl From<StreamId> for String {
    fn from(id: StreamId) -> String {
        id.0
    }
}

impl StreamId {
    /// A stream id from its three parts; `None` unless each is lowercase hex of its width.
    ///
    /// The widths are checked here rather than trusted, because every accessor slices the
    /// text at fixed offsets: an id built from a part of the wrong length would be a panic
    /// waiting in a peer's marks, and the parts often come from another machine.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    ///
    /// assert!(StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).is_some());
    /// assert!(StreamId::new("short", &"b".repeat(16), &"c".repeat(16)).is_none());
    /// assert!(StreamId::new(&"A".repeat(32), &"b".repeat(16), &"c".repeat(16)).is_none(),
    ///     "one spelling only: lowercase hex");
    /// ```
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
            && parts.iter().all(|p| {
                p.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            });
        ok.then(|| StreamId(text.to_string()))
    }

    /// The node (machine key digest) part: which machine's key signs everything this
    /// stream carries. Two streams that share a node are two servers of one machine, and
    /// the per-node bounds of the journal are counted against this.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    ///
    /// let id = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap();
    /// let sibling = StreamId::new(&"a".repeat(32), &"e".repeat(16), &"f".repeat(16)).unwrap();
    /// assert_eq!(id.node(), sibling.node(), "two runtimes of one machine");
    /// assert_ne!(id.runtime_key(), sibling.runtime_key());
    /// ```
    pub fn node(&self) -> &str {
        &self.0[..32]
    }

    /// The runtime part: which server of that machine — one per checkout — so two
    /// worktrees of one clone are told apart and can claim against each other.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    ///
    /// let id = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap();
    /// assert_eq!(id.runtime(), "b".repeat(16));
    /// assert_eq!(id.runtime_key(), format!("{}-{}", id.node(), id.runtime()));
    /// ```
    pub fn runtime(&self) -> &str {
        &self.0[33..49]
    }

    /// The instance part: which run of that server. It is what makes a restart visible —
    /// the same runtime with a new instance has a fresh sequence, so a peer knows to say
    /// hello again rather than resume a link whose counters no longer mean anything.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    ///
    /// let before = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap();
    /// let after = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"d".repeat(16)).unwrap();
    /// assert_ne!(before.instance(), after.instance(), "a restart is a new run");
    /// assert_eq!(before.runtime(), after.runtime(), "of the same server");
    /// ```
    pub fn instance(&self) -> &str {
        &self.0[50..66]
    }

    /// `<node>-<runtime>`: the durable identity of a runtime across its restarts. It is
    /// what the peer table, a claim key and `mesh peer` are all keyed by, because a worker
    /// cares which server it is talking to and not which run of it.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamId;
    ///
    /// let before = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap();
    /// let after = StreamId::new(&"a".repeat(32), &"b".repeat(16), &"d".repeat(16)).unwrap();
    /// assert_eq!(before.runtime_key(), after.runtime_key(), "one peer, two runs");
    /// assert_eq!(before.runtime_key().len(), 49);
    /// ```
    pub fn runtime_key(&self) -> String {
        self.0[..49].to_string()
    }

    /// The id as text, in the one spelling every surface carries: a map key, a mark, a
    /// claim's prefix and a log line all print this, so an id read anywhere can be compared
    /// with an id read anywhere else.
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
///
/// Almost everything here is optional, and that is the design: a session says what it
/// knows, and a field it leaves out is omitted rather than carried empty, so a reader can
/// tell "no task" from "a task called nothing". Opening the same session id again replaces
/// this record, which is how a session that learns its branch later tells the mesh.
///
/// ```
/// use majordomus_cli::mesh::journal::SessionInfo;
///
/// let mut info = SessionInfo::named("s1", "claude-code");
/// assert_eq!(info.task, None);
/// assert!(serde_json::to_value(&info).unwrap().get("task").is_none(), "unsaid, not empty");
///
/// info.branch = Some("feature/mesh-cooperation".into());
/// assert_eq!(serde_json::to_value(&info).unwrap()["branch"], "feature/mesh-cooperation");
/// ```
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
    /// A session with only an id and a client: what a test or an example needs. The two
    /// arguments are the two facts a session cannot be without — something to refer to it
    /// by, and what kind of worker it is — and everything else is filled in afterwards by
    /// whoever knows it.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::SessionInfo;
    ///
    /// let info = SessionInfo::named("s1", "codex");
    /// assert_eq!((info.session.as_str(), info.client.as_str()), ("s1", "codex"));
    /// assert_eq!(info.worker, None, "the rest is the session's to say later");
    /// ```
    pub fn named(session: &str, client: &str) -> Self {
        SessionInfo {
            session: session.into(),
            client: client.into(),
            ..SessionInfo::default()
        }
    }
}

/// Whether a claim excludes others or only tells them. Both are useful and they answer
/// different questions: an exclusive claim is a worker saying "do not edit this while I
/// am", an advisory one is saying "I am here". Exclusive is the default because a claim
/// taken without a thought about the mode is a claim somebody expects to be respected.
///
/// ```
/// use majordomus_cli::mesh::journal::ClaimMode;
///
/// assert_eq!(ClaimMode::default(), ClaimMode::Exclusive);
/// assert_eq!(serde_json::to_value(ClaimMode::Advisory).unwrap(), serde_json::json!("advisory"));
/// ```
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
///
/// What travels is the writing, plus the few facts that let a receiver file it — the task,
/// the issue, the branch, the commit. No path of the author's disk is among them, because
/// the checkout that reads this is somewhere else and a path from another machine is at
/// best noise. Identity is the digest of the body, so a handover published twice by a
/// retry or by two runtimes is one handover everywhere.
///
/// ```
/// use majordomus_cli::mesh::journal::HandoverBody;
///
/// let body = "# Objective\nship the mesh\n".to_string();
/// let handover = HandoverBody {
///     id: HandoverBody::digest_of(&body),
///     task: Some("t-1".into()),
///     issue: Some("#184".into()),
///     milestone: None,
///     branch: Some("feature/mesh-cooperation".into()),
///     head: None,
///     created_at: None,
///     name: None,
///     body,
/// };
/// assert_eq!(handover.id, HandoverBody::digest_of(&handover.body));
///
/// let wire = serde_json::to_value(&handover).unwrap();
/// assert!(wire.get("milestone").is_none(), "what it does not say is omitted");
/// ```
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
    /// The content digest a handover body is identified by. Identifying a handover by what
    /// it says rather than by who published it is what makes publication idempotent across
    /// the mesh: a retry, a relay and a second publisher all produce the same id, and the
    /// fold keeps one handover with one list of consumers.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::HandoverBody;
    ///
    /// let body = "# Objective\nship\n";
    /// assert_eq!(HandoverBody::digest_of(body).len(), 32);
    /// assert_eq!(HandoverBody::digest_of(body), HandoverBody::digest_of(body));
    /// assert_ne!(HandoverBody::digest_of(body), HandoverBody::digest_of("# Objective\nwait\n"));
    /// ```
    pub fn digest_of(body: &str) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(body.as_bytes());
        digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }
}

/// What an event says. One vocabulary for every runtime; an event of a kind this
/// executable does not know is stored and relayed but not interpreted, so a newer peer
/// never breaks an older one's replication.
///
/// The vocabulary is small on purpose: everything the mesh coordinates is a session, a
/// claim, a handover or a review, and an event that would need a ninth kind is a feature
/// that has not been agreed. The `kind` tag is what goes on the wire, so it is protocol —
/// two executables of different ages read the same word for the same event.
///
/// ```
/// use majordomus_cli::mesh::journal::{EventBody, SessionInfo, KNOWN_KINDS};
///
/// let opened = EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") };
/// assert_eq!(opened.kind(), "session_opened");
/// assert!(KNOWN_KINDS.contains(&opened.kind()));
/// assert_eq!(serde_json::to_value(&opened).unwrap()["kind"], "session_opened");
/// ```
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
    /// The kind's wire word: the same snake-case spelling the serialized event carries, so
    /// a counter, a log line and a peer's JSON all name an event the same way. It is
    /// written out rather than derived from the variant name, because the word is protocol
    /// and renaming a variant must not change what another runtime reads.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::{EventBody, KNOWN_KINDS};
    ///
    /// let released = EventBody::ClaimReleased { claim: "c1".into() };
    /// assert_eq!(released.kind(), "claim_released");
    /// assert_eq!(serde_json::to_value(&released).unwrap()["kind"], released.kind());
    /// assert!(KNOWN_KINDS.contains(&released.kind()));
    /// ```
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
    ///
    /// It runs on both sides: on the writer, so a runtime never signs something its peers
    /// will refuse, and on the reader, so nothing a peer sends is stored unchecked. The
    /// bounds are not arbitrary — a scope must be repository-relative because a claim over
    /// `/etc` means nothing on another machine, a handover's id must be the digest of its
    /// body or identity would be forgeable, and the fields that become front matter where
    /// a handover is consumed must be single lines.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::{EventBody, HandoverBody, SessionInfo};
    ///
    /// assert!(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }.validate().is_ok());
    ///
    /// // a scope that is not repository-relative is refused
    /// let escaping = EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
    ///     scope: vec!["../../etc".into()], intent: None, mode: Default::default(), issue: None };
    /// assert!(escaping.validate().is_err());
    ///
    /// // and a handover whose id is not the digest of its body is not that handover
    /// let body = "# Objective\nship\n".to_string();
    /// let lying = EventBody::HandoverPublished { handover: HandoverBody {
    ///     id: HandoverBody::digest_of("something else"), task: None, issue: None,
    ///     milestone: None, branch: None, head: None, created_at: None, name: None, body } };
    /// assert!(lying.validate().is_err());
    /// ```
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
                        line(name, v, max)?;
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
                    // These become front matter and a file name wherever the handover is
                    // consumed: one line each, never a place to smuggle a key or a path.
                    if let Some(v) = value {
                        line(name, v, max)?;
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
                    return Err(format!(
                        "verdict '{verdict}' is not approved, changes_requested or commented"
                    ));
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
        Err(format!(
            "{name} '{value}' is not an identifier (1-64 of [A-Za-z0-9._:#-])"
        ))
    }
}

fn text(name: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max || value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!(
            "{name} is over {max} bytes or carries control characters"
        ));
    }
    Ok(())
}

fn line(name: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} is over {max} bytes or is not a single line"
        ));
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
///
/// Every field above the signature is covered by it, which is what makes a relay harmless:
/// a runtime that forwards an event cannot alter its stream, its sequence, its repository
/// or its body without the next reader noticing, and so vouches for nothing. The wall
/// clock is the one field nothing is decided by — freshness is measured with beats and
/// monotonic clocks, never by comparing two machines' idea of the time.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, MeshEvent};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
///
/// let event: &MeshEvent = &j.events()[0];
/// assert_eq!(event.seq, 1, "dense from 1 within its stream");
/// assert_eq!(event.kind(), "session_closed");
/// assert_eq!(event.id(), format!("{}/1", event.stream));
///
/// // a relayed byte changed anywhere under the signature is a different event
/// let mut tampered = event.clone();
/// tampered.repo = "another-repository".into();
/// assert_ne!(tampered.signing_bytes(), event.signing_bytes());
/// ```
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

/// The causal position of an event: its Lamport stamp, then its stream, then its sequence.
///
/// One definition, because every runtime must apply the same events in the same sequence
/// for the fold to converge, and two comparators that drift apart would be two different
/// states with one name. Collections key a `BTreeMap` by it rather than sorting themselves.
///
/// ```
/// use majordomus_cli::mesh::journal::{causal_key, EventBody, Journal};
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use std::sync::Arc;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
/// let event = &j.events()[0];
/// assert_eq!(causal_key(event), (event.lamport, event.stream.clone(), event.seq));
/// ```
pub fn causal_key(event: &MeshEvent) -> (u64, StreamId, u64) {
    (event.lamport, event.stream.clone(), event.seq)
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
            // A BTreeSet, not a sorted vector: the signing order is the byte order of the
            // keys, decided by the container rather than by a comparator written here.
            let keys: std::collections::BTreeSet<&String> = map.keys().collect();
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
    /// The bytes the signature covers: a domain separator, then the canonical JSON of
    /// every field of the event except the signature itself. Canonical, so that signer and
    /// verifier agree however their JSON parsers order keys; domain-separated, so that a
    /// signature over an event can never be presented as one over an advertisement or a
    /// link message.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// let event = j.events()[0].clone();
    ///
    /// // the signature is not part of what it covers, so re-signing changes nothing here
    /// let mut resigned = event.clone();
    /// resigned.sig = "00".repeat(64);
    /// assert_eq!(resigned.signing_bytes(), event.signing_bytes());
    ///
    /// // the body is
    /// let mut edited = event.clone();
    /// edited.body = serde_json::json!({ "kind": "session_closed", "session": "s2" });
    /// assert_ne!(edited.signing_bytes(), event.signing_bytes());
    /// ```
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

    /// `<stream>/<seq>`: the event's identity everywhere. It is the pair rather than a
    /// digest because it is also the address replication works with — a peer's marks name
    /// a stream and a sequence, and what is missing is arithmetic rather than a search.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    ///
    /// let events = j.events();
    /// assert_eq!(events[0].id(), format!("{}/1", events[0].stream));
    /// assert_ne!(events[0].id(), events[1].id());
    /// ```
    pub fn id(&self) -> String {
        format!("{}/{}", self.stream, self.seq)
    }

    /// The interpreted body; `None` for a kind this executable does not know. An event
    /// this version cannot read is still stored and relayed byte-for-byte, so an older
    /// runtime in a mesh with newer ones carries their traffic instead of breaking it —
    /// the fold counts what it could not interpret rather than dropping it silently.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// let event = j.events()[0].clone();
    /// assert!(matches!(event.body(), Some(EventBody::SessionClosed { .. })));
    ///
    /// // a kind from a newer peer: unreadable here, and still an event
    /// let mut newer = event.clone();
    /// newer.body = serde_json::json!({ "kind": "telepathy_offered" });
    /// assert!(newer.body().is_none());
    /// assert_eq!(newer.kind(), "telepathy_offered");
    /// ```
    pub fn body(&self) -> Option<EventBody> {
        serde_json::from_value(self.body.clone()).ok()
    }

    /// The body's `kind` word, known or not. It reads the tag out of the stored JSON
    /// rather than going through [`MeshEvent::body`], so that an event this executable
    /// cannot interpret can still be counted and shown by name.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionOpened {
    ///     info: majordomus_cli::mesh::journal::SessionInfo::named("s1", "cli"),
    /// })
    /// .unwrap();
    /// assert_eq!(j.events()[0].kind(), "session_opened");
    /// ```
    pub fn kind(&self) -> &str {
        self.body.get("kind").and_then(Value::as_str).unwrap_or("")
    }
}

/// Why an event was not stored. A journal that silently drops what it is sent is a journal
/// nobody can debug, so every refusal is counted under one of these reasons and shown: a
/// mesh whose runtimes will not converge can be diagnosed from the numbers, because the
/// reasons distinguish a misconfiguration (`Repository`, `Untrusted`) from an attack
/// (`Signature`, `Identity`) from a limit reached (`Capacity`, `Oversized`).
///
/// ```
/// use majordomus_cli::mesh::journal::Rejection;
///
/// // the wire word is what a counter and a JSON report both carry
/// assert_eq!(serde_json::to_value(Rejection::Untrusted).unwrap(), serde_json::json!("untrusted"));
/// assert_ne!(Rejection::Untrusted, Rejection::Repository);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
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

/// What one ingest did: how many events were stored, how many were deliveries of something
/// already held, how many are waiting for a gap ahead of them, and what was refused and
/// why. Every event handed in is accounted for in exactly one of those, which is what makes
/// a sync round auditable — a peer that sends ten events and is told about ten knows
/// nothing went missing between the wire and the store.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, IngestReport, Journal, SessionInfo};
///
/// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
///     "repo".into(), None).unwrap();
/// a.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }).unwrap();
///
/// let events = a.missing_for(&b.marks(), 1 << 20);
/// let first: IngestReport = b.ingest(&events, &|_| Ok(()));
/// assert_eq!((first.accepted, first.duplicate, first.rejected_total()), (1, 0, 0));
///
/// // at-least-once delivery: the same event again is absorbed, and changes nothing
/// let second = b.ingest(&events, &|_| Ok(()));
/// assert_eq!((second.accepted, second.duplicate), (0, 1));
/// ```
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
/// beat with its origin's signature, and how long ago that beat was seen to rise (`None`
/// when never).
///
/// A mark is both halves of replication in one value. The sequence is what a peer compares
/// to decide what to send, which is why replication is a comparison rather than a flood.
/// The beat is how liveness crosses machines without comparing clocks: the origin signs it,
/// a relay forwards the signature and adds how long ago *it* saw the beat rise, and the
/// reader converts that to freshness on its own monotonic clock. Because only the origin
/// can sign its beat, a relay can carry a stream's liveness without being able to invent it.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamMark};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
/// j.beat_own();
///
/// let marks = j.marks();
/// let mark: &StreamMark = &marks[&j.own_stream()];
/// assert_eq!(mark.seq, 1, "every event up to here is held");
/// assert!(mark.beat > 0 && mark.sig.is_some(), "the beat is the origin's signed statement");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamMark {
    /// Every event up to and including this sequence is held.
    pub seq: u64,
    /// The highest beat heard.
    pub beat: u64,
    /// Milliseconds since that beat rose, on the sender's monotonic clock. Clamped by the
    /// reader to its expiry: a relay can report a beat stale, never older than dead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_ms: Option<u64>,
    /// The origin's public key, hex: the beat is its statement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pk: Option<String>,
    /// The origin's signature over the stream and the beat, hex. A relay forwards it
    /// verbatim; only the origin can raise its own beat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
}

/// The bytes a beat signature covers.
fn beat_bytes(stream: &StreamId, beat: u64) -> Vec<u8> {
    format!("majordomus-mesh-beat/v1\n{stream}\n{beat}").into_bytes()
}

/// Every stream's mark: what a sync request and answer carry.
pub type Marks = BTreeMap<StreamId, StreamMark>;

/// Whether a stream is speaking now, on this runtime's clock. It is a verdict about a
/// stream and not about a link: a runtime reachable only through a relay is alive, and a
/// runtime that crashed stops beating for everyone at once. `Own` is kept apart from `Live`
/// because a runtime's own stream needs no evidence — it is the thing doing the beating.
///
/// ```
/// use majordomus_cli::mesh::journal::StreamLiveness;
///
/// assert!(StreamLiveness::Own.is_alive() && StreamLiveness::Live.is_alive());
/// assert!(!StreamLiveness::Expired.is_alive());
/// assert_eq!(
///     serde_json::to_value(StreamLiveness::Expired).unwrap(),
///     serde_json::json!("expired"),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamLiveness {
    /// This runtime's own current stream.
    Own,
    /// Its beat rose within the expiry.
    Live,
    /// Its beat has not risen within the expiry, or was never heard.
    Expired,
}

impl StreamLiveness {
    /// Whether the stream counts as alive: its claims hold, its sessions are present. The
    /// two living verdicts are collapsed here because everything downstream treats them
    /// alike — a claim of this runtime's own and a claim of a peer that still beats both
    /// exclude — while the distinction is kept in the enum for anything that reports it.
    ///
    /// ```
    /// use majordomus_cli::mesh::journal::StreamLiveness;
    ///
    /// assert!(StreamLiveness::Own.is_alive(), "no evidence needed for one's own stream");
    /// assert!(StreamLiveness::Live.is_alive());
    /// assert!(!StreamLiveness::Expired.is_alive(), "its claims end without anyone releasing them");
    /// ```
    pub fn is_alive(self) -> bool {
        !matches!(self, StreamLiveness::Expired)
    }
}

/// The journal's counters since start: how much it holds, how much it has written,
/// received, absorbed, refused, held back and compacted away. They are what makes a
/// replication problem diagnosable from one machine — a runtime whose `received` never
/// rises is not linked, one whose `rejected` rises is being sent something it will not
/// take, and one whose `pending` stays high is missing an event ahead of what it has.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, JournalTallies};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let before: JournalTallies = j.tallies();
/// assert_eq!((before.written, before.events), (0, 0));
///
/// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
/// let after = j.tallies();
/// assert_eq!((after.written, after.events, after.streams), (1, 1, 1));
/// assert!(after.lamport > before.lamport);
/// ```
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
    beat_pk: Option<String>,
    beat_sig: Option<String>,
    fresh_at: Option<Instant>,
}

impl StreamLog {
    fn high_water(&self) -> u64 {
        self.events.keys().next_back().copied().unwrap_or(0)
    }
}

struct Inner {
    /// Streams compacted away, with the high-water sequence they had: their marks keep
    /// being advertised, so a peer that still holds the stream does not send it again.
    tombstones: BTreeMap<StreamId, u64>,
    streams: BTreeMap<StreamId, StreamLog>,
    lamport: u64,
    tallies: JournalTallies,
}

/// The journal of one runtime. `Mutex<BTreeMap>`: a handful of events a second, and a
/// deterministic order falls out for free.
///
/// One journal holds every stream this runtime knows of, not only its own: what arrives
/// from a peer is stored beside what this runtime wrote, which is what lets it relay. It
/// is the only thing in the mesh that writes, and everything a surface shows about
/// cooperation is a fold of what it holds.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo};
///
/// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
///     "repo".into(), None).unwrap();
/// a.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }).unwrap();
///
/// // what B receives is held beside what B writes, so B can relay A's stream onwards
/// b.ingest(&a.missing_for(&b.marks(), 1 << 20), &|_| Ok(()));
/// assert_eq!(b.tallies().streams, 2, "B's own stream, and the one it heard from A");
/// assert!(b.events().iter().any(|e| &e.stream == a.own_stream()));
/// ```
pub struct Journal {
    identity: Arc<NodeIdentity>,
    own: StreamId,
    repo: String,
    path: Option<PathBuf>,
    inner: Mutex<Inner>,
    rotation: std::sync::atomic::AtomicUsize,
}

impl Journal {
    /// Open the journal of this runtime: the own stream is `<node>-<runtime>-<instance>` of
    /// the identity given, and `path`, when given, is the JSONL file events persist to and
    /// are reloaded from. Reloaded events keep their streams; none of them is fresh until
    /// a beat is heard again, so a restart never resurrects anybody's ownership.
    ///
    /// That last part is the reason persistence is safe here: reloading events would
    /// otherwise bring back claims whose holders are long gone, and a restarted runtime
    /// would refuse work on behalf of machines that are not running. Persistence is
    /// optional because a journal without a path is a complete journal — it simply starts
    /// empty and refills from its peers.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let path = dir.path().join("journal.jsonl");
    ///
    /// let before = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()),
    ///     "0000000000000001", "repo".into(), Some(path.clone())).unwrap();
    /// before.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    ///
    /// // another runtime opens the same file: the events are back, and whoever wrote them
    /// // is not alive here until a beat is heard again
    /// let after = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()),
    ///     "0000000000000002", "repo".into(), Some(path)).unwrap();
    /// assert_eq!(after.events().len(), 1);
    /// assert_eq!(
    ///     after.liveness(before.own_stream(), Duration::from_secs(30)),
    ///     StreamLiveness::Expired,
    /// );
    ///
    /// // a runtime slot that is not 16 hex is not a runtime
    /// let ephemeral = Arc::new(NodeIdentity::ephemeral().unwrap());
    /// assert!(Journal::open(ephemeral, "nope", "repo".into(), None).is_err());
    /// ```
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
        .ok_or_else(|| {
            MeshError::Protocol(format!("runtime '{runtime}' is not 16 hex characters"))
        })?;
        let mut streams = BTreeMap::new();
        streams.insert(own.clone(), StreamLog::default());
        let journal = Journal {
            identity,
            own,
            repo,
            path,
            inner: Mutex::new(Inner {
                tombstones: BTreeMap::new(),
                streams,
                lamport: 0,
                tallies: JournalTallies::default(),
            }),
            rotation: std::sync::atomic::AtomicUsize::new(0),
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

    /// This runtime's own stream: the one stream this journal may write to, and the prefix
    /// every claim, session and review key it creates carries. Nothing else in the journal
    /// is this runtime's to number or to sign.
    pub fn own_stream(&self) -> &StreamId {
        &self.own
    }

    /// The mesh repository id this journal belongs to.
    pub fn repository(&self) -> &str {
        &self.repo
    }

    /// Sign, store and persist an event of this runtime. The body's bounds are checked
    /// first: nothing out of bounds is ever signed.
    ///
    /// The sequence is dense from 1 and the Lamport stamp rises above every stamp this
    /// runtime has seen, so a peer can tell from the numbers alone whether it is missing
    /// something, and every runtime folds concurrent events in the same order. Writing is
    /// the only way an event enters the mesh; a relay never creates one.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let first = j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// let second = j.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    /// assert_eq!((first.seq, second.seq), (1, 2), "dense from 1");
    /// assert!(second.lamport > first.lamport);
    /// assert_eq!(first.stream, *j.own_stream());
    ///
    /// // out of bounds is refused before anything is signed or stored
    /// let escaping = EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
    ///     scope: vec!["/etc".into()], intent: None, mode: Default::default(), issue: None };
    /// assert!(j.append_own(escaping).is_err());
    /// assert_eq!(j.events().len(), 2, "and nothing was written");
    /// ```
    pub fn append_own(&self, body: EventBody) -> Result<MeshEvent, MeshError> {
        body.validate().map_err(MeshError::Protocol)?;
        let body = serde_json::to_value(&body).map_err(|e| MeshError::Protocol(e.to_string()))?;
        let mut inner = self.inner.lock().expect("journal lock");
        let log = inner.streams.entry(self.own.clone()).or_default();
        let seq = log.high_water().saturating_add(1);
        let lamport = inner.lamport.saturating_add(1);
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
        let size = serde_json::to_vec(&event)
            .map(|b| b.len())
            .unwrap_or(usize::MAX);
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

    /// Raise this runtime's own beat: one heartbeat, signed, so that no relay can raise it.
    ///
    /// The beat is how a runtime says "still here" without writing an event for it — it
    /// travels in marks, which every sync round carries anyway, so liveness costs nothing
    /// extra. The signature is what makes a relay safe: a runtime can pass on somebody
    /// else's beat and cannot invent one, so a crashed runtime stops being alive for
    /// everybody at once and its claims end on their own.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::Journal;
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.beat_own();
    /// let first = j.marks()[j.own_stream()].clone();
    /// j.beat_own();
    /// let second = j.marks()[j.own_stream()].clone();
    ///
    /// assert!(second.beat > first.beat, "the beat only rises");
    /// assert_ne!(second.sig, first.sig, "each beat is its own signed statement");
    /// ```
    pub fn beat_own(&self) {
        let mut inner = self.inner.lock().expect("journal lock");
        let log = inner.streams.entry(self.own.clone()).or_default();
        log.beat += 1;
        log.beat_sig = Some(self.identity.sign(&beat_bytes(&self.own, log.beat)));
        log.beat_pk = Some(self.identity.public.public_key.clone());
        log.fresh_at = Some(Instant::now());
    }

    /// Every stream's mark, own included, ages measured now. This is the whole of what a
    /// runtime tells a peer about its state: a sequence per stream and a beat per stream,
    /// from which the peer computes what to send. Nothing is asked for and nothing is
    /// flooded, which is why an event reaches every runtime once per link and stops.
    ///
    /// The ages are measured at the moment of the call rather than stored, because an age
    /// is only meaningful relative to when it was taken.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// assert_eq!(j.marks()[j.own_stream()].seq, 0, "nothing written yet");
    ///
    /// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// j.beat_own();
    /// let mark = j.marks()[j.own_stream()].clone();
    /// assert_eq!(mark.seq, 1);
    /// assert!(mark.age_ms.is_some(), "a beat has been heard, and how long ago");
    /// ```
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
                        pk: log.beat_pk.clone(),
                        sig: log.beat_sig.clone(),
                    },
                )
            })
            .chain(inner.tombstones.iter().map(|(id, seq)| {
                (
                    id.clone(),
                    StreamMark {
                        seq: *seq,
                        ..StreamMark::default()
                    },
                )
            }))
            .collect()
    }

    /// Merge a peer's marks. Only a higher beat carrying its origin's valid signature, from
    /// an origin `trusted` accepts, makes a stream fresher here — a relay forwards beats and
    /// cannot mint one — and the relayed age is clamped to the expiry, so a stale report can
    /// age a stream but never hold a dead one alive past one expiry after its last real beat.
    /// Sequences are not merged: only events move high-water marks. A stream is created from
    /// a mark only when the mark verifies, and only within the stream quotas.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{Journal, StreamLiveness};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// a.beat_own();
    ///
    /// // B learns that A is beating, because A signed the beat
    /// b.merge_marks(&a.marks(), &|_| true, Duration::from_secs(30));
    /// assert_eq!(b.liveness(a.own_stream(), Duration::from_secs(30)), StreamLiveness::Live);
    ///
    /// // an untrusted origin's beat tells B nothing, however well signed
    /// let c = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000003",
    ///     "repo".into(), None).unwrap();
    /// c.beat_own();
    /// b.merge_marks(&c.marks(), &|_| false, Duration::from_secs(30));
    /// assert_eq!(b.liveness(c.own_stream(), Duration::from_secs(30)), StreamLiveness::Expired);
    /// ```
    pub fn merge_marks(&self, marks: &Marks, trusted: &dyn Fn(&str) -> bool, expiry: Duration) {
        let now = Instant::now();
        let ceiling = (expiry + Duration::from_secs(1)).as_millis() as u64;
        let mut inner = self.inner.lock().expect("journal lock");
        for (id, mark) in marks {
            if *id == self.own || mark.beat == 0 {
                continue;
            }
            let (Some(pk), Some(sig)) = (&mark.pk, &mark.sig) else {
                continue;
            };
            if node_id_of_key(pk).map(|n| n.as_str().to_string()) != Some(id.node().to_string())
                || !verify(pk, &beat_bytes(id, mark.beat), sig)
                || !trusted(pk)
            {
                continue;
            }
            if !inner.streams.contains_key(id) {
                let of_node = inner
                    .streams
                    .keys()
                    .filter(|s| s.node() == id.node())
                    .count();
                if inner.streams.len() >= MAX_STREAMS || of_node >= MAX_STREAMS_PER_NODE {
                    continue;
                }
            }
            let log = inner.streams.entry(id.clone()).or_default();
            if mark.beat > log.beat {
                log.beat = mark.beat;
                log.beat_pk = Some(pk.clone());
                log.beat_sig = Some(sig.clone());
                log.fresh_at = mark
                    .age_ms
                    .map(|age| age.min(ceiling))
                    .and_then(|age| now.checked_sub(Duration::from_millis(age)));
            }
        }
    }

    /// The events a peer lacks by its marks, in stream-then-sequence order, at most
    /// `budget` serialized bytes (always at least one event when any is missing).
    ///
    /// This is the half of replication that decides what travels, and it decides it from
    /// the peer's own marks rather than from a request: a runtime sends what the peer says
    /// it lacks and nothing else, so an event crosses each link once and stops. The budget
    /// keeps one round bounded, and the streams are visited from a rotating start, so a
    /// stream a peer keeps refusing cannot spend the whole budget round after round and
    /// starve the streams behind it.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// a.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// a.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    ///
    /// // B has nothing, so both events are missing for it
    /// let missing = a.missing_for(&b.marks(), 1 << 20);
    /// assert_eq!(missing.len(), 2);
    ///
    /// // once B holds them, its marks say so and nothing is sent again
    /// b.ingest(&missing, &|_| Ok(()));
    /// assert!(a.missing_for(&b.marks(), 1 << 20).is_empty());
    ///
    /// // a budget of nothing still moves one event: progress is never zero
    /// assert_eq!(a.missing_for(&Default::default(), 0).len(), 1);
    /// ```
    pub fn missing_for(&self, peer: &Marks, budget: usize) -> Vec<MeshEvent> {
        let inner = self.inner.lock().expect("journal lock");
        let mut out = Vec::new();
        let mut spent = 0usize;
        // Start at a different stream each round, so that one stream a peer refuses — and
        // keeps lacking — cannot spend the whole budget round after round and starve the
        // streams behind it.
        let count = inner.streams.len().max(1);
        let start = self
            .rotation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % count;
        let order = inner
            .streams
            .iter()
            .skip(start)
            .chain(inner.streams.iter().take(start));
        for (id, log) in order {
            let have = peer.get(id).map(|m| m.seq).unwrap_or(0);
            let Some(from) = have.checked_add(1) else {
                continue;
            };
            for (_, event) in log.events.range(from..) {
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
    ///
    /// Trust is the caller's because it is policy, and everything else is decided here
    /// because it must be decided the same way for every arrival: a datagram, a sync round
    /// and a file reloaded from disk are all input, and none of them is believed. Events
    /// are applied in stream order, so one that arrives ahead of its gap waits rather than
    /// being applied out of turn, and one that arrives twice is counted and changes nothing.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal, Rejection};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// let first = a.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// let second = a.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    ///
    /// // the second event alone arrives first: it waits for the gap ahead of it
    /// let report = b.ingest(&[second.clone()], &|_| Ok(()));
    /// assert_eq!((report.accepted, report.pending), (0, 1));
    ///
    /// // the first arrives and both are applied, in the stream's order
    /// let report = b.ingest(&[first], &|_| Ok(()));
    /// assert_eq!(report.accepted, 2);
    /// assert_eq!(b.events().len(), 2);
    ///
    /// // an untrusted origin is refused, with the reason the caller gave
    /// let c = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000003",
    ///     "repo".into(), None).unwrap();
    /// let theirs = c.append_own(EventBody::SessionClosed { session: "s9".into() }).unwrap();
    /// let report = b.ingest(&[theirs], &|_| Err(Rejection::Untrusted));
    /// assert_eq!(report.rejected[&Rejection::Untrusted], 1);
    /// ```
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
        if serde_json::to_vec(&event)
            .map(|b| b.len())
            .unwrap_or(usize::MAX)
            > MAX_EVENT_BYTES
        {
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
        if inner
            .tombstones
            .get(&event.stream)
            .is_some_and(|high| event.seq <= *high)
        {
            // Already held once and compacted away as dead: a late copy changes nothing.
            report.duplicate += 1;
            if count {
                inner.tallies.duplicates += 1;
            }
            return None;
        }
        if !inner.streams.contains_key(&event.stream) && inner.streams.len() >= MAX_STREAMS {
            return refuse(inner, report, Rejection::Capacity);
        }
        // Quotas per node: stream, runtime and instance ids cost nothing to invent, so one
        // key — even a trusted one — gets a bounded share of the journal.
        let node = event.stream.node().to_string();
        if !inner.streams.contains_key(&event.stream)
            && inner.streams.keys().filter(|s| s.node() == node).count() >= MAX_STREAMS_PER_NODE
        {
            return refuse(inner, report, Rejection::Capacity);
        }
        let held_by_node: usize = inner
            .streams
            .iter()
            .filter(|(s, _)| s.node() == node)
            .map(|(_, l)| l.events.len() + l.pending.len())
            .sum();
        if held_by_node >= MAX_EVENTS_PER_NODE {
            return refuse(inner, report, Rejection::Capacity);
        }
        let pending_total: usize = inner.streams.values().map(|l| l.pending.len()).sum();
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
            if log.pending.len() >= MAX_PENDING || pending_total >= MAX_PENDING_TOTAL {
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
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
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

    /// A stream's liveness on this runtime's clock, against `expiry`. The measurement is
    /// local and monotonic — how long since this process saw the beat rise — so two
    /// machines never compare wall clocks and a machine with a wrong clock cannot hold a
    /// dead runtime's claims alive. A stream nobody has ever beaten for is expired, which
    /// is the right answer for a stream learned from a relay that has not heard from it.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{Journal, StreamLiveness};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// let expiry = Duration::from_secs(30);
    ///
    /// assert_eq!(a.liveness(a.own_stream(), expiry), StreamLiveness::Own);
    /// assert_eq!(a.liveness(b.own_stream(), expiry), StreamLiveness::Expired, "never heard of");
    ///
    /// b.beat_own();
    /// a.merge_marks(&b.marks(), &|_| true, expiry);
    /// assert_eq!(a.liveness(b.own_stream(), expiry), StreamLiveness::Live);
    /// ```
    pub fn liveness(&self, stream: &StreamId, expiry: Duration) -> StreamLiveness {
        if *stream == self.own {
            return StreamLiveness::Own;
        }
        let now = Instant::now();
        let inner = self.inner.lock().expect("journal lock");
        match inner.streams.get(stream).and_then(|l| l.fresh_at) {
            Some(t) if now.saturating_duration_since(t) <= expiry => StreamLiveness::Live,
            _ => StreamLiveness::Expired,
        }
    }

    /// Every stream's liveness and time since its beat rose, in stream order. The age
    /// travels with the verdict because "expired" alone is not diagnosable: a stream whose
    /// beat rose a minute ago is a runtime that stopped, and one whose beat was never heard
    /// is a runtime this one has only been told about.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{Journal, StreamLiveness};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let all = j.stream_liveness(Duration::from_secs(30));
    /// let (liveness, age) = all[j.own_stream()];
    /// assert_eq!(liveness, StreamLiveness::Own);
    /// assert_eq!(age, None, "no beat has been raised yet");
    ///
    /// j.beat_own();
    /// assert!(j.stream_liveness(Duration::from_secs(30))[j.own_stream()].1.is_some());
    /// ```
    pub fn stream_liveness(
        &self,
        expiry: Duration,
    ) -> BTreeMap<StreamId, (StreamLiveness, Option<Duration>)> {
        let now = Instant::now();
        let inner = self.inner.lock().expect("journal lock");
        inner
            .streams
            .iter()
            .map(|(id, log)| {
                let age = log.fresh_at.map(|t| now.saturating_duration_since(t));
                let liveness = if *id == self.own {
                    StreamLiveness::Own
                } else if age.is_some_and(|a| a <= expiry) {
                    StreamLiveness::Live
                } else {
                    StreamLiveness::Expired
                };
                (id.clone(), (liveness, age))
            })
            .collect()
    }

    /// Every held event, in stream-then-sequence order. Events waiting for a gap are not
    /// here: what this returns is what the journal is prepared to stand behind, and the
    /// fold takes it whole, which is why the order it comes out in does not matter to the
    /// result.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// let first = a.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// let second = a.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    ///
    /// // an event still waiting for the gap ahead of it is not yet held
    /// b.ingest(&[second], &|_| Ok(()));
    /// assert!(b.events().is_empty());
    /// b.ingest(&[first], &|_| Ok(()));
    /// assert_eq!(b.events().len(), 2);
    /// assert_eq!(b.events()[0].seq, 1, "stream order, whatever the arrival order was");
    /// ```
    pub fn events(&self) -> Vec<MeshEvent> {
        let inner = self.inner.lock().expect("journal lock");
        inner
            .streams
            .values()
            .flat_map(|log| log.events.values().cloned())
            .collect()
    }

    /// Held events whose Lamport stamp is above `after`, in Lamport order, at most `limit`.
    ///
    /// The Lamport stamp is what a reader can resume from: it rises with causality rather
    /// than with anybody's clock, so a caller that remembers the last stamp it saw reads
    /// each event once however the machines involved disagree about the time. This is what
    /// `mesh events --after` is.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let first = j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
    ///
    /// assert_eq!(j.events_after(0, 100).len(), 2, "from the beginning");
    /// let rest = j.events_after(first.lamport, 100);
    /// assert_eq!(rest.len(), 1, "resumed where the reader left off");
    /// assert!(rest[0].lamport > first.lamport);
    /// assert_eq!(j.events_after(0, 1).len(), 1, "one page");
    /// ```
    pub fn events_after(&self, after: u64, limit: usize) -> Vec<MeshEvent> {
        self.events()
            .into_iter()
            .filter(|e| e.lamport > after)
            .map(|e| (causal_key(&e), e))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .take(limit)
            .collect()
    }

    /// Drop every stream that has been expired for longer than `retention` and holds no
    /// handover, when the journal is over its bound or `force` is set; the file is
    /// rewritten to what remains. Only dead streams go, so no live claim can lose its
    /// release to compaction.
    ///
    /// The two exclusions are what make dropping events safe. Only a stream that has been
    /// silent well past its expiry goes, so no claim can lose the release that would have
    /// ended it; and a stream holding a published handover stays, because a handover is
    /// continuity somebody may still be waiting to pick up. What is dropped leaves a
    /// tombstone carrying the sequence it reached, so a peer that still holds the stream
    /// is not sent it all over again.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let a = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// let b = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
    ///     "repo".into(), None).unwrap();
    /// a.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    /// b.ingest(&a.missing_for(&b.marks(), 1 << 20), &|_| Ok(()));
    /// assert_eq!(b.events().len(), 1);
    ///
    /// // under its bounds, compaction does nothing unless it is asked
    /// assert_eq!(b.compact(Duration::ZERO, Duration::ZERO, false), 0);
    ///
    /// // asked, the silent stream goes, and its mark stays so it is not re-sent
    /// assert_eq!(b.compact(Duration::ZERO, Duration::ZERO, true), 1);
    /// assert!(b.events().is_empty());
    /// assert_eq!(b.marks()[a.own_stream()].seq, 1, "the tombstone remembers how far it got");
    /// ```
    pub fn compact(&self, expiry: Duration, retention: Duration, force: bool) -> u64 {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("journal lock");
        let total: usize = inner.streams.values().map(|l| l.events.len()).sum();
        if !force && total <= MAX_EVENTS && inner.streams.len() <= MAX_STREAMS * 3 / 4 {
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
                    && !log
                        .events
                        .values()
                        .any(|e| e.kind() == "handover_published")
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut dropped = 0u64;
        for id in dead {
            if let Some(log) = inner.streams.remove(&id) {
                dropped += log.events.len() as u64;
                if inner.tombstones.len() >= MAX_STREAMS * 4 {
                    if let Some(first) = inner.tombstones.keys().next().cloned() {
                        inner.tombstones.remove(&first);
                    }
                }
                inner.tombstones.insert(id.clone(), log.high_water());
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

    /// The tallies now: a snapshot of the counters, taken under the lock so that the
    /// numbers in one answer are consistent with each other rather than read one at a time
    /// while the journal moves. This is what `mesh status` and the cooperation status show.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal};
    ///
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionClosed { session: "s1".into() }).unwrap();
    ///
    /// let snapshot = j.tallies();
    /// assert_eq!(snapshot.written, 1);
    /// assert_eq!(snapshot.events, j.events().len());
    /// assert_eq!(snapshot.received, 0, "nothing came from a peer");
    /// ```
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
        assert_eq!(
            report.accepted, 3,
            "the gap closes and its successors apply"
        );
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
        let wrong_version = MeshEvent {
            v: 2,
            ..good.clone()
        };
        let mut stolen_stream = good.clone();
        stolen_stream.stream = b.own_stream().clone();
        let mut huge = good.clone();
        huge.body = serde_json::json!({"kind": "future", "blob": "x".repeat(MAX_EVENT_BYTES)});

        let r = b.ingest(
            &[other_repo, wrong_version, stolen_stream, huge],
            &accept_all,
        );
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
        assert_eq!(
            b.liveness(a.own_stream(), expiry),
            StreamLiveness::Expired,
            "never heard"
        );
        a.beat_own();
        b.merge_marks(&a.marks(), &|_| true, expiry);
        assert_eq!(b.liveness(a.own_stream(), expiry), StreamLiveness::Live);
        // C hears A only through B.
        c.merge_marks(&b.marks(), &|_| true, expiry);
        assert_eq!(c.liveness(a.own_stream(), expiry), StreamLiveness::Live);
        // An old beat relayed with a huge age is clamped to the expiry, never fresh.
        let d = journal("0000000000000004");
        let mut stale = b.marks();
        stale.get_mut(a.own_stream()).unwrap().age_ms = Some(u64::MAX);
        d.merge_marks(&stale, &|_| true, expiry);
        assert_eq!(d.liveness(a.own_stream(), expiry), StreamLiveness::Expired);
        assert_eq!(d.liveness(d.own_stream(), expiry), StreamLiveness::Own);
    }

    #[test]
    fn a_relay_cannot_mint_a_beat_and_an_untrusted_origin_is_not_heard() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let expiry = Duration::from_secs(30);
        a.beat_own();
        let genuine = a.marks();
        // A relay raises A's beat to the ceiling: the signature no longer covers it.
        let mut forged = genuine.clone();
        forged.get_mut(a.own_stream()).unwrap().beat = u64::MAX;
        b.merge_marks(&forged, &|_| true, expiry);
        assert_eq!(b.liveness(a.own_stream(), expiry), StreamLiveness::Expired);
        // An unsigned mark is ignored, and creates no stream.
        let mut unsigned = genuine.clone();
        unsigned.get_mut(a.own_stream()).unwrap().sig = None;
        b.merge_marks(&unsigned, &|_| true, expiry);
        assert_eq!(b.tallies().streams, 1, "only B's own stream exists");
        // A genuine beat from an origin the policy does not trust is not heard.
        b.merge_marks(&genuine, &|_| false, expiry);
        assert_eq!(b.liveness(a.own_stream(), expiry), StreamLiveness::Expired);
        // And the genuine, trusted beat is.
        b.merge_marks(&genuine, &|_| true, expiry);
        assert_eq!(b.liveness(a.own_stream(), expiry), StreamLiveness::Live);
        // A later beat after the original one cannot be replayed backwards.
        a.beat_own();
        b.merge_marks(&a.marks(), &|_| true, expiry);
        b.merge_marks(&genuine, &|_| true, expiry);
        assert_eq!(b.marks()[a.own_stream()].beat, 2);
    }

    #[test]
    fn a_stream_id_off_the_wire_is_parsed_or_refused() {
        let bad: Result<StreamId, _> = serde_json::from_value(serde_json::json!("x"));
        assert!(
            bad.is_err(),
            "an unparsed stream id would panic a later slice"
        );
        let marks: Result<Marks, _> =
            serde_json::from_value(serde_json::json!({"x": {"seq": 1, "beat": 1}}));
        assert!(marks.is_err());
    }

    #[test]
    fn one_node_gets_a_bounded_share_of_the_journal() {
        let own = journal("0000000000000002");
        let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        let mut refused = 0;
        for slot in 0..(MAX_STREAMS_PER_NODE + 4) {
            let j = Journal::open(
                Arc::clone(&identity),
                &format!("{slot:016x}"),
                "repo".into(),
                None,
            )
            .unwrap();
            let event = opened(&j, "s1");
            refused += own.ingest(&[event], &accept_all).rejected_total();
        }
        assert_eq!(
            refused, 4,
            "the streams past the per-node quota are refused"
        );
    }

    #[test]
    fn a_compacted_stream_is_not_sent_again() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        opened(&a, "s1");
        b.ingest(&a.missing_for(&b.marks(), usize::MAX), &accept_all);
        assert_eq!(b.compact(Duration::ZERO, Duration::ZERO, true), 1);
        assert_eq!(
            b.marks()[a.own_stream()].seq,
            1,
            "the tombstone keeps the mark"
        );
        assert!(a.missing_for(&b.marks(), usize::MAX).is_empty());
        let again = b.ingest(&a.events(), &accept_all);
        assert_eq!((again.accepted, again.duplicate), (0, 1));
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
        b.merge_marks(&a.marks(), &|_| true, Duration::from_secs(30));
        b.ingest(&a.missing_for(&b.marks(), usize::MAX), &accept_all);
        let own_before = b.own_stream().clone();
        b.append_own(EventBody::SessionOpened {
            info: SessionInfo::named("mine", "test"),
        })
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
        assert_eq!(
            restarted.liveness(a.own_stream(), expiry),
            StreamLiveness::Expired
        );
        assert_eq!(
            restarted.liveness(&own_before, expiry),
            StreamLiveness::Expired,
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
