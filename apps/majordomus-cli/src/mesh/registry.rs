//! The one registry of discovered nodes. Every provider's observation converges here,
//! deduplicated by node id (a digest of the key — never by address), so a node seen over
//! multicast and handed back by a rendezvous is one record with two sources, not two
//! rows with one truth each. Every surface — CLI, HTTP, OpenAPI, MCP, Cockpit — is a
//! projection of this map; nothing else may hold peers.
//!
//! In memory, bounded, and gone with the process, like the peer board beside it: a
//! discovered node is an observation about *now*, and persisting it would only let the
//! registry disagree with the network.
//!
//! ```
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::protocol::advertise;
//! use majordomus_cli::mesh::registry::{MeshRegistry, MeshSource};
//! use majordomus_cli::mesh::trust::TrustState;
//!
//! let registry = MeshRegistry::new();
//! let node = NodeIdentity::ephemeral().unwrap();
//! let heard = advertise(&node, 1, &[], &[], &[], "docs");
//! registry.observe(&heard.adv, node.public.node_id.clone(), TrustState::Observed,
//!     MeshSource::Synthetic, "docs", serde_json::Value::Null);
//! assert_eq!(registry.list().len(), 1);
//! assert_eq!(registry.tallies().accepted, 1);
//! ```

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::{InstanceId, NodeId};
use super::protocol::Advertisement;
use super::trust::TrustState;

/// The most nodes the registry holds. When full, an insertion evicts the longest-unseen
/// untrusted node; trusted nodes are never evicted for space.
pub const MAX_NODES: usize = 256;

/// How long a node stays `present` after its last advertisement.
pub const PRESENCE_TTL: Duration = Duration::from_secs(60);

/// How long an absent node stays listed before it expires out of the registry entirely.
pub const RETENTION: Duration = Duration::from_secs(15 * 60);

/// Where an observation came from. A record accumulates these; provenance is the answer
/// to "why do I see this node".
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MeshSource {
    /// A multicast datagram.
    UdpMulticast,
    /// A broadcast datagram.
    UdpBroadcast,
    /// A rendezvous answer or registration.
    Rendezvous,
    /// A synthetic provider (tests, self-check).
    Synthetic,
}

impl MeshSource {
    /// The source's wire word, as sightings and status listings carry it.
    pub fn as_str(&self) -> &'static str {
        match self {
            MeshSource::UdpMulticast => "udp_multicast",
            MeshSource::UdpBroadcast => "udp_broadcast",
            MeshSource::Rendezvous => "rendezvous",
            MeshSource::Synthetic => "synthetic",
        }
    }
}

/// Whether the node is answering now, by its own announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// An advertisement arrived within [`PRESENCE_TTL`].
    Present,
    /// Nothing within the TTL; the node may be gone or unreachable. It stays listed
    /// until [`RETENTION`] passes, then expires out entirely.
    Absent,
}

/// One provenance entry: a source, and where the observation physically came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sighting {
    /// The provider kind that saw it.
    pub source: MeshSource,
    /// The network path, as text: a sender address, an interface, a rendezvous URL.
    pub path: String,
    /// When, RFC 3339.
    pub at: String,
}

/// One discovered node: the canonical record every surface projects.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeRecord {
    /// The node id: the digest of the key below.
    pub node_id: NodeId,
    /// The Ed25519 public key that proved this record's advertisements.
    pub public_key: String,
    /// The node's own display name. Presentation only.
    pub display_name: String,
    /// The node's current run.
    pub instance_id: InstanceId,
    /// What the policy decided.
    pub trust: TrustState,
    /// Whether the node is answering now.
    pub presence: Presence,
    /// Advertised `host:port` authorities.
    pub endpoints: Vec<String>,
    /// Advertised transports (`http`, `mcp`, `ws`).
    pub capabilities: Vec<String>,
    /// Advertised repository digests.
    pub repositories: Vec<String>,
    /// The advertised protocol version.
    pub protocol_version: u32,
    /// The advertised executable version.
    pub version: String,
    /// Every source this node was seen through, most recent sighting per source.
    pub sources: Vec<Sighting>,
    /// First seen, RFC 3339.
    pub first_seen: String,
    /// Last seen, RFC 3339.
    pub last_seen: String,
    /// How many instances this node id has shown (a restart increments it).
    pub restarts: u64,
}

