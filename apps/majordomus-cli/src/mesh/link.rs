//! The link protocol: what two runtimes say to become linked, and what they exchange once
//! they are. Discovery ([`super::protocol`]) proves that a key exists somewhere; a link
//! proves that the key is answering *now*, serves the *same repository*, speaks a
//! *compatible protocol*, and is *trusted* — and only then does anything cooperative
//! travel.
//!
//! Two exchanges, both request–answer, both signed end to end:
//!
//! 1. **Hello → Welcome.** The dialer presents its [`RuntimeCard`] (key, runtime,
//!    instance, repository, version, features, endpoints), a fresh nonce and its supported
//!    protocol range, signed. The answerer verifies the signature, freshness and nonce
//!    (a replayed hello is refused), negotiates the protocol, refuses another repository,
//!    itself, or an untrusted key — each with a typed [`RefusalCode`] — and otherwise
//!    answers with its own card, a link id, its marks, and the dialer's nonce, signed. The
//!    dialer verifies all of it and applies the same checks the other way. Mutual:
//!    neither side is linked on the other's word alone.
//! 2. **Sync ↔ Sync.** The dialer sends its marks and the events the answerer lacks,
//!    under the link id with a rising counter, signed; the answerer ingests, answers with
//!    its marks and the events the dialer lacks, signed. One round is one heartbeat and
//!    one full replication step in both directions, so a link works even when only one
//!    side can reach the other.
//!
//! Every message is a [`Signed`] body: canonical JSON under a per-message domain
//! separator, so a signature over one message kind can never be replayed as another.
//! Discovery never grants any of this: a refused hello is recorded, visible, and links
//! nothing (`project.mesh-cooperation-is-authenticated`).
//!
//! ```
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::link::{negotiate, sign, verify_signed, Domain};
//!
//! let node = NodeIdentity::ephemeral().unwrap();
//! let signed = sign(&node, Domain::Hello, serde_json::json!({"hello": 1}));
//! assert!(verify_signed(&node.public.public_key, Domain::Hello, &signed));
//! assert!(!verify_signed(&node.public.public_key, Domain::Welcome, &signed), "domains do not cross");
//! assert_eq!(negotiate(1, 3), Some(1));
//! assert_eq!(negotiate(2, 3), None);
//! ```

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::identity::{node_id_of_key, verify, NodeIdentity};
use super::journal::{canonical_json, IngestReport, Marks, MeshEvent, StreamId};

/// The oldest link protocol version this executable speaks.
pub const LINK_PROTOCOL_MIN: u32 = 1;

/// The newest link protocol version this executable speaks.
pub const LINK_PROTOCOL_MAX: u32 = 1;

/// The route a hello is posted to.
pub const HELLO_PATH: &str = "/api/v1/mesh/link/hello";

/// The route a sync round is posted to.
pub const SYNC_PATH: &str = "/api/v1/mesh/link/sync";

/// What this executable's links can carry. A peer lacking a feature is told so with
/// [`RefusalCode::FeatureUnsupported`] when an operation needs it of that peer.
pub const FEATURES: &[&str] = &["sessions", "claims", "handovers", "reviews"];

/// The largest link message, serialized, in bytes: under the HTTP server's one-megabyte
/// body bound, so the protocol refuses before the transport has to.
pub const MAX_LINK_MESSAGE: usize = 900 * 1024;

/// The bytes of events one sync message carries at most; the rest follow next round.
pub const SYNC_EVENT_BUDGET: usize = 600 * 1024;

/// How far a hello's or welcome's timestamp may sit from the reader's clock.
pub const MAX_LINK_SKEW_SECONDS: u64 = super::protocol::MAX_CLOCK_SKEW_SECONDS;

/// One request's bound.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The message kinds, each its own signing domain. The domain is mixed into the bytes a
/// signature covers, so a signature made over one kind of message cannot be presented as
/// another: a captured welcome cannot be replayed as a sync answer even though both are
/// signed by the same key.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{sign, verify_signed, Domain};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let hello = sign(&node, Domain::Hello, serde_json::json!({ "proto_min": 1 }));
/// assert!(verify_signed(&node.public.public_key, Domain::Hello, &hello));
/// for other in [Domain::Welcome, Domain::SyncRequest, Domain::SyncAnswer] {
///     assert!(!verify_signed(&node.public.public_key, other, &hello), "domains do not cross");
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// A dialer's hello.
    Hello,
    /// An answerer's welcome.
    Welcome,
    /// A dialer's sync round.
    SyncRequest,
    /// An answerer's sync answer.
    SyncAnswer,
}

impl Domain {
    fn prefix(self) -> &'static [u8] {
        match self {
            Domain::Hello => b"majordomus-mesh-link-hello/v1\n",
            Domain::Welcome => b"majordomus-mesh-link-welcome/v1\n",
            Domain::SyncRequest => b"majordomus-mesh-link-sync-request/v1\n",
            Domain::SyncAnswer => b"majordomus-mesh-link-sync-answer/v1\n",
        }
    }
}

