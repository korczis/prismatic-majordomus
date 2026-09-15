//! The mesh declaration: whether this repository's servers participate in discovery, and
//! how. One typed object of the `.ai/` layer (kind `mesh`, schema `mesh/v1`), parsed
//! here and nowhere else. No declaration, or `enabled: false`, means the mesh is off and
//! not one socket opens — the repository's default posture ("nothing leaves the
//! machine") holds until a person commits the object that says otherwise.
//!
//! ```
//! use majordomus_cli::mesh::config::{MeshConfig, BroadcastMode};
//!
//! // The defaults are the safe ones: multicast prepared, broadcast off, nobody trusted.
//! let minimal: MeshConfig = serde_json::from_value(serde_json::json!({
//!     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": false
//! })).unwrap();
//! assert!(!minimal.enabled);
//! assert_eq!(minimal.broadcast.mode, BroadcastMode::Disabled);
//! assert!(minimal.trust.allow.is_empty());
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Object;

use super::trust::TrustPolicy;
use super::MeshError;

/// The kind the declaration is discovered under.
pub const KIND: &str = "mesh-declaration";

/// The schema version this executable reads.
pub const SCHEMA_VERSION: &str = "mesh/v1";

/// The default multicast group: an address in the IPv4 organization-local scope
/// (239.0.0.0/8, RFC 2365), chosen once here.
pub const DEFAULT_GROUP: &str = "239.255.77.77";

/// The default UDP port for multicast and broadcast datagrams.
pub const DEFAULT_PORT: u16 = 7741;

/// The default seconds between announcements.
pub const DEFAULT_INTERVAL: u64 = 15;

/// The whole declaration: the master switch and one section per mechanism. Unknown
/// keys are refused, so a typo is an error and not a silently ignored wish.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshConfig {
    /// The format version: `mesh/v1`.
    pub schema: String,
    /// The kind: `mesh`, the same word the discovery class carries.
    pub kind: String,
    /// The declaration's identity.
    pub id: String,
    /// The master switch. `false` starts nothing.
    #[serde(default)]
    pub enabled: bool,
    /// UDP multicast, the preferred LAN mechanism.
    #[serde(default)]
    pub multicast: MulticastConfig,
    /// UDP broadcast, the controlled fallback.
    #[serde(default)]
    pub broadcast: BroadcastConfig,
    /// Rendezvous endpoints, for networks multicast cannot cross.
    #[serde(default)]
    pub rendezvous: RendezvousConfig,
    /// Who to trust.
    #[serde(default)]
    pub trust: TrustConfig,
    /// Cooperation: the authenticated links to trusted runtimes of the same repository,
    /// and the journal they replicate (ADR 0067).
    #[serde(default)]
    pub cooperation: CooperationConfig,
}

/// Cooperation settings: whether this runtime links to the trusted runtimes of its
/// repository, how often it proves it is alive, when a silent runtime counts as gone, and
/// which endpoints to dial when discovery cannot find them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CooperationConfig {
    /// Whether links open at all. On by default once the mesh is enabled: a link needs a
    /// trusted peer of the same repository, so under the default `deny_unknown` policy
    /// with an empty allowlist nothing links until a person lists a key.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Seconds between heartbeats: one beat and one sync per link per interval.
    #[serde(default = "default_heartbeat")]
    pub heartbeat_seconds: u64,
    /// Seconds of silence after which a runtime is expired: its sessions end and its
    /// claims stop excluding. At least three heartbeats, so one lost round never expires
    /// anyone.
    #[serde(default = "default_expiry")]
    pub expiry_seconds: u64,
    /// Endpoints (`http://host:port`) to dial whether or not discovery heard them: a
    /// static fleet, or a segment neither multicast nor a rendezvous reaches.
    #[serde(default)]
    pub seeds: Vec<String>,
    /// The repository's mesh identity, declared rather than derived from its root commits:
    /// what a shallow clone needs, and what separates two repositories sharing a history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

