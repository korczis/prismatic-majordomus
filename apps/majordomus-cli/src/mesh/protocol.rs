//! The discovery wire protocol: one envelope, every transport. A multicast datagram, a
//! broadcast datagram and a rendezvous candidate all carry the same signed advertisement,
//! so there is one serializer, one parser and one verification path — a second protocol
//! per transport is exactly the five-truths defect ADR 0043 refuses.
//!
//! An advertisement says "a Majordomus node exists, here is its public key, here is where
//! it answers, and the key signs all of it". It grants nothing: trust is evaluated above
//! this module, and authorization does not exist at this layer at all. It never carries a
//! secret, a repository's content, or an environment.
//!
//! The parser is written for hostile input: bounded before it is read, refused before it
//! is trusted, and no input of any shape panics — a property test holds that.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::{node_id_of_key, InstanceId, NodeId, NodeIdentity};
use super::MeshError;

/// The protocol version this executable speaks. A reader accepts exactly this version
/// today; a future version bump is a conscious compatibility decision, not a drift.
pub const PROTOCOL_VERSION: u32 = 1;

/// The largest datagram the protocol sends or reads, in bytes. A packet above this is
/// refused before parsing: the bound is the first defense, not the parser.
pub const MAX_DATAGRAM: usize = 1200;

/// How far an advertisement's timestamp may sit from the reader's clock, in seconds,
/// before the packet is stale (or from the future) and ignored.
pub const MAX_CLOCK_SKEW_SECONDS: u64 = 300;

/// The most endpoints one advertisement may carry.
pub const MAX_ENDPOINTS: usize = 4;

/// The advertised body: everything the signature covers. Field names are one or two
/// characters because this travels in a single UDP datagram; the types above the wire
/// spell things out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Advertisement {
    /// Protocol version.
    pub v: u32,
    /// The Ed25519 public key, hex. The node id is derived from it, never carried:
    /// a field that could disagree with the key is a field somebody will forge.
    pub pk: String,
    /// This run of the node.
    pub inst: InstanceId,
    /// A human name for the node. Presentation only.
    #[serde(default)]
    pub name: String,
    /// Monotonic per instance: a replayed datagram carries a counter the reader has
    /// already seen and is dropped.
    pub seq: u64,
    /// Seconds since the Unix epoch at signing time.
    pub ts: u64,
    /// Where this node answers HTTP, as `host:port` authorities, at most
    /// [`MAX_ENDPOINTS`]. Advisory: an endpoint is where to *try*, identity is the key.
    #[serde(default)]
    pub ep: Vec<String>,
    /// The transports the node serves: `http`, `mcp`, `ws`. Sanitized capability
    /// awareness, never an authorization.
    #[serde(default)]
    pub caps: Vec<String>,
    /// The repositories this node serves, as the same 32-hex git-repository digests
    /// `server.status` reports. A digest discloses nothing about a path; a reader that
    /// serves the same repository recognises it, anyone else learns only "some repo".
    #[serde(default)]
    pub repos: Vec<String>,
    /// The executable version, informational.
    #[serde(default)]
    pub ver: String,
}

/// The wire envelope: the advertisement plus its signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope {
    /// The signed body.
    #[serde(flatten)]
    pub adv: Advertisement,
    /// Ed25519 over [`Advertisement::signing_bytes`], hex.
    pub sig: String,
}

impl Advertisement {
    /// The bytes the signature covers: the canonical JSON of the advertisement alone.
    /// Field order is the struct's, which serde keeps stable, so signer and verifier
    /// serialize identically without a second canonicalization scheme.
    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The node id this advertisement proves, when its key parses.
    pub fn node_id(&self) -> Option<NodeId> {
        node_id_of_key(&self.pk)
    }
}

/// Build and sign this node's advertisement.
#[allow(clippy::too_many_arguments)]
pub fn advertise(
    identity: &NodeIdentity,
    seq: u64,
    endpoints: &[String],
    caps: &[String],
    repos: &[String],
    version: &str,
) -> Envelope {
    let adv = Advertisement {
        v: PROTOCOL_VERSION,
        pk: identity.public.public_key.clone(),
        inst: identity.public.instance_id.clone(),
        name: identity.public.display_name.clone(),
        seq,
        ts: now(),
        ep: endpoints.iter().take(MAX_ENDPOINTS).cloned().collect(),
        caps: caps.to_vec(),
        repos: repos.to_vec(),
        ver: version.into(),
    };
    let sig = identity.sign(&adv.signing_bytes());
    Envelope { adv, sig }
}