/// The registry's tallies, for status and health.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
pub struct Tallies {
    /// Records held.
    pub nodes: usize,
    /// Of which trusted.
    pub trusted: usize,
    /// Of which present.
    pub present: usize,
    /// Observations accepted since start.
    pub accepted: u64,
    /// Replayed datagrams dropped since start.
    pub replayed: u64,
    /// Records expired out since start.
    pub expired: u64,
    /// Records evicted for space since start.
    pub evicted: u64,
}

struct Slot {
    record: NodeRecord,
    last_seen: Instant,
    /// The highest sequence seen per instance: a datagram at or below it is a replay.
    last_seq: u64,
    /// The last envelope as heard, signature included, for rendezvous answers: a
    /// candidate travels verbatim so its signature verifies end-to-end at the consumer.
    raw: serde_json::Value,
}

struct Inner {
    slots: BTreeMap<NodeId, Slot>,
    tallies: Tallies,
}

/// The registry. `Mutex<BTreeMap>`, like the peer board: contention is a handful of
/// datagrams a second, and a deterministic order falls out for free.
pub struct MeshRegistry {
    inner: Mutex<Inner>,
}

impl Default for MeshRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshRegistry {
    /// An empty registry; it fills only through [`MeshRegistry::observe`].
    pub fn new() -> Self {
        MeshRegistry {
            inner: Mutex::new(Inner {
                slots: BTreeMap::new(),
                tallies: Tallies::default(),
            }),
        }
    }

    /// The key currently bound to a node id, for trust evaluation.
    pub fn known_key(&self, node: &NodeId) -> Option<String> {
        let inner = self.inner.lock().expect("mesh registry lock");
        inner.slots.get(node).map(|s| s.record.public_key.clone())
    }

    /// The current trust state of a node id.
    pub fn trust_of(&self, node: &NodeId) -> Option<TrustState> {
        let inner = self.inner.lock().expect("mesh registry lock");
        inner.slots.get(node).map(|s| s.record.trust.clone())
    }

