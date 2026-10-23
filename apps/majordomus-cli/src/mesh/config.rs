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
///
/// Every field has a default that is safe on its own, so a declaration that says nothing
/// about cooperation still behaves as intended: links are on, but a link needs a trusted
/// peer of the same repository, and under the default trust policy there is none until a
/// person lists a key.
///
/// ```
/// use majordomus_cli::mesh::config::{CooperationConfig, DEFAULT_EXPIRY, DEFAULT_HEARTBEAT};
///
/// let default = CooperationConfig::default();
/// assert!(default.enabled && default.seeds.is_empty() && default.repository.is_none());
/// assert!(
///     default.expiry_seconds >= 3 * default.heartbeat_seconds,
///     "one lost round must never expire a runtime",
/// );
///
/// // and what a declaration leaves out, it gets
/// let stated: CooperationConfig =
///     serde_json::from_value(serde_json::json!({ "seeds": ["http://10.0.0.2:8742"] })).unwrap();
/// assert_eq!(stated.seeds, ["http://10.0.0.2:8742"]);
/// assert_eq!(stated.heartbeat_seconds, DEFAULT_HEARTBEAT);
/// assert_eq!(stated.expiry_seconds, DEFAULT_EXPIRY);
/// ```
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
    /// Endpoints (`https://host[:port]` or `http://host:port`) to dial whether or not
    /// discovery heard them: a static fleet, or a segment neither multicast nor a rendezvous
    /// reaches.
    #[serde(default)]
    pub seeds: Vec<String>,
    /// The repository's mesh identity, declared rather than derived from its root commits:
    /// what a shallow clone needs, and what separates two repositories sharing a history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Seconds a session holding an exclusive claim may make no progress — no real work,
    /// no `mesh beat` — before its exclusive claims are suspended: still held, refusing
    /// nobody until it beats again; its session is not closed. 0 turns the rule off (I3990).
    /// A runtime's beat proves the runtime is up and nothing about the worker behind it:
    /// the 5d executor held the integration lease for hours waiting on nobody while its
    /// server beat (2026-10-22). At least four heartbeats, so a working session is beaten
    /// for several times inside one ttl, and at most an hour. Left out of the repository's
    /// declaration on purpose: an older executable refuses a key it does not know.
    #[serde(default = "default_session_ttl")]
    pub session_ttl_seconds: u64,
}

impl Default for CooperationConfig {
    fn default() -> Self {
        CooperationConfig {
            enabled: true,
            heartbeat_seconds: DEFAULT_HEARTBEAT,
            expiry_seconds: DEFAULT_EXPIRY,
            seeds: Vec::new(),
            repository: None,
            session_ttl_seconds: DEFAULT_SESSION_TTL,
        }
    }
}

/// The default seconds a claiming session may make no progress before its exclusive claims
/// are suspended:
/// ten minutes — longer than any one tool call a working session waits on, far shorter
/// than the hours a session stuck on a dialog used to hold a scope (I3990).
pub const DEFAULT_SESSION_TTL: u64 = 600;