/// Encode an envelope for the wire. An envelope that does not fit [`MAX_DATAGRAM`] is an
/// error at the sender — the receiver's bound must never be the first place it is felt.
pub fn encode(envelope: &Envelope) -> Result<Vec<u8>, MeshError> {
    let bytes = serde_json::to_vec(envelope).map_err(|e| MeshError::Protocol(e.to_string()))?;
    if bytes.len() > MAX_DATAGRAM {
        return Err(MeshError::Protocol(format!(
            "an advertisement of {} bytes exceeds the {MAX_DATAGRAM}-byte datagram bound",
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Why a packet was refused. Every reason is a counter somewhere: refusals are the
/// diagnosis of a hostile network, not silence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// Longer than [`MAX_DATAGRAM`].
    Oversized,
    /// Not the JSON shape of an envelope.
    Malformed,
    /// A protocol version this executable does not read.
    Version,
    /// More endpoints than [`MAX_ENDPOINTS`], or an endpoint that is not `host:port`,
    /// or an oversized field.
    Bounds,
    /// The timestamp sits outside [`MAX_CLOCK_SKEW_SECONDS`].
    Stale,
    /// The signature does not verify against the carried key.
    Signature,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = match self {
            Refusal::Oversized => "oversized",
            Refusal::Malformed => "malformed",
            Refusal::Version => "version",
            Refusal::Bounds => "bounds",
            Refusal::Stale => "stale",
            Refusal::Signature => "signature",
        };
        f.write_str(word)
    }
}

/// Parse and verify one datagram. `Ok` means: well-formed, in bounds, current, and the
/// signature proves the key. It does NOT mean trusted — trust policy runs above.
pub fn parse(bytes: &[u8]) -> Result<Envelope, Refusal> {
    parse_at(bytes, now())
}

/// [`parse`] against an explicit clock, so staleness is a tested property rather than a
/// property of the test machine's clock.
pub fn parse_at(bytes: &[u8], clock: u64) -> Result<Envelope, Refusal> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(Refusal::Oversized);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)?;
    let adv = &envelope.adv;
    if adv.v != PROTOCOL_VERSION {
        return Err(Refusal::Version);
    }
    if adv.ep.len() > MAX_ENDPOINTS
        || adv.ep.iter().any(|e| !plausible_authority(e))
        || adv.name.len() > 64
        || adv.caps.len() > 8
        || adv.caps.iter().any(|c| c.len() > 16)
        || adv.repos.len() > 16
        || adv.repos.iter().any(|r| r.len() > 64)
        || adv.ver.len() > 32
    {
        return Err(Refusal::Bounds);
    }
    if adv.ts.abs_diff(clock) > MAX_CLOCK_SKEW_SECONDS {
        return Err(Refusal::Stale);
    }
    if !super::identity::verify(&adv.pk, &adv.signing_bytes(), &envelope.sig) {
        return Err(Refusal::Signature);
    }
    Ok(envelope)
}

/// `host:port` with a numeric port and a non-empty host, nothing more clever. The
/// endpoint is advisory; this bound only keeps garbage out of listings.
fn plausible_authority(text: &str) -> bool {
    if text.len() > 64 {
        return false;
    }
    let Some((host, port)) = text.rsplit_once(':') else {
        return false;
    };
    !host.is_empty() && port.parse::<u16>().is_ok()
}

/// Seconds since the Unix epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> NodeIdentity {
        let dir = tempfile::tempdir().unwrap();
        NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap()
    }

    fn sample(identity: &NodeIdentity) -> Envelope {
        advertise(
            identity,
            1,
            &["127.0.0.1:8741".into()],
            &["http".into(), "mcp".into()],
            &[],
            "0.5.0",
        )
    }

    #[test]
    fn a_signed_advertisement_round_trips() {
        let id = identity();
        let envelope = sample(&id);
        let bytes = encode(&envelope).unwrap();
        assert!(bytes.len() <= MAX_DATAGRAM);
        let parsed = parse(&bytes).expect("a fresh signed envelope parses");
        assert_eq!(parsed.adv, envelope.adv);
        assert_eq!(parsed.adv.node_id(), Some(id.public.node_id.clone()));
    }

    #[test]
    fn a_tampered_envelope_is_refused_as_signature() {
        let id = identity();
        let mut envelope = sample(&id);
        envelope.adv.name = "impostor".into();
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(parse(&bytes), Err(Refusal::Signature));
    }

    #[test]
    fn spoofing_a_key_moves_the_node_id() {
        // A node id is a digest of the key: carrying somebody else's id is impossible
        // because the id is never carried at all.
        let a = identity();
        let b = identity();
        let envelope = sample(&a);
        assert_ne!(envelope.adv.node_id(), Some(b.public.node_id));
    }

    #[test]
    fn an_oversized_packet_is_refused_before_parsing() {
        let bytes = vec![b'x'; MAX_DATAGRAM + 1];
        assert_eq!(parse(&bytes), Err(Refusal::Oversized));
    }

    #[test]
    fn garbage_is_malformed_not_a_panic() {
        assert_eq!(parse(b"not json"), Err(Refusal::Malformed));
        assert_eq!(parse(b""), Err(Refusal::Malformed));
        assert_eq!(parse(b"{}"), Err(Refusal::Malformed));
    }

    #[test]
    fn a_wrong_version_is_refused_without_a_signature_check() {
        let id = identity();
        let mut envelope = sample(&id);
        envelope.adv.v = 99;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(parse(&bytes), Err(Refusal::Version));
    }

    #[test]
    fn a_stale_timestamp_is_ignored() {
        let id = identity();
        let envelope = sample(&id);
        let bytes = encode(&envelope).unwrap();
        assert_eq!(
            parse_at(&bytes, envelope.adv.ts + MAX_CLOCK_SKEW_SECONDS + 1),
            Err(Refusal::Stale)
        );
        assert!(parse_at(&bytes, envelope.adv.ts + 5).is_ok());
    }

    #[test]
    fn endpoint_bounds_hold() {
        let id = identity();
        let mut envelope = sample(&id);
        envelope.adv.ep = vec!["no-port".into()];
        let sig = id.sign(&envelope.adv.signing_bytes());
        envelope.sig = sig;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(parse(&bytes), Err(Refusal::Bounds));
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        // The deterministic edge of the fuzz property; the proptest below widens it.
        for bytes in [
            &[0xffu8, 0xfe, 0x00][..],
            b"[1,2,3]",
            b"{\"v\":1}",
            "🦀🦀🦀".as_bytes(),
        ] {
            let _ = parse(bytes);
        }
    }

    proptest::proptest! {
        #[test]
        fn parse_never_panics_on_any_input(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..1400)) {
            let _ = parse(&bytes);
        }
    }
}