    /// Record one verified observation; `raw` is the envelope as heard, kept verbatim
    /// for rendezvous answers. Returns `false` when the datagram was a replay (same
    /// instance, sequence not above the highest seen) and the record was left unchanged.
    pub fn observe(
        &self,
        adv: &Advertisement,
        node_id: NodeId,
        trust: TrustState,
        source: MeshSource,
        path: &str,
        raw: serde_json::Value,
    ) -> bool {
        let now = Instant::now();
        let stamp = crate::peers::rfc3339(std::time::SystemTime::now());
        let mut inner = self.inner.lock().expect("mesh registry lock");
        self.expire_locked(&mut inner, now);
        if let Some(slot) = inner.slots.get_mut(&node_id) {
            let same_instance = slot.record.instance_id == adv.inst;
            if same_instance && adv.seq <= slot.last_seq {
                inner.tallies.replayed += 1;
                return false;
            }
            if !same_instance {
                slot.record.restarts += 1;
                slot.record.instance_id = adv.inst.clone();
            }
            slot.last_seq = adv.seq;
            slot.last_seen = now;
            slot.record.display_name = adv.name.clone();
            slot.record.endpoints = adv.ep.clone();
            slot.record.capabilities = adv.caps.clone();
            slot.record.repositories = adv.repos.clone();
            slot.record.protocol_version = adv.v;
            slot.record.version = adv.ver.clone();
            slot.record.trust = trust;
            slot.record.presence = Presence::Present;
            slot.record.last_seen = stamp.clone();
            slot.raw = raw;
            let sighting = Sighting {
                source,
                path: path.into(),
                at: stamp,
            };
            match slot.record.sources.iter_mut().find(|s| s.source == source) {
                Some(existing) => *existing = sighting,
                None => slot.record.sources.push(sighting),
            }
            slot.record.sources.sort_by_key(|s| s.source);
            inner.tallies.accepted += 1;
            return true;
        }
        // A new node. Make room if the table is full: the longest-unseen untrusted
        // record goes; if every record is trusted, the observation is dropped instead —
        // a full table of trusted nodes is not something an attacker gets to flush.
        if inner.slots.len() >= MAX_NODES {
            let victim = inner
                .slots
                .iter()
                .filter(|(_, s)| !s.record.trust.is_trusted())
                .min_by_key(|(_, s)| s.last_seen)
                .map(|(id, _)| id.clone());
            match victim {
                Some(id) => {
                    inner.slots.remove(&id);
                    inner.tallies.evicted += 1;
                }
                None => return false,
            }
        }
        inner.slots.insert(
            node_id.clone(),
            Slot {
                record: NodeRecord {
                    node_id,
                    public_key: adv.pk.clone(),
                    display_name: adv.name.clone(),
                    instance_id: adv.inst.clone(),
                    trust,
                    presence: Presence::Present,
                    endpoints: adv.ep.clone(),
                    capabilities: adv.caps.clone(),
                    repositories: adv.repos.clone(),
                    protocol_version: adv.v,
                    version: adv.ver.clone(),
                    sources: vec![Sighting {
                        source,
                        path: path.into(),
                        at: stamp.clone(),
                    }],
                    first_seen: stamp.clone(),
                    last_seen: stamp,
                    restarts: 0,
                },
                last_seen: now,
                last_seq: adv.seq,
                raw,
            },
        );
        inner.tallies.accepted += 1;
        true
    }

    /// The envelopes of present, unrejected nodes, verbatim as heard, at most `limit`.
    /// What a rendezvous answers with: each candidate carries its own proof.
    pub fn candidates(&self, limit: usize) -> Vec<serde_json::Value> {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("mesh registry lock");
        self.expire_locked(&mut inner, now);
        inner
            .slots
            .values()
            .filter(|s| {
                now.duration_since(s.last_seen) <= PRESENCE_TTL
                    && !matches!(s.record.trust, TrustState::Rejected(_))
                    && !s.raw.is_null()
            })
            .take(limit)
            .map(|s| s.raw.clone())
            .collect()
    }

    /// Every record, in node-id order, presence recomputed against now.
    pub fn list(&self) -> Vec<NodeRecord> {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("mesh registry lock");
        self.expire_locked(&mut inner, now);
        inner
            .slots
            .values()
            .map(|slot| {
                let mut record = slot.record.clone();
                record.presence = if now.duration_since(slot.last_seen) <= PRESENCE_TTL {
                    Presence::Present
                } else {
                    Presence::Absent
                };
                record
            })
            .collect()
    }

    /// The tallies, presence recomputed against now.
    pub fn tallies(&self) -> Tallies {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("mesh registry lock");
        self.expire_locked(&mut inner, now);
        let mut tallies = inner.tallies;
        tallies.nodes = inner.slots.len();
        tallies.trusted = inner
            .slots
            .values()
            .filter(|s| s.record.trust.is_trusted())
            .count();
        tallies.present = inner
            .slots
            .values()
            .filter(|s| now.duration_since(s.last_seen) <= PRESENCE_TTL)
            .count();
        tallies
    }