/// A signed message: a JSON body and the Ed25519 signature over its canonical bytes under
/// the message's domain. The body stays JSON so that fields a newer peer adds survive
/// verification here unchanged.
///
/// Keeping the body as JSON rather than as the bytes that were signed is what makes the
/// protocol extensible: a field a newer peer added is carried through verification
/// untouched, because the signature is checked over the canonical form of whatever is
/// there rather than over a shape this version knows.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{sign, verify_signed, Domain, Signed};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let signed: Signed = sign(&node, Domain::SyncRequest, serde_json::json!({ "b": 2, "a": 1 }));
/// assert_eq!(signed.sig.len(), 128, "hex Ed25519");
///
/// // key order is not content, so a round trip through JSON still verifies
/// let wire = serde_json::to_string(&signed).unwrap();
/// let back: Signed = serde_json::from_str(&wire).unwrap();
/// assert!(verify_signed(&node.public.public_key, Domain::SyncRequest, &back));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Signed {
    /// The message.
    pub body: Value,
    /// Hex Ed25519 over the domain and the canonical JSON of `body`.
    pub sig: String,
}

/// Sign `body` as a message of `domain`. The signature is taken over the domain separator
/// followed by the canonical JSON of the body, so two peers that serialize the same facts
/// in different key orders still agree about what was signed, and nothing signed for one
/// exchange verifies as part of another.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{sign, verify_signed, Domain};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let signed = sign(&node, Domain::Welcome, serde_json::json!({ "link": "abc" }));
/// assert!(verify_signed(&node.public.public_key, Domain::Welcome, &signed));
///
/// // the body travels as given; the signature is what is derived
/// assert_eq!(signed.body["link"], "abc");
/// ```
pub fn sign(identity: &NodeIdentity, domain: Domain, body: Value) -> Signed {
    let mut bytes = domain.prefix().to_vec();
    bytes.extend(canonical_json(&body));
    Signed {
        sig: identity.sign(&bytes),
        body,
    }
}

/// Whether `signed` is a message of `domain` signed by `public_key`. Three things have to
/// hold at once — the right key, the right domain, and an untouched body — and this
/// answers about all three with one boolean, because a link has no use for the difference:
/// any of them failing means the message does not come from the peer it claims to.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{sign, verify_signed, Domain};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let impostor = NodeIdentity::ephemeral().unwrap();
/// let mut signed = sign(&node, Domain::Hello, serde_json::json!({ "a": 1, "b": 2 }));
/// assert!(verify_signed(&node.public.public_key, Domain::Hello, &signed));
///
/// // another key did not sign it, and a reordered body is the same body
/// assert!(!verify_signed(&impostor.public.public_key, Domain::Hello, &signed));
/// signed.body = serde_json::json!({ "b": 2, "a": 1 });
/// assert!(verify_signed(&node.public.public_key, Domain::Hello, &signed));
///
/// // an edited one is not
/// signed.body = serde_json::json!({ "b": 3, "a": 1 });
/// assert!(!verify_signed(&node.public.public_key, Domain::Hello, &signed));
/// ```
pub fn verify_signed(public_key: &str, domain: Domain, signed: &Signed) -> bool {
    let mut bytes = domain.prefix().to_vec();
    bytes.extend(canonical_json(&signed.body));
    verify(public_key, &bytes, &signed.sig)
}

/// The version both sides speak: the highest in the intersection of their ranges, or
/// `None` when the ranges do not meet.
///
/// Taking the highest common version is what lets two executables of different ages link
/// without either being told about the other: each states the range it speaks and the
/// newest both understand wins. An inverted range is no range, so a peer that states
/// nonsense is refused rather than accommodated.
///
/// ```
/// use majordomus_cli::mesh::link::{negotiate, LINK_PROTOCOL_MAX, LINK_PROTOCOL_MIN};
///
/// assert_eq!(negotiate(LINK_PROTOCOL_MIN, LINK_PROTOCOL_MAX), Some(LINK_PROTOCOL_MAX));
/// assert_eq!(negotiate(0, 99), Some(LINK_PROTOCOL_MAX), "the newest both speak");
/// assert_eq!(negotiate(LINK_PROTOCOL_MAX + 1, LINK_PROTOCOL_MAX + 3), None, "no overlap");
/// assert_eq!(negotiate(3, 2), None, "an inverted range is no range");
/// ```
pub fn negotiate(theirs_min: u32, theirs_max: u32) -> Option<u32> {
    let low = theirs_min.max(LINK_PROTOCOL_MIN);
    let high = theirs_max.min(LINK_PROTOCOL_MAX);
    (theirs_min <= theirs_max && low <= high).then_some(high)
}

