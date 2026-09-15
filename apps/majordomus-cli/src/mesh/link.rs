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

/// The message kinds, each its own signing domain.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Signed {
    /// The message.
    pub body: Value,
    /// Hex Ed25519 over the domain and the canonical JSON of `body`.
    pub sig: String,
}

/// Sign `body` as a message of `domain`.
pub fn sign(identity: &NodeIdentity, domain: Domain, body: Value) -> Signed {
    let mut bytes = domain.prefix().to_vec();
    bytes.extend(canonical_json(&body));
    Signed {
        sig: identity.sign(&bytes),
        body,
    }
}

/// Whether `signed` is a message of `domain` signed by `public_key`.
pub fn verify_signed(public_key: &str, domain: Domain, signed: &Signed) -> bool {
    let mut bytes = domain.prefix().to_vec();
    bytes.extend(canonical_json(&signed.body));
    verify(public_key, &bytes, &signed.sig)
}

/// The version both sides speak: the highest in the intersection of their ranges, or
/// `None` when the ranges do not meet.
pub fn negotiate(theirs_min: u32, theirs_max: u32) -> Option<u32> {
    let low = theirs_min.max(LINK_PROTOCOL_MIN);
    let high = theirs_max.min(LINK_PROTOCOL_MAX);
    (theirs_min <= theirs_max && low <= high).then_some(high)
}

/// What a runtime says about itself on a link: public facts only.
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
    /// The stream this card's runtime writes, when the card is well-formed.
    pub fn stream(&self) -> Option<StreamId> {
        let node = node_id_of_key(&self.pk)?;
        StreamId::new(node.as_str(), &self.runtime, &self.instance)
    }

    /// `<node>-<runtime>`, when the card is well-formed.
    pub fn runtime_key(&self) -> Option<String> {
        self.stream().map(|s| s.runtime_key())
    }

    /// Whether the card's runtime carries `feature`.
    pub fn supports(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    /// Refuse an out-of-bounds card: a key that does not parse, ids of the wrong shape,
    /// oversized text.
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

/// A dialer's hello.
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

/// An answerer's welcome.
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

/// A dialer's sync round.
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

/// An answerer's sync answer.
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
    /// The code's wire word.
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
    /// A refusal with this executable's protocol range.
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

/// What a hello or a sync answers over the wire: a signed message or a refusal.
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
    /// An accepted reply.
    pub fn accepted(signed: Signed) -> Self {
        LinkReply {
            signed: Some(signed),
            refusal: None,
        }
    }

    /// A refused reply.
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
pub trait LinkTransport: Send + Sync {
    /// Post `message` to `path` at `endpoint` (`host:port`) and return the peer's reply.
    /// `Err` is a transport failure — unreachable, timed out, not a reply — never a
    /// refusal; a refusal is an `Ok` reply that says so.
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String>;
}

/// The transport the shared server answers: plain HTTP/1.1 through the crate's one
/// minimal client (ADR 0032: no HTTP client dependency, no TLS). A private network or an
/// overlay provides confidentiality; the protocol provides authenticity.
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

/// 32 hex characters of OS entropy: nonces and link ids.
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