    fn expire_locked(&self, inner: &mut Inner, now: Instant) {
        let before = inner.slots.len();
        inner
            .slots
            .retain(|_, slot| now.duration_since(slot.last_seen) <= RETENTION);
        inner.tallies.expired += (before - inner.slots.len()) as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::protocol::advertise;

    fn node() -> (NodeIdentity, Advertisement) {
        let dir = tempfile::tempdir().unwrap();
        let id = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        let env = advertise(
            &id,
            1,
            &["127.0.0.1:8741".into()],
            &["http".into()],
            &[],
            "0.5.0",
        );
        (id, env.adv)
    }

    #[test]
    fn two_sources_converge_into_one_record() {
        let registry = MeshRegistry::new();
        let (id, adv) = node();
        let mut second = adv.clone();
        second.seq = 2;
        assert!(registry.observe(
            &adv,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::UdpMulticast,
            "192.168.1.5:7741",
            serde_json::Value::Null
        ));
        assert!(registry.observe(
            &second,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::Rendezvous,
            "http://127.0.0.1:8741",
            serde_json::Value::Null
        ));
        let listed = registry.list();
        assert_eq!(listed.len(), 1, "one node, not one per source");
        let sources: Vec<MeshSource> = listed[0].sources.iter().map(|s| s.source).collect();
        assert_eq!(
            sources,
            vec![MeshSource::UdpMulticast, MeshSource::Rendezvous]
        );
    }

    #[test]
    fn a_replay_is_dropped_and_counted() {
        let registry = MeshRegistry::new();
        let (id, adv) = node();
        assert!(registry.observe(
            &adv,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::UdpMulticast,
            "a",
            serde_json::Value::Null
        ));
        assert!(!registry.observe(
            &adv,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::UdpMulticast,
            "a",
            serde_json::Value::Null
        ));
        assert_eq!(registry.tallies().replayed, 1);
    }

    #[test]
    fn a_restart_keeps_the_record_and_counts() {
        let registry = MeshRegistry::new();
        let (id, adv) = node();
        registry.observe(
            &adv,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::UdpMulticast,
            "a",
            serde_json::Value::Null,
        );
        // The same node key, a new instance: sequence numbering starts over.
        let dir = tempfile::tempdir().unwrap();
        let restarted = NodeIdentity::load_or_create(&dir.path().join("other.json")).unwrap();
        let mut again = adv.clone();
        again.inst = restarted.public.instance_id.clone();
        again.seq = 1;
        assert!(registry.observe(
            &again,
            id.public.node_id.clone(),
            TrustState::Observed,
            MeshSource::UdpMulticast,
            "a",
            serde_json::Value::Null
        ));
        let listed = registry.list();
        assert_eq!(listed.len(), 1, "a restart is not a second node");
        assert_eq!(listed[0].restarts, 1);
    }

    #[test]
    fn the_table_is_bounded_and_trusted_nodes_survive_pressure() {
        let registry = MeshRegistry::new();
        let (id, adv) = node();
        registry.observe(
            &adv,
            id.public.node_id.clone(),
            TrustState::Trusted("allowlist".into()),
            MeshSource::Synthetic,
            "t",
            serde_json::Value::Null,
        );
        for i in 0..(MAX_NODES + 10) {
            let (other_id, mut other) = node();
            other.seq = 1;
            other.name = format!("n{i}");
            registry.observe(
                &other,
                other_id.public.node_id.clone(),
                TrustState::Observed,
                MeshSource::Synthetic,
                "s",
                serde_json::Value::Null,
            );
        }
        let listed = registry.list();
        assert!(listed.len() <= MAX_NODES);
        assert!(
            listed.iter().any(|r| r.node_id == id.public.node_id),
            "the trusted node was not evicted by untrusted pressure"
        );
        assert!(registry.tallies().evicted > 0);
    }

    #[test]
    fn listing_is_in_node_id_order() {
        let registry = MeshRegistry::new();
        for _ in 0..5 {
            let (id, adv) = node();
            registry.observe(
                &adv,
                id.public.node_id.clone(),
                TrustState::Observed,
                MeshSource::Synthetic,
                "s",
                serde_json::Value::Null,
            );
        }
        let listed = registry.list();
        let mut ids: Vec<String> = listed.iter().map(|r| r.node_id.to_string()).collect();
        let sorted = {
            let mut s = ids.clone();
            s.sort();
            s
        };
        assert_eq!(ids, sorted, "the registry lists in one deterministic order");
        ids.dedup();
        assert_eq!(ids.len(), listed.len());
    }
}
