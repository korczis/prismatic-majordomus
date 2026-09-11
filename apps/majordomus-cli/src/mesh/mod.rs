//! The mesh: how one running Majordomus learns that others exist, and proves which one
//! it is. ADR 0043 is the decision; this module is the whole implementation, and its
//! boundaries are the point:
//!
//! - **Identity is a key, not an address.** A node is an Ed25519 keypair kept per user
//!   and machine ([`identity`]); the node id is a digest of the public key. Hostnames,
//!   IPs, ports and PIDs are runtime attributes and never identity.
//! - **One protocol** ([`protocol`]): a compact, versioned, bounded, signed envelope,
//!   the same over multicast, broadcast and rendezvous. Parsing never panics; hostile
//!   input is a counted refusal.
//! - **Providers observe, the manager decides** ([`provider`], [`manager`]): a
//!   discovery mechanism hands raw bytes up and answers for its own health. The manager
//!   owns the one verification path and the one [`registry`]; adding a mechanism is
//!   implementing the provider contract, not editing consumers.
//! - **Discovery creates awareness, not authority** ([`trust`]): a valid unknown node
//!   is observed and trusted for nothing; even a trusted node gains no execution
//!   rights — there is no remote execution to gain. The default policy is
//!   `deny_unknown`, and the mesh itself is off until the repository commits a
//!   declaration ([`config`]) that says otherwise: the "nothing leaves the machine"
//!   posture holds by default.
//! - **Every surface is a projection.** CLI, HTTP, OpenAPI, MCP and the Cockpit read
//!   [`MeshRuntime`] and the registry; none holds peers of its own.
//!
//! Nothing here writes to the repository. The identity file lives under the user's
//! state directory; the registry lives in process memory and dies with it.

pub mod broadcast;
pub mod config;
pub mod doctor;
pub mod identity;
pub mod manager;
pub mod multicast;
pub mod protocol;
pub mod provider;
pub mod registry;
pub mod rendezvous;
pub mod trust;

pub use config::{MeshConfig, KIND};
pub use doctor::{DoctorCheck, MeshDoctorReport};
pub use identity::{default_identity_path, InstanceId, NodeId, NodeIdentity, PublicIdentity};
pub use manager::{MeshRuntime, MeshStatus, Refusals};
pub use protocol::{Advertisement, Envelope};
pub use provider::{MeshProvider, Observation, ProviderContext, ProviderStatus};
pub use registry::{MeshRegistry, NodeRecord, Presence, Sighting, Source, Tallies};
pub use rendezvous::RegisterAnswer;
pub use trust::{TrustPolicy, TrustState};

/// Why a mesh operation did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeshError {
    /// The identity file: unreadable, malformed, or nowhere to keep one.
    #[error("identity: {0}")]
    Identity(String),
    /// The protocol: an envelope that cannot be built or does not fit.
    #[error("protocol: {0}")]
    Protocol(String),
    /// A provider: a socket, a group, an endpoint.
    #[error("provider: {0}")]
    Provider(String),
    /// The declaration: missing, malformed, or a version this executable does not read.
    #[error("config: {0}")]
    Config(String),
}