/// What a runtime says about itself on a link: public facts only. The card carries no
/// claim a peer has to take on faith — the node id is derived from the key rather than
/// stated, so a card cannot name a node it cannot sign for, and everything else is either
/// checkable (the repository, the protocol) or advisory (the name, the endpoints).
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{RuntimeCard, FEATURES};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let card = RuntimeCard {
///     pk: node.public.public_key.clone(),
///     runtime: "00000000000000aa".into(),
///     instance: node.public.instance_id.as_str().into(),
///     repo: "r".repeat(32),
///     name: "laptop".into(),
///     version: "0.7.0".into(),
///     features: FEATURES.iter().map(|f| f.to_string()).collect(),
///     endpoints: vec!["10.0.0.2:8742".into()],
/// };
/// assert!(card.check().is_ok());
/// assert_eq!(card.runtime_key().unwrap(), format!("{}-00000000000000aa", node.public.node_id));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeCard {
    /// The node's Ed25519 public key, hex.
    pub pk: String,
    /// The runtime slot, 16 hex.
    pub runtime: String,
    /// This run of the runtime, 16 hex.
    pub instance: String,
    /// The mesh repository id.
    pub repo: String,
    /// The node's display name.
    #[serde(default)]
    pub name: String,
    /// The executable version.
    #[serde(default)]
    pub version: String,
    /// The link features this runtime carries.
    #[serde(default)]
    pub features: Vec<String>,
    /// Where this runtime answers, `host:port`.
    #[serde(default)]
    pub endpoints: Vec<String>,
}

impl RuntimeCard {
    /// The stream this card's runtime writes, when the card is well-formed. The node half
    /// is derived from the key the card carries rather than read from a field, which is
    /// what makes a stream id unforgeable: to write under a stream a runtime must hold the
    /// key that stream is derived from. `None` is the answer for a card whose key, runtime
    /// or instance is not of the right shape — there is no stream for such a card to write.
    ///
    /// ```
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::RuntimeCard;
    /// # let node = NodeIdentity::ephemeral().unwrap();
    /// # let card = RuntimeCard {
    /// #     pk: node.public.public_key.clone(),
    /// #     runtime: "00000000000000aa".into(),
    /// #     instance: node.public.instance_id.as_str().into(),
    /// #     repo: "r".repeat(32),
    /// #     name: String::new(),
    /// #     version: String::new(),
    /// #     features: vec![],
    /// #     endpoints: vec![],
    /// # };
    /// let stream = card.stream().unwrap();
    /// assert!(stream.as_str().starts_with(node.public.node_id.as_str()), "derived from the key");
    ///
    /// // a runtime slot that is not 16 hex is not a runtime slot
    /// let malformed = RuntimeCard { runtime: "zz".into(), ..card };
    /// assert!(malformed.stream().is_none());
    /// ```
    pub fn stream(&self) -> Option<StreamId> {
        let node = node_id_of_key(&self.pk)?;
        StreamId::new(node.as_str(), &self.runtime, &self.instance)
    }

    /// `<node>-<runtime>`, when the card is well-formed: the key the peer table is kept
    /// under. It deliberately drops the instance the stream carries, so that a runtime
    /// which restarts is recognised as the same peer rather than accumulating an entry per
    /// run — the instance is how a restart is noticed, not who the peer is.
    ///
    /// ```
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::RuntimeCard;
    /// # let node = NodeIdentity::ephemeral().unwrap();
    /// # let card = RuntimeCard {
    /// #     pk: node.public.public_key.clone(),
    /// #     runtime: "00000000000000aa".into(),
    /// #     instance: node.public.instance_id.as_str().into(),
    /// #     repo: "r".repeat(32),
    /// #     name: String::new(),
    /// #     version: String::new(),
    /// #     features: vec![],
    /// #     endpoints: vec![],
    /// # };
    /// let restarted = RuntimeCard { instance: "0123456789abcdef".into(), ..card.clone() };
    /// assert_eq!(card.runtime_key(), restarted.runtime_key(), "a restart is the same peer");
    /// assert_ne!(card.stream(), restarted.stream(), "and a different run of it");
    /// ```
    pub fn runtime_key(&self) -> Option<String> {
        self.stream().map(|s| s.runtime_key())
    }

