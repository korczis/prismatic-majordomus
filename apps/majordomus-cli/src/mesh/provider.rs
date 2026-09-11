//! The provider contract: a discovery mechanism is a source of observations and nothing
//! more. A provider watches one transport (a multicast group, a broadcast port, a
//! rendezvous endpoint set), hands every datagram it hears to the manager as raw bytes,
//! and answers for its own health. It parses nothing, verifies nothing, trusts nothing
//! and holds no peers — the manager owns the one verification path and the one registry,
//! which is what makes adding a provider a registration rather than a rewrite.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::NodeIdentity;
use super::protocol::{advertise, Envelope};
use super::registry::Source;
use super::MeshError;

/// One raw observation: where it came from, and the bytes as heard. Parsing and
/// verification happen once, in the manager, for every provider alike.
pub struct Observation {
    /// The provider kind that heard it.
    pub source: Source,
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
pub enum ProviderState {
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
    pub state: ProviderState,
    /// Why, when `Failed`; what it watches, when `Running`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Advertisements transmitted.
    pub sent: u64,
    /// Datagrams heard and handed to the manager.
    pub received: u64,
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
    /// The provider id, stable, snake_case.
    fn id(&self) -> &'static str;
    /// Start the provider's threads. An `Err` marks this provider `Failed` and starts
    /// the others regardless.
    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError>;
    /// The current status.
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
