//! The provider contract: a discovery mechanism is a source of observations and nothing
//! more. A provider watches one transport (a multicast group, a broadcast port, a
//! rendezvous endpoint set), hands every datagram it hears to the manager as raw bytes,
//! and answers for its own health. It parses nothing, verifies nothing, trusts nothing
//! and holds no peers — the manager owns the one verification path and the one registry,
//! which is what makes adding a provider a registration rather than a rewrite.
//!
//! ```
//! use std::sync::Arc;
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::provider::Beacon;
//!
//! // The beacon signs this node's advertisements with one rising sequence, so every
//! // transmitting provider shares the replay window.
//! let identity = NodeIdentity::ephemeral().unwrap();
//! let beacon = Beacon::new(Arc::new(identity), vec!["127.0.0.1:8741".into()],
//!     vec!["http".into()], vec![], "docs");
//! let first = beacon.next_envelope();
//! let second = beacon.next_envelope();
//! assert!(second.adv.seq > first.adv.seq, "the sequence only rises");
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::NodeIdentity;
use super::protocol::{advertise, Envelope};
use super::registry::MeshSource;
use super::MeshError;

/// One raw observation: where it came from, and the bytes as heard. Parsing and
/// verification happen once, in the manager, for every provider alike.
pub struct Observation {
    /// The provider kind that heard it.
    pub source: MeshSource,
    /// The network path, as text: a sender address, a rendezvous URL.
    pub path: String,
    /// The datagram, unparsed and untrusted.
    pub bytes: Vec<u8>,
}

/// What this node says about itself: the signer plus the advertised facts, with one
/// monotonically increasing sequence shared by every transmitting provider — a reader
/// deduplicates by (instance, seq) regardless of which transport delivered first.
pub struct Beacon {
    identity: Arc<NodeIdentity>,
    seq: AtomicU64,
    endpoints: Vec<String>,
    caps: Vec<String>,
    repos: Vec<String>,
    version: String,
}

impl Beacon {
    /// A beacon over this node's identity and advertised facts.
    pub fn new(
        identity: Arc<NodeIdentity>,
        endpoints: Vec<String>,
        caps: Vec<String>,
        repos: Vec<String>,
        version: &str,
    ) -> Self {
        Beacon {
            identity,
            seq: AtomicU64::new(0),
            endpoints,
            caps,
            repos,
            version: version.into(),
        }
    }

    /// The next signed envelope, sequence advanced.
    pub fn next_envelope(&self) -> Envelope {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        advertise(
            &self.identity,
            seq,
            &self.endpoints,
            &self.caps,
            &self.repos,
            &self.version,
        )
    }

    /// The public identity behind the beacon.
    pub fn identity(&self) -> &NodeIdentity {
        &self.identity
    }
}

/// The provider's health, as every surface reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshProviderState {
    /// Threads up, socket bound (where the provider has one).
    Running,
    /// The provider could not start or died; `ProviderStatus::detail` says why. One
    /// provider failing never stops another — failure isolation is the contract.
    Failed,
    /// Stopped by the manager.
    Stopped,
}

/// One provider's status: state, why, and its counters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderStatus {
    /// The provider id: `udp_multicast`, `udp_broadcast`, `rendezvous`, `synthetic`.
    pub id: String,
    /// The state.
    pub state: MeshProviderState,
    /// Why, when `Failed`; what it watches, when `Running`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Advertisements transmitted.
    pub sent: u64,
    /// Datagrams heard and handed to the manager.
    pub received: u64,
}

/// A provider id is unique in a mesh, so it is both what a reader looks for and the
/// identity that makes the status list total.
impl crate::order::Ordered for ProviderStatus {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id)
    }
}

/// The counters a provider's threads share with its status.
#[derive(Default)]
pub struct Counters {
    /// Advertisements transmitted.
    pub sent: AtomicU64,
    /// Datagrams heard.
    pub received: AtomicU64,
}

/// What a provider gets to work with: the channel to the manager, the shared stop flag,
/// and the beacon it announces.
pub struct ProviderContext {
    /// Where observations go.
    pub tx: Sender<Observation>,
    /// Set once by the manager; every provider thread ends at its next bounded wait.
    pub stop: Arc<AtomicBool>,
    /// What this node announces.
    pub beacon: Arc<Beacon>,
}

/// A discovery mechanism. Implementing this — and handing an instance to the manager —
/// is the whole registration: no consumer, surface or registry learns provider names.
pub trait MeshProvider: Send {
    /// The provider id, stable, snake_case: the word status listings and sighting
    /// provenance carry for this mechanism.
    fn id(&self) -> &'static str;
    /// Start the provider's threads. An `Err` marks this provider `Failed` and starts
    /// the others regardless.
    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError>;
    /// The current status: state, the reason or subject, and this provider's counters.
    fn status(&self) -> ProviderStatus;
}

/// A deterministic small jitter in `0..bound` milliseconds, from OS entropy; zero when
/// entropy is unavailable — jitter is a nicety, never a dependency.
pub fn jitter_ms(bound: u64) -> u64 {
    if bound == 0 {
        return 0;
    }
    let mut b = [0u8; 8];
    if getrandom::getrandom(&mut b).is_err() {
        return 0;
    }
    u64::from_le_bytes(b) % bound
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn the_beacon_signs_with_one_rising_sequence() {
        let identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        let node = identity.public.node_id.clone();
        let beacon = Beacon::new(Arc::new(identity), vec![], vec![], vec![], "test");
        let a = beacon.next_envelope();
        let b = beacon.next_envelope();
        assert_eq!(a.adv.seq + 1, b.adv.seq);
        assert_eq!(a.adv.node_id(), Some(node));
    }

    #[test]
    fn jitter_stays_under_its_bound_and_zero_is_zero() {
        assert_eq!(jitter_ms(0), 0);
        for _ in 0..32 {
            assert!(jitter_ms(50) < 50);
        }
    }
}