    /// Whether the card's runtime carries `feature`. Features are how two executables of
    /// different ages tell each other what they can do after the protocol version has been
    /// agreed: an operation that needs something the peer does not carry is refused with
    /// [`RefusalCode::FeatureUnsupported`], naming the feature, instead of failing
    /// somewhere further in.
    ///
    /// ```
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::{RuntimeCard, FEATURES};
    /// # let node = NodeIdentity::ephemeral().unwrap();
    /// # let card = RuntimeCard {
    /// #     pk: node.public.public_key.clone(),
    /// #     runtime: "00000000000000aa".into(),
    /// #     instance: node.public.instance_id.as_str().into(),
    /// #     repo: "r".repeat(32),
    /// #     name: String::new(),
    /// #     version: String::new(),
    /// #     features: FEATURES.iter().map(|f| f.to_string()).collect(),
    /// #     endpoints: vec![],
    /// # };
    /// assert!(card.supports("reviews"));
    /// assert!(!card.supports("teleport"), "an unknown word is not a capability");
    /// ```
    pub fn supports(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    /// Refuse an out-of-bounds card: a key that does not parse, ids of the wrong shape,
    /// oversized text.
    ///
    /// Every field of a card was written by somebody else, so each is bounded before any
    /// of it is stored or shown. The bounds are small on purpose — a name, a version, a
    /// handful of features and endpoints — because nothing legitimate needs more, and a
    /// card is the first thing a stranger gets to put into this process.
    ///
    /// ```
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::RuntimeCard;
    /// # let node = NodeIdentity::ephemeral().unwrap();
    /// # let card = RuntimeCard {
    /// #     pk: node.public.public_key.clone(),
    /// #     runtime: "00000000000000aa".into(),
    /// #     instance: node.public.instance_id.as_str().into(),
    /// #     repo: "r".repeat(32),
    /// #     name: String::new(),
    /// #     version: String::new(),
    /// #     features: vec![],
    /// #     endpoints: vec!["10.0.0.2:8742".into()],
    /// # };
    /// assert!(card.check().is_ok());
    /// assert!(RuntimeCard { repo: String::new(), ..card.clone() }.check().is_err());
    /// assert!(RuntimeCard { name: "n".repeat(500), ..card.clone() }.check().is_err());
    /// assert!(
    ///     RuntimeCard { endpoints: vec!["no-port".into()], ..card }.check().is_err(),
    ///     "an endpoint without a port is not somewhere to dial",
    /// );
    /// ```
    pub fn check(&self) -> Result<(), String> {
        if self.stream().is_none() {
            return Err("the key, runtime or instance is not well-formed".into());
        }
        if self.repo.is_empty() || self.repo.len() > 64 {
            return Err("the repository id is empty or oversized".into());
        }
        if self.name.len() > 64
            || self.version.len() > 32
            || self.features.len() > 16
            || self.features.iter().any(|f| f.len() > 32)
            || self.endpoints.len() > super::protocol::MAX_ENDPOINTS
            || self
                .endpoints
                .iter()
                .any(|e| e.len() > 64 || !e.contains(':'))
        {
            return Err("a field of the card is out of bounds".into());
        }
        Ok(())
    }
}

/// A dialer's hello: everything the answerer needs to decide whether to link, in one
/// signed message. The nonce makes the exchange unrepeatable — the welcome must echo it,
/// and a nonce is never accepted twice — and the timestamp makes it unstorable, since a
/// hello captured today cannot be presented tomorrow. The protocol range is stated rather
/// than assumed, so two executables of different ages find their common version here and
/// nowhere else.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{fresh_token, sign, verify_signed, Domain, Hello,
///     RuntimeCard, LINK_PROTOCOL_MAX, LINK_PROTOCOL_MIN};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let hello = Hello {
///     proto_min: LINK_PROTOCOL_MIN,
///     proto_max: LINK_PROTOCOL_MAX,
///     card: RuntimeCard {
///         pk: node.public.public_key.clone(),
///         runtime: "00000000000000aa".into(),
///         instance: node.public.instance_id.as_str().into(),
///         repo: "r".repeat(32),
///         name: String::new(),
///         version: String::new(),
///         features: vec![],
///         endpoints: vec![],
///     },
///     nonce: fresh_token(),
///     ts: 1_700_000_000,
/// };
/// assert_eq!(hello.nonce.len(), 32, "32 hex of fresh entropy, echoed by the welcome");
///
/// let signed = sign(&node, Domain::Hello, serde_json::to_value(&hello).unwrap());
/// assert!(verify_signed(&node.public.public_key, Domain::Hello, &signed));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hello {
    /// The oldest link protocol the dialer speaks.
    pub proto_min: u32,
    /// The newest link protocol the dialer speaks.
    pub proto_max: u32,
    /// The dialer.
    pub card: RuntimeCard,
    /// 32 hex of fresh entropy: the welcome must echo it, and it is never accepted twice.
    pub nonce: String,
    /// The dialer's clock, seconds since the Unix epoch.
    pub ts: u64,
}

/// An answerer's welcome: the mirror of the hello, plus the two things that make the link
/// usable. The link id is what the dialer syncs under afterwards, and the marks let the
/// very first sync carry exactly the events the answerer lacks instead of a round spent
/// discovering that. The echoed nonce is what binds this welcome to that hello: without
/// it, a welcome kept from an earlier exchange would answer a hello it never saw.
///
/// ```
/// use std::collections::BTreeMap;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{fresh_token, sign, verify_signed, Domain, RuntimeCard,
///     Welcome, LINK_PROTOCOL_MAX};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let nonce = fresh_token();
/// let welcome = Welcome {
///     proto: LINK_PROTOCOL_MAX,
///     link: fresh_token(),
///     card: RuntimeCard {
///         pk: node.public.public_key.clone(),
///         runtime: "00000000000000aa".into(),
///         instance: node.public.instance_id.as_str().into(),
///         repo: "r".repeat(32),
///         name: String::new(),
///         version: String::new(),
///         features: vec![],
///         endpoints: vec![],
///     },
///     nonce: nonce.clone(),
///     ts: 1_700_000_000,
///     marks: BTreeMap::new(),
///     heartbeat_seconds: 5,
///     expiry_seconds: 30,
/// };
/// assert_eq!(welcome.nonce, nonce, "this welcome answers that hello and no other");
/// assert!(welcome.marks.is_empty(), "a runtime that has written nothing has nothing to mark");
///
/// let signed = sign(&node, Domain::Welcome, serde_json::to_value(&welcome).unwrap());
/// assert!(verify_signed(&node.public.public_key, Domain::Welcome, &signed));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Welcome {
    /// The negotiated protocol version.
    pub proto: u32,
    /// The link id the dialer syncs under.
    pub link: String,
    /// The answerer.
    pub card: RuntimeCard,
    /// The hello's nonce, echoed: this welcome answers that hello and no other.
    pub nonce: String,
    /// The answerer's clock.
    pub ts: u64,
    /// The answerer's marks, so the first sync carries exactly what it lacks.
    pub marks: Marks,
    /// The answerer's heartbeat, informational.
    pub heartbeat_seconds: u64,
    /// The answerer's expiry, informational.
    pub expiry_seconds: u64,
}