impl Default for CooperationConfig {
    fn default() -> Self {
        CooperationConfig {
            enabled: true,
            heartbeat_seconds: DEFAULT_HEARTBEAT,
            expiry_seconds: DEFAULT_EXPIRY,
            seeds: Vec::new(),
            repository: None,
        }
    }
}

/// The default seconds between cooperation heartbeats.
pub const DEFAULT_HEARTBEAT: u64 = 5;

/// The default seconds of silence before a runtime expires.
pub const DEFAULT_EXPIRY: u64 = 30;

fn default_heartbeat() -> u64 {
    DEFAULT_HEARTBEAT
}
fn default_expiry() -> u64 {
    DEFAULT_EXPIRY
}

/// Multicast settings: the group, the port, how far a datagram travels and how often
/// this node announces.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MulticastConfig {
    /// Whether the multicast provider runs.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The group, dotted IPv4 in 239.0.0.0/8 by convention.
    #[serde(default = "default_group")]
    pub group: String,
    /// The UDP port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// The datagram TTL; 1 keeps announcements on the local segment.
    #[serde(default = "one")]
    pub ttl: u32,
    /// Seconds between announcements (a jitter is added).
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
}

impl Default for MulticastConfig {
    fn default() -> Self {
        MulticastConfig {
            enabled: true,
            group: DEFAULT_GROUP.into(),
            port: DEFAULT_PORT,
            ttl: 1,
            interval_seconds: DEFAULT_INTERVAL,
        }
    }
}

/// Broadcast mode: off, automatic (the limited broadcast address), or explicit networks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastMode {
    /// No broadcast datagrams.
    Disabled,
    /// Announce to 255.255.255.255.
    Auto,
    /// Announce to the listed directed broadcast addresses.
    Explicit,
}

/// Broadcast settings: the fallback's mode, destinations and cadence; `disabled`
/// unless a person declared otherwise.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BroadcastConfig {
    /// The mode; `disabled` unless somebody decided otherwise.
    #[serde(default = "disabled")]
    pub mode: BroadcastMode,
    /// The UDP port (the same protocol, one port).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Directed broadcast addresses for `explicit` mode, `a.b.c.255` shaped.
    #[serde(default)]
    pub networks: Vec<String>,
    /// Seconds between announcements.
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
}

impl Default for BroadcastConfig {
    fn default() -> Self {
        BroadcastConfig {
            mode: BroadcastMode::Disabled,
            port: DEFAULT_PORT,
            networks: Vec::new(),
            interval_seconds: DEFAULT_INTERVAL,
        }
    }
}

/// Rendezvous settings: other Majordomus servers to register with. Plain HTTP on a
/// private network; there is no TLS in this executable, and the protocol assumes none —
/// candidates verify end-to-end by signature, a poisoned rendezvous can withhold nodes
/// but not invent them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RendezvousConfig {
    /// Base URLs, `http://host:port`. Several endpoints are several independent
    /// sources; one failing never stops another.
    #[serde(default)]
    pub endpoints: Vec<String>,
    /// Seconds between registrations per endpoint (backoff stretches it on failure).
    #[serde(default = "rendezvous_interval")]
    pub interval_seconds: u64,
}

impl Default for RendezvousConfig {
    fn default() -> Self {
        RendezvousConfig {
            endpoints: Vec::new(),
            interval_seconds: rendezvous_interval(),
        }
    }
}

/// Trust settings: the policy, and the public keys trusted regardless of it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustConfig {
    /// The policy; `deny_unknown` unless somebody decided otherwise.
    #[serde(default)]
    pub policy: TrustPolicy,
    /// Ed25519 public keys (hex) trusted regardless of policy.
    #[serde(default)]
    pub allow: Vec<String>,
}

