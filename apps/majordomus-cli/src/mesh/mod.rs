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
//!
//! The whole lifecycle, in memory:
//!
//! ```
//! use majordomus_cli::mesh::{identity::NodeIdentity, protocol};
//!
//! // a node proves itself: sign, encode, parse, verify — the one path every
//! // transport's datagrams take
//! let node = NodeIdentity::ephemeral().unwrap();
//! let envelope = protocol::advertise(&node, 1, &["127.0.0.1:8741".into()],
//!     &["http".into()], &[], "docs");
//! let bytes = protocol::encode(&envelope).unwrap();
//! let heard = protocol::parse(&bytes).expect("a fresh signed envelope verifies");
//! assert_eq!(heard.adv.node_id(), Some(node.public.node_id.clone()));
//!
//! // and hostile input is a refusal, never a panic
//! assert!(protocol::parse(b"garbage").is_err());
//! ```

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
pub use registry::{MeshRegistry, MeshSource, NodeRecord, Presence, Sighting, Tallies};
pub use rendezvous::RegisterAnswer;
pub use trust::{TrustPolicy, TrustState};

/// Why a mesh operation did not happen. Each variant names the layer that refused, so
/// a log line places the failure without a backtrace.
///
/// ```
/// use majordomus_cli::mesh::MeshError;
/// let e = MeshError::Config("an interval of 0 would be a busy loop".into());
/// assert_eq!(e.to_string(), "config: an interval of 0 would be a busy loop");
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_layer_names_itself() {
        for (error, prefix) in [
            (MeshError::Identity("x".into()), "identity:"),
            (MeshError::Protocol("x".into()), "protocol:"),
            (MeshError::Provider("x".into()), "provider:"),
            (MeshError::Config("x".into()), "config:"),
        ] {
            assert!(error.to_string().starts_with(prefix));
        }
    }
}