/// A dialer's sync round: what this runtime holds, and what it believes the answerer
/// lacks. Sending the events rather than asking for them is what makes one heartbeat a
/// full replication step in both directions, so replication keeps working when only one
/// side can open a connection. The counter refuses a replayed round and the instance
/// refuses a stale one: a restarted dialer carries a new instance and must say hello
/// again rather than resume a link the answerer no longer has.
///
/// ```
/// use std::collections::BTreeMap;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{fresh_token, sign, verify_signed, Domain, SyncRequest};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let link = fresh_token();
/// let round = SyncRequest {
///     link: link.clone(),
///     counter: 1,
///     instance: node.public.instance_id.as_str().into(),
///     marks: BTreeMap::new(),
///     events: vec![],
/// };
/// let next = SyncRequest { counter: round.counter + 1, ..round.clone() };
/// assert!(next.counter > round.counter, "a round at or below the last accepted is a replay");
/// assert_eq!(next.link, link, "every round of one link travels under its id");
///
/// let signed = sign(&node, Domain::SyncRequest, serde_json::to_value(&next).unwrap());
/// assert!(verify_signed(&node.public.public_key, Domain::SyncRequest, &signed));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SyncRequest {
    /// The link id the welcome gave.
    pub link: String,
    /// Rising per link: a request at or below the last accepted counter is a replay.
    pub counter: u64,
    /// The dialer's instance: a different one is a restarted runtime that must say hello.
    pub instance: String,
    /// The dialer's marks.
    pub marks: Marks,
    /// The events the answerer lacks, by the answerer's last marks.
    pub events: Vec<MeshEvent>,
}

/// An answerer's sync answer: the other half of the round. Its marks are taken *after*
/// ingesting the request's events, so the dialer learns in one exchange both what arrived
/// and what it is still missing. The report travels with it because a silently dropped
/// event is the failure mode this protocol most needs to make visible: a peer that refuses
/// what it is sent says so, with reasons and numbers, rather than staying quietly behind.
///
/// ```
/// use std::collections::BTreeMap;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::IngestReport;
/// use majordomus_cli::mesh::link::{fresh_token, sign, verify_signed, Domain, SyncAnswer};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let answer = SyncAnswer {
///     link: fresh_token(),
///     counter: 7,
///     marks: BTreeMap::new(),
///     events: vec![],
///     report: IngestReport::default(),
/// };
/// assert_eq!(answer.counter, 7, "the request's counter, echoed");
/// assert_eq!(answer.report.rejected_total(), 0, "nothing was refused this round");
///
/// let signed = sign(&node, Domain::SyncAnswer, serde_json::to_value(&answer).unwrap());
/// assert!(verify_signed(&node.public.public_key, Domain::SyncAnswer, &signed));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SyncAnswer {
    /// The link id.
    pub link: String,
    /// The request's counter, echoed.
    pub counter: u64,
    /// The answerer's marks, after ingesting the request's events.
    pub marks: Marks,
    /// The events the dialer lacks, by the request's marks.
    pub events: Vec<MeshEvent>,
    /// What ingesting the request's events did.
    pub report: IngestReport,
}

/// Why a link message was turned down. Every refusal is typed, counted and shown: a mesh
/// that refuses a peer says which rule did, instead of dropping a socket.
///
/// The codes are the diagnosis a person needs when two runtimes will not link, and they
/// are deliberately distinct where the remedies differ: `Untrusted` is a key to add,
/// `RepositoryMismatch` is two different repositories, `SelfLink` is a configuration that
/// dials its own endpoint, and `UnknownLink` is not a failure at all but an instruction to
/// say hello again.
///
/// ```
/// use majordomus_cli::mesh::link::RefusalCode;
///
/// // the wire word is the code's identity across surfaces, and it is stable
/// assert_eq!(RefusalCode::RepositoryMismatch.as_str(), "repository_mismatch");
/// assert_eq!(
///     serde_json::to_value(RefusalCode::Untrusted).unwrap(),
///     serde_json::json!("untrusted"),
/// );
/// assert_ne!(RefusalCode::Untrusted, RefusalCode::RepositoryMismatch);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// Not the shape of the message.
    Malformed,
    /// Over [`MAX_LINK_MESSAGE`].
    Oversized,
    /// The protocol ranges do not meet.
    ProtocolUnsupported,
    /// The signature does not verify.
    Signature,
    /// The timestamp is outside the skew window.
    Stale,
    /// A nonce or a counter already seen.
    Replay,
    /// The peer serves another repository: the isolation rule.
    RepositoryMismatch,
    /// The peer's key is not trusted by this runtime's policy: discovery is not trust.
    Untrusted,
    /// The peer is this very runtime.
    SelfLink,
    /// This runtime's cooperation is not active.
    NotActive,
    /// The link id is unknown here: the answerer restarted or expired the link; say hello.
    UnknownLink,
    /// A bound of this runtime is full.
    Capacity,
    /// The peer lacks a feature the operation needs.
    FeatureUnsupported,
}