fn yes() -> bool {
    true
}
fn one() -> u32 {
    1
}
fn disabled() -> BroadcastMode {
    BroadcastMode::Disabled
}
fn default_group() -> String {
    DEFAULT_GROUP.into()
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_interval() -> u64 {
    DEFAULT_INTERVAL
}
fn rendezvous_interval() -> u64 {
    60
}

impl CooperationConfig {
    /// Refuse the settings that cannot work: a zero heartbeat, an expiry under three
    /// heartbeats (one lost round would expire a live runtime), a seed that is not
    /// `http://host:port`, an empty or oversized declared repository.
    ///
    /// ```
    /// use majordomus_cli::mesh::config::CooperationConfig;
    /// assert!(CooperationConfig::default().validate().is_ok());
    /// let flapping = CooperationConfig { heartbeat_seconds: 10, expiry_seconds: 15, ..Default::default() };
    /// assert!(flapping.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if self.heartbeat_seconds == 0 {
            return Err("cooperation.heartbeat_seconds of 0 would be a busy loop".into());
        }
        if self.expiry_seconds < 3 * self.heartbeat_seconds {
            return Err(format!(
                "cooperation.expiry_seconds {} is under three heartbeats ({}): one lost round would expire a live runtime",
                self.expiry_seconds,
                3 * self.heartbeat_seconds
            ));
        }
        for seed in &self.seeds {
            let authority = seed.strip_prefix("http://").unwrap_or_default();
            if authority.is_empty() || !authority.contains(':') || authority.contains('/') {
                return Err(format!("cooperation seed '{seed}' is not http://host:port"));
            }
        }
        if let Some(repository) = &self.repository {
            if repository.trim().is_empty() || repository.len() > 128 {
                return Err("cooperation.repository is empty or over 128 characters".into());
            }
        }
        Ok(())
    }
}

impl MeshConfig {
    /// Parse one indexed object into a mesh declaration, or say why it is not one:
    /// the wrong schema version, an unknown key, and a zero interval are each refused
    /// with the object's URI in the reason.
    pub fn parse(object: &Object) -> Result<MeshConfig, MeshError> {
        let parsed: MeshConfig = serde_json::from_value(object.metadata.clone())
            .map_err(|e| MeshError::Config(format!("{}: {e}", object.uri)))?;
        if parsed.schema != SCHEMA_VERSION {
            return Err(MeshError::Config(format!(
                "{}: schema {} is not {SCHEMA_VERSION}",
                object.uri, parsed.schema
            )));
        }
        if parsed.kind != KIND {
            return Err(MeshError::Config(format!(
                "{}: kind {} is not {KIND}",
                object.uri, parsed.kind
            )));
        }
        if parsed.multicast.interval_seconds == 0
            || parsed.broadcast.interval_seconds == 0
            || parsed.rendezvous.interval_seconds == 0
        {
            return Err(MeshError::Config(format!(
                "{}: an interval of 0 would be a busy loop",
                object.uri
            )));
        }
        parsed
            .cooperation
            .validate()
            .map_err(|e| MeshError::Config(format!("{}: {e}", object.uri)))?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(metadata: serde_json::Value) -> Object {
        Object {
            kind: KIND.into(),
            identity: "majordomus".into(),
            uri: "majordomus://mesh-declaration/majordomus".into(),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: ".ai/repo/mesh/majordomus.yaml".into(),
                directory: ".ai/repo/mesh".into(),
                source_class: KIND.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn a_minimal_declaration_parses_with_safe_defaults() {
        let config = MeshConfig::parse(&object(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "majordomus", "enabled": true
        })))
        .unwrap();
        assert!(config.enabled);
        assert!(config.multicast.enabled);
        assert_eq!(config.multicast.group, DEFAULT_GROUP);
        assert_eq!(config.broadcast.mode, BroadcastMode::Disabled);
        assert!(config.rendezvous.endpoints.is_empty());
        assert_eq!(config.trust.policy, TrustPolicy::DenyUnknown);
    }

    #[test]
    fn a_wrong_schema_and_an_unknown_key_are_refused() {
        assert!(MeshConfig::parse(&object(serde_json::json!({
            "schema": "mesh/v2", "kind": "mesh", "id": "x"
        })))
        .is_err());
        assert!(MeshConfig::parse(&object(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "surprise": 1
        })))
        .is_err());
    }

    #[test]
    fn a_zero_interval_is_refused_as_a_busy_loop() {
        assert!(MeshConfig::parse(&object(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x",
            "multicast": {"interval_seconds": 0}
        })))
        .is_err());
    }
}