fn default_session_ttl() -> u64 {
    DEFAULT_SESSION_TTL
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

/// Rendezvous settings: other Majordomus servers to register with. An `https://` hub is
/// reached over TLS with its certificate verified; an `http://` hub only at an address that is
/// loopback, private or an overlay's, because the client refuses cleartext to a public one
/// (ADR 0220). Either way candidates verify end-to-end by signature: a poisoned rendezvous can
/// withhold nodes but not invent them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RendezvousConfig {
    /// Base URLs, `https://host[:port]` or `http://host:port` (a bare `host:port` is http).
    /// Several endpoints are several independent sources; one failing never stops another.
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
    /// heartbeats (one lost round would expire a live runtime), a seed that is neither
    /// `https://host[:port]` nor `http://host:port`, an empty or oversized declared repository.
    ///
    /// ```
    /// use majordomus_cli::mesh::config::CooperationConfig;
    /// assert!(CooperationConfig::default().validate().is_ok());
    /// let flapping = CooperationConfig { heartbeat_seconds: 10, expiry_seconds: 15, ..Default::default() };
    /// assert!(flapping.validate().is_err());
    /// // a progress ttl a working session could lapse inside, and one that never lapses
    /// let racing = CooperationConfig { session_ttl_seconds: 10, ..Default::default() };
    /// assert!(racing.validate().is_err());
    /// let off = CooperationConfig { session_ttl_seconds: 0, ..Default::default() };
    /// assert!(off.validate().is_ok());
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
            if !endpoint_shape(seed, false) {
                return Err(format!(
                    "cooperation seed '{seed}' is not https://host[:port] or http://host:port"
                ));
            }
        }
        if let Some(repository) = &self.repository {
            if repository.trim().is_empty() || repository.len() > 128 {
                return Err("cooperation.repository is empty or over 128 characters".into());
            }
        }
        let ttl = self.session_ttl_seconds;
        if ttl != 0 && ttl < 4 * self.heartbeat_seconds {
            return Err(format!(
                "cooperation.session_ttl_seconds {ttl} is under four heartbeats ({}): a working session would lapse between two beats",
                4 * self.heartbeat_seconds
            ));
        }
        if ttl > u64::from(super::journal::MAX_SESSION_TTL_SECONDS) {
            return Err(format!(
                "cooperation.session_ttl_seconds {ttl} is over {} (an hour)",
                super::journal::MAX_SESSION_TTL_SECONDS
            ));
        }
        Ok(())
    }
}

impl MeshConfig {
    /// Parse one indexed object into a mesh declaration, or say why it is not one:
    /// the wrong schema version, an unknown key, and a zero interval are each refused
    /// with the object's URI in the reason.
    pub fn parse(object: &Object) -> Result<MeshConfig, MeshError> {
        Self::from_metadata(&object.metadata, &object.uri)
    }

    /// [`MeshConfig::parse`] for a declaration that is not an indexed object: the metadata
    /// mapping as the YAML subset parsed it, and the name to quote in a refusal. The trunk's
    /// copy of the declaration is read this way (see [`super::root`]).
    ///
    /// ```
    /// use majordomus_cli::mesh::MeshConfig;
    /// let ok = serde_json::json!({ "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x" });
    /// assert!(MeshConfig::from_metadata(&ok, "master:x.yaml").is_ok());
    /// let wrong = serde_json::json!({ "schema": "mesh/v0", "kind": "mesh-declaration", "id": "x" });
    /// assert!(MeshConfig::from_metadata(&wrong, "master:x.yaml").unwrap_err().to_string().contains("master:x.yaml"));
    /// ```
    pub fn from_metadata(metadata: &serde_json::Value, uri: &str) -> Result<MeshConfig, MeshError> {
        let parsed: MeshConfig = serde_json::from_value(metadata.clone())
            .map_err(|e| MeshError::Config(format!("{}: {e}", uri)))?;
        if parsed.schema != SCHEMA_VERSION {
            return Err(MeshError::Config(format!(
                "{}: schema {} is not {SCHEMA_VERSION}",
                uri, parsed.schema
            )));
        }
        if parsed.kind != KIND {
            return Err(MeshError::Config(format!(
                "{}: kind {} is not {KIND}",
                uri, parsed.kind
            )));
        }
        if parsed.multicast.interval_seconds == 0
            || parsed.broadcast.interval_seconds == 0
            || parsed.rendezvous.interval_seconds == 0
        {
            return Err(MeshError::Config(format!(
                "{}: an interval of 0 would be a busy loop",
                uri
            )));
        }
        parsed
            .cooperation
            .validate()
            .map_err(|e| MeshError::Config(format!("{}: {e}", uri)))?;
        if let Some(endpoint) = parsed
            .rendezvous
            .endpoints
            .iter()
            .find(|e| !endpoint_shape(e, true))
        {
            return Err(MeshError::Config(format!(
                "{uri}: rendezvous endpoint '{endpoint}' is not https://host[:port] or http://host:port"
            )));
        }
        Ok(parsed)
    }
}