impl RefusalCode {
    /// The code's wire word: the snake-case spelling every surface uses, so a refusal
    /// reads the same in a log line, a JSON answer and a peer's counters. It is written
    /// out here rather than derived from the variant name, because the word is protocol
    /// and renaming a variant must not silently change what another runtime reads.
    ///
    /// ```
    /// use majordomus_cli::mesh::link::RefusalCode;
    ///
    /// assert_eq!(RefusalCode::ProtocolUnsupported.as_str(), "protocol_unsupported");
    /// assert_eq!(RefusalCode::SelfLink.as_str(), "self_link");
    /// assert_eq!(
    ///     serde_json::to_value(RefusalCode::SelfLink).unwrap(),
    ///     serde_json::json!(RefusalCode::SelfLink.as_str()),
    ///     "one spelling, whichever surface prints it",
    /// );
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            RefusalCode::Malformed => "malformed",
            RefusalCode::Oversized => "oversized",
            RefusalCode::ProtocolUnsupported => "protocol_unsupported",
            RefusalCode::Signature => "signature",
            RefusalCode::Stale => "stale",
            RefusalCode::Replay => "replay",
            RefusalCode::RepositoryMismatch => "repository_mismatch",
            RefusalCode::Untrusted => "untrusted",
            RefusalCode::SelfLink => "self_link",
            RefusalCode::NotActive => "not_active",
            RefusalCode::UnknownLink => "unknown_link",
            RefusalCode::Capacity => "capacity",
            RefusalCode::FeatureUnsupported => "feature_unsupported",
        }
    }
}

/// A refusal: the code, the reason in words, and the protocol range of the refuser so a
/// version mismatch is diagnosable from either end.
///
/// Carrying the refuser's protocol range in every refusal, not only in the one about
/// protocols, is what makes a version problem visible from the side that has the log: the
/// dialer sees what the answerer speaks without anyone having to read the other machine.
///
/// ```
/// use majordomus_cli::mesh::link::{LinkRefusal, RefusalCode, LINK_PROTOCOL_MAX};
///
/// let refusal = LinkRefusal::new(RefusalCode::Untrusted, "the key is not in the allowlist");
/// assert_eq!(refusal.code, RefusalCode::Untrusted);
/// assert_eq!(refusal.proto_max, LINK_PROTOCOL_MAX);
/// assert_eq!(refusal.to_string(), "untrusted: the key is not in the allowlist");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkRefusal {
    /// The rule that refused.
    pub code: RefusalCode,
    /// Why, in words.
    pub detail: String,
    /// The refuser's oldest protocol.
    pub proto_min: u32,
    /// The refuser's newest protocol.
    pub proto_max: u32,
}

impl LinkRefusal {
    /// A refusal with this executable's protocol range. Every refusal is built here rather
    /// than by hand, so that no call site can forget the range and leave the other end
    /// guessing whether it was turned down over a version or over a rule.
    ///
    /// ```
    /// use majordomus_cli::mesh::link::{LinkRefusal, RefusalCode, LINK_PROTOCOL_MAX,
    ///     LINK_PROTOCOL_MIN};
    ///
    /// let refusal = LinkRefusal::new(RefusalCode::Capacity, "256 peers already linked");
    /// assert_eq!((refusal.proto_min, refusal.proto_max), (LINK_PROTOCOL_MIN, LINK_PROTOCOL_MAX));
    /// assert_eq!(refusal.detail, "256 peers already linked");
    /// ```
    pub fn new(code: RefusalCode, detail: impl Into<String>) -> Self {
        LinkRefusal {
            code,
            detail: detail.into(),
            proto_min: LINK_PROTOCOL_MIN,
            proto_max: LINK_PROTOCOL_MAX,
        }
    }
}

impl std::fmt::Display for LinkRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

/// What a hello or a sync answers over the wire: a signed message or a refusal. Both are
/// successful replies — the transport carries them the same way and neither is an error —
/// because a refusal is a decision this protocol wants recorded and shown, and an answer
/// that arrived as an HTTP failure would be indistinguishable from a peer that is down.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::link::{sign, Domain, LinkReply, RefusalCode};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let accepted = LinkReply::accepted(sign(&node, Domain::Welcome, serde_json::json!({})));
/// assert!(accepted.signed.is_some() && accepted.refusal.is_none());
///
/// let refused = LinkReply::refused(RefusalCode::SelfLink, "this runtime dialled itself");
/// assert!(refused.signed.is_none());
/// assert_eq!(refused.refusal.unwrap().code, RefusalCode::SelfLink);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LinkReply {
    /// The signed welcome or sync answer, when accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed: Option<Signed>,
    /// The refusal, when not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<LinkRefusal>,
}

impl LinkReply {
    /// An accepted reply: the signed welcome or sync answer, and no refusal. The two
    /// halves are built through these constructors rather than by filling the struct in,
    /// so a reply can never be both at once or neither, which is the one shape a reader
    /// would have no sensible way to act on.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::link::{sign, Domain, LinkReply};
    ///
    /// let node = NodeIdentity::ephemeral().unwrap();
    /// let reply = LinkReply::accepted(sign(&node, Domain::SyncAnswer, serde_json::json!({})));
    /// assert!(reply.refusal.is_none(), "an acceptance refuses nothing");
    /// assert!(reply.signed.unwrap().sig.len() == 128);
    /// ```
    pub fn accepted(signed: Signed) -> Self {
        LinkReply {
            signed: Some(signed),
            refusal: None,
        }
    }

    /// A refused reply: the typed code that turned the message down, the reason in words,
    /// and nothing signed. The detail is for the person reading a log and the code is for
    /// the peer deciding what to do next — retry, say hello again, or stop dialling.
    ///
    /// ```
    /// use majordomus_cli::mesh::link::{LinkReply, RefusalCode};
    ///
    /// let reply = LinkReply::refused(RefusalCode::UnknownLink, "say hello again");
    /// assert!(reply.signed.is_none(), "a refusal signs nothing");
    /// let refusal = reply.refusal.unwrap();
    /// assert_eq!(refusal.code, RefusalCode::UnknownLink);
    /// assert_eq!(refusal.detail, "say hello again");
    /// ```
    pub fn refused(code: RefusalCode, detail: impl Into<String>) -> Self {
        LinkReply {
            signed: None,
            refusal: Some(LinkRefusal::new(code, detail)),
        }
    }
}

/// How a link message reaches a peer. The domain above never names a socket: the HTTP
/// adapter is the one the shared server answers, and a test can put two runtimes in one
/// process behind another adapter without changing a line of protocol.
///
/// One method is the whole contract, which is what keeps the protocol testable: the
/// authenticity of a link comes from the signatures inside the messages, never from the
/// channel, so an implementation owes nothing but delivery.
///
/// ```
/// use majordomus_cli::mesh::link::{LinkReply, LinkTransport, RefusalCode, Signed, HELLO_PATH};
///
/// // an adapter that reaches nobody is still a transport; it refuses nothing, it fails
/// struct Unplugged;
/// impl LinkTransport for Unplugged {
///     fn post(&self, endpoint: &str, _path: &str, _message: &Signed) -> Result<LinkReply, String> {
///         Err(format!("{endpoint}: no network here"))
///     }
/// }
///
/// let signed = Signed { body: serde_json::json!({}), sig: String::new() };
/// let error = Unplugged.post("10.0.0.2:8742", HELLO_PATH, &signed).unwrap_err();
/// assert!(error.contains("10.0.0.2:8742"), "a failure names where it was going");
///
/// // a refusal, by contrast, is a reply that arrived
/// let refusal = LinkReply::refused(RefusalCode::Untrusted, "not in the allowlist");
/// assert!(refusal.refusal.is_some());
/// ```
pub trait LinkTransport: Send + Sync {
    /// Post `message` to `path` at `endpoint` (`host:port`) and return the peer's reply.
    /// `Err` is a transport failure — unreachable, timed out, not a reply — never a
    /// refusal; a refusal is an `Ok` reply that says so.
    ///
    /// The distinction is what the caller acts on: a transport failure is a peer to back
    /// off from and try again later, while a refusal is a decision that will be made the
    /// same way next time and is recorded instead of retried.
    ///
    /// ```
    /// use majordomus_cli::mesh::link::{HttpTransport, LinkTransport, Signed, HELLO_PATH};
    ///
    /// let signed = Signed { body: serde_json::json!({}), sig: String::new() };
    /// // port 9 discards: unreachable is a transport failure, not a refusal
    /// let error = HttpTransport.post("127.0.0.1:9", HELLO_PATH, &signed).unwrap_err();
    /// assert!(error.contains("127.0.0.1:9"), "{error}");
    /// ```
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String>;
}

/// The transport the shared server answers: plain HTTP/1.1 through the crate's one
/// minimal client (ADR 0032: no HTTP client dependency, no TLS). A private network or an
/// overlay provides confidentiality; the protocol provides authenticity.
///
/// It refuses an oversized message before opening a socket, so the bound the protocol
/// states is enforced by the sender too and a peer is never asked to read something it
/// would have to refuse. An endpoint may be written with or without a scheme, because a
/// person writing a seed should not have to know which.
///
/// ```
/// use majordomus_cli::mesh::link::{HttpTransport, LinkTransport, Signed, MAX_LINK_MESSAGE,
///     SYNC_PATH};
///
/// let oversized = Signed { body: serde_json::json!("x".repeat(MAX_LINK_MESSAGE)), sig: String::new() };
/// let error = HttpTransport.post("127.0.0.1:9", SYNC_PATH, &oversized).unwrap_err();
/// assert!(error.contains("exceeds"), "refused before the socket: {error}");
/// ```
pub struct HttpTransport;