/// Whether `value` is an endpoint the mesh client can dial: `https://host[:port]`, where the
/// host may be a name and the port defaults to 443, or `http://host:port`, and — where
/// `bare` allows it, as a rendezvous endpoint always has — `host:port` read as http. No path,
/// no other scheme. Whether an http endpoint's address may carry cleartext is the client's
/// question, asked at every dial ([`crate::mcp::bridge::transport`]): an address is not a
/// shape, and a name resolves differently on different days.
///
/// ```
/// use majordomus_cli::mesh::config::endpoint_shape;
/// assert!(endpoint_shape("https://hub.example", false));
/// assert!(endpoint_shape("https://hub.example:8443", false));
/// assert!(endpoint_shape("http://10.0.0.2:8742", false));
/// assert!(!endpoint_shape("http://10.0.0.2", false), "an http endpoint names its port");
/// assert!(!endpoint_shape("10.0.0.2:8742", false));
/// assert!(endpoint_shape("10.0.0.2:8742", true));
/// assert!(!endpoint_shape("ws://hub.example:80", true));
/// assert!(!endpoint_shape("https://hub.example/mesh", false));
/// ```
pub fn endpoint_shape(value: &str, bare: bool) -> bool {
    let (tls, authority) = if let Some(rest) = value.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = value.strip_prefix("http://") {
        (false, rest)
    } else if bare && !value.contains("://") {
        (false, value)
    } else {
        return false;
    };
    !authority.contains('/')
        && crate::mcp::bridge::split_authority(authority)
            .is_ok_and(|(_, port)| tls || port.is_some())
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
    fn a_declaration_of_another_kind_or_a_cooperation_it_cannot_keep_is_refused() {
        let refusal = |metadata: serde_json::Value| {
            MeshConfig::from_metadata(&metadata, "master:m.yaml")
                .unwrap_err()
                .to_string()
        };
        let kind = refusal(serde_json::json!({
            "schema": "mesh/v1", "kind": "rule", "id": "x"
        }));
        assert!(kind.contains("master:m.yaml: kind rule is not"), "{kind}");
        let cooperation = refusal(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x",
            "cooperation": { "heartbeat_seconds": 5, "expiry_seconds": 6 }
        }));
        assert!(
            cooperation
                .contains("master:m.yaml: cooperation.expiry_seconds 6 is under three heartbeats"),
            "{cooperation}"
        );
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

    #[test]
    fn https_seeds_and_endpoints_are_accepted_by_host_name_and_other_schemes_are_refused() {
        let declaration = |seeds: serde_json::Value, endpoints: serde_json::Value| {
            MeshConfig::from_metadata(
                &serde_json::json!({
                    "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x",
                    "rendezvous": { "endpoints": endpoints },
                    "cooperation": { "seeds": seeds }
                }),
                "master:m.yaml",
            )
        };
        let accepted = declaration(
            serde_json::json!([
                "https://hub.example",
                "https://hub.example:8443",
                "http://10.0.0.2:8742"
            ]),
            serde_json::json!([
                "https://hub.example",
                "http://192.168.100.30:8791",
                "127.0.0.1:8791",
                "http://[::1]:8791"
            ]),
        )
        .unwrap();
        assert_eq!(accepted.cooperation.seeds.len(), 3);
        assert_eq!(accepted.rendezvous.endpoints.len(), 4);
        // a public http hub is a shape the declaration accepts and the client refuses to dial
        assert!(declaration(
            serde_json::json!([]),
            serde_json::json!(["http://203.0.113.5:8791"])
        )
        .is_ok());

        let refused = |seeds: serde_json::Value, endpoints: serde_json::Value| {
            declaration(seeds, endpoints).unwrap_err().to_string()
        };
        for seed in [
            "ws://hub.example:80",
            "10.0.0.2:8742",
            "http://10.0.0.2",
            "https://hub.example/mesh",
            "https://:443",
        ] {
            let text = refused(serde_json::json!([seed]), serde_json::json!([]));
            assert!(
                text.contains(&format!(
                    "cooperation seed '{seed}' is not https://host[:port] or http://host:port"
                )),
                "{text}"
            );
        }
        for endpoint in [
            "ftp://hub.example:21",
            "http://hub.example",
            "https://hub.example:0",
            "http://10.0.0.2:8791/x",
        ] {
            let text = refused(serde_json::json!([]), serde_json::json!([endpoint]));
            assert!(
                text.contains(&format!(
                    "master:m.yaml: rendezvous endpoint '{endpoint}' is not https://host[:port]"
                )),
                "{text}"
            );
        }
    }
}