impl LinkTransport for HttpTransport {
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
        let body = serde_json::to_string(message).map_err(|e| e.to_string())?;
        if body.len() > MAX_LINK_MESSAGE {
            return Err(format!(
                "a link message of {} bytes exceeds {MAX_LINK_MESSAGE}",
                body.len()
            ));
        }
        let base = if endpoint.starts_with("http://") {
            endpoint.to_string()
        } else {
            format!("http://{endpoint}")
        };
        let reply = crate::mcp::bridge::request(
            &base,
            "POST",
            path,
            &[("Content-Type", "application/json")],
            Some(&body),
            REQUEST_TIMEOUT,
        )
        .map_err(|e| format!("{base}{path}: {e}"))?;
        if reply.status != 200 {
            return Err(format!("{base}{path}: status {}", reply.status));
        }
        serde_json::from_str(&reply.body)
            .map_err(|e| format!("{base}{path}: not a link reply: {e}"))
    }
}

/// 32 hex characters of OS entropy: nonces and link ids. Both uses need unpredictability
/// rather than uniqueness alone — a guessable nonce would let a captured hello be answered
/// in advance, and a guessable link id would let a stranger sync under somebody else's
/// link. When the OS has no entropy to give, a clock-derived value keeps the link working
/// and the replay caches still refuse exact repeats.
///
/// ```
/// use majordomus_cli::mesh::link::fresh_token;
///
/// let (a, b) = (fresh_token(), fresh_token());
/// assert_eq!(a.len(), 32);
/// assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
/// assert_ne!(a, b, "a token is fresh, not a counter");
/// ```
pub fn fresh_token() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        // Without entropy a nonce is guessable; a clock-derived fallback keeps the link
        // working and the replay cache still refuses exact repeats.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes = nanos.to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_takes_the_highest_common_version_or_refuses() {
        assert_eq!(negotiate(1, 1), Some(1));
        assert_eq!(negotiate(0, 9), Some(LINK_PROTOCOL_MAX));
        assert_eq!(
            negotiate(LINK_PROTOCOL_MAX + 1, LINK_PROTOCOL_MAX + 3),
            None
        );
        assert_eq!(negotiate(3, 2), None, "an inverted range is no range");
    }

    #[test]
    fn a_signature_covers_the_body_and_its_domain() {
        let node = NodeIdentity::ephemeral().unwrap();
        let other = NodeIdentity::ephemeral().unwrap();
        let mut signed = sign(
            &node,
            Domain::SyncRequest,
            serde_json::json!({"b": 2, "a": 1}),
        );
        assert!(verify_signed(
            &node.public.public_key,
            Domain::SyncRequest,
            &signed
        ));
        assert!(!verify_signed(
            &other.public.public_key,
            Domain::SyncRequest,
            &signed
        ));
        assert!(!verify_signed(
            &node.public.public_key,
            Domain::SyncAnswer,
            &signed
        ));
        // Key order is not content: a reordered body still verifies.
        signed.body = serde_json::json!({"a": 1, "b": 2});
        assert!(verify_signed(
            &node.public.public_key,
            Domain::SyncRequest,
            &signed
        ));
        signed.body = serde_json::json!({"a": 1, "b": 3});
        assert!(!verify_signed(
            &node.public.public_key,
            Domain::SyncRequest,
            &signed
        ));
    }

    #[test]
    fn a_card_is_bounded_and_names_its_runtime() {
        let node = NodeIdentity::ephemeral().unwrap();
        let card = RuntimeCard {
            pk: node.public.public_key.clone(),
            runtime: "00000000000000aa".into(),
            instance: node.public.instance_id.as_str().into(),
            repo: "r".repeat(32),
            name: "n".into(),
            version: "0.7.0".into(),
            features: FEATURES.iter().map(|f| f.to_string()).collect(),
            endpoints: vec!["127.0.0.1:1".into()],
        };
        assert!(card.check().is_ok());
        assert_eq!(
            card.runtime_key().unwrap(),
            format!("{}-00000000000000aa", node.public.node_id)
        );
        assert!(card.supports("claims") && !card.supports("teleport"));
        let broken = RuntimeCard {
            runtime: "zz".into(),
            ..card.clone()
        };
        assert!(broken.check().is_err());
        let spoofed = RuntimeCard {
            pk: "00".repeat(32),
            ..card
        };
        assert_ne!(spoofed.runtime_key(), broken.runtime_key());
    }

    #[test]
    fn tokens_are_fresh_hex() {
        let (a, b) = (fresh_token(), fresh_token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }

    #[test]
    fn an_unreachable_endpoint_is_a_transport_error_not_a_refusal() {
        let node = NodeIdentity::ephemeral().unwrap();
        let signed = sign(&node, Domain::Hello, serde_json::json!({}));
        let error = HttpTransport
            .post("127.0.0.1:9", HELLO_PATH, &signed)
            .unwrap_err();
        assert!(error.contains("127.0.0.1:9"), "{error}");
    }
}
