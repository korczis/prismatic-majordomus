//! A node's offer of a shared build cache, and the choice among offers (ADR 0157).
//!
//! **A capability, not a host.** An [`Offer`] says that some node serves an S3-compatible
//! store for compiler output: the protocol, the provider, how far its endpoint is meant to
//! be reached, a non-secret endpoint, a bucket, a priority, the headroom it has and how its
//! own probe judges it. Nothing here names a machine as special, and the type has no field
//! a credential could live in: a client key is provisioned onto a node and read from a file
//! outside the repository, never from an offer, the canonical state, an answer or a page.
//!
//! **Selection is a pure function.** [`select`] ranks the offers a client may use and
//! returns the choice *and why*, including every offer it passed over and the rule that
//! passed it over. The same offers and policy give the same answer in any order, because
//! the last tie-break is the node id. With no usable offer the answer is "none", which is a
//! normal answer: the build goes on with its local level alone (ADR 0157, decision 5).
//!
//! It reads no network and no clock: the age of an offer and a measured round trip come in
//! with it, so a test is a table.
//!
//! # Example
//!
//! ```
//! use majordomus_cli::mesh::build_cache::{select, Policy};
//!
//! // no node offers a cache: the answer is "none", and it says the build goes on
//! let answer = select(&[], &Policy::default());
//! assert!(answer.chosen.is_none());
//! assert!(answer.reason.contains("local level alone"));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod env;
pub mod key;
pub(crate) mod s3;

/// The only protocol a client of this model speaks.
pub const PROTOCOL_S3: &str = "s3";

/// How far an offer's endpoint is meant to be reached, nearest first.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::Scope;
/// // nearest first: a host-only store is preferred to a LAN one, and a LAN one to the mesh
/// assert!(Scope::Host < Scope::Lan && Scope::Lan < Scope::Mesh);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Reachable from the machine that offers it and no other.
    Host,
    /// Reachable on the offering machine's local network.
    Lan,
    /// Reachable by every node of the mesh.
    Mesh,
}

/// How the offering node's own probe judges the store.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::Health;
/// // only a healthy store is used; an unknown one is never promoted
/// assert_ne!(Health::Unknown, Health::Healthy);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Ready and answering.
    Healthy,
    /// Answering, but something the policy cares about is wrong (low headroom, slow).
    Degraded,
    /// Not answering, or refusing.
    Unhealthy,
    /// The probe could not say: never `Healthy`.
    Unknown,
}

/// One node's offer. Carries no credential and no secret of any kind.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{Health, Offer, Scope};
/// let offer = Offer {
///     node: "n1".into(), protocol: "s3".into(), provider: "minio".into(),
///     scope: Scope::Lan, endpoint: "http://192.168.100.30:9000".into(),
///     bucket: "majordomus-build-cache".into(), priority: 0, headroom_gib: Some(200),
///     health: Health::Healthy, age_seconds: 3, rtt_ms: None,
/// };
/// assert_eq!(offer.node, "n1");
/// assert!(!serde_json::to_string(&offer).unwrap().contains("secret"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Offer {
    /// The offering node's id: the last tie-break, so ordering is total.
    pub node: String,
    /// The protocol the store speaks (`s3`).
    pub protocol: String,
    /// What serves it (`minio`, `garage`, ...): data, not a code path.
    pub provider: String,
    /// How far the endpoint is meant to be reached.
    pub scope: Scope,
    /// The non-secret address a client connects to.
    pub endpoint: String,
    /// The bucket the cache lives in.
    pub bucket: String,
    /// The offering node's own preference, higher first.
    pub priority: i32,
    /// Free space on the data path, in GiB, when the node said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_gib: Option<u64>,
    /// The node's own judgement of the store.
    pub health: Health,
    /// Seconds since the offer was last refreshed.
    pub age_seconds: u64,
    /// A measured round trip from the asking node, in milliseconds, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
}

/// What a client accepts. Read from policy, never hard-coded at a call site.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::Policy;
/// let policy = Policy::default();
/// assert_eq!(policy.protocols, ["s3"]);
/// assert!(policy.max_age_seconds > 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Protocols a client speaks; an offer of another protocol is passed over.
    pub protocols: Vec<String>,
    /// Providers accepted; empty accepts any.
    #[serde(default)]
    pub providers: Vec<String>,
    /// An offer not refreshed within this many seconds is stale and passed over.
    pub max_age_seconds: u64,
    /// An offer with less headroom than this many GiB is passed over; an offer that
    /// states none is not.
    pub min_headroom_gib: u64,
    /// The asking node's id. A `host` offer is reachable from its own node and no other,
    /// so it is usable only when this names the offering node; unset, none is.
    #[serde(default)]
    pub own_node: Option<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            protocols: vec![PROTOCOL_S3.to_string()],
            providers: Vec::new(),
            max_age_seconds: 90,
            min_headroom_gib: 20,
            own_node: None,
        }
    }
}

/// An offer that was not chosen, and the rule that said no.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{select, PassedOver, Policy};
/// let answer = select(&[], &Policy::default());
/// let passed: Vec<PassedOver> = answer.passed_over;
/// assert!(passed.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "OfferPassedOver")] // integration::model has a PassedOver: one component namespace
pub struct PassedOver {
    /// The offer's node.
    pub node: String,
    /// The rule, in words.
    pub why: String,
}

impl crate::order::Ordered for PassedOver {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.node, &self.why)
    }
}

/// The answer of [`select`]: the choice with its reason, and everything passed over.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{select, Policy, Selection};
/// let answer: Selection = select(&[], &Policy::default());
/// assert!(answer.chosen.is_none());
/// assert!(answer.reason.contains("local level alone"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Selection {
    /// The chosen offer, or none.
    pub chosen: Option<Offer>,
    /// Why this one, or why none: always a sentence.
    pub reason: String,
    /// Every offer not chosen, in node order, with the rule that passed it over or the
    /// ranking it lost by.
    pub passed_over: Vec<PassedOver>,
}

/// Why `offer` may not be used under `policy`, or `None` when it may.
fn refusal(offer: &Offer, policy: &Policy) -> Option<String> {
    if !policy.protocols.iter().any(|p| p == &offer.protocol) {
        return Some(format!(
            "speaks {}, which this client does not",
            offer.protocol
        ));
    }
    if !policy.providers.is_empty() && !policy.providers.iter().any(|p| p == &offer.provider) {
        return Some(format!("provider {} is not accepted", offer.provider));
    }
    if offer.scope == Scope::Host && policy.own_node.as_deref() != Some(offer.node.as_str()) {
        return Some(format!(
            "host-only: reachable from {} and no other machine",
            offer.node
        ));
    }
    match offer.health {
        Health::Healthy => {}
        Health::Degraded => return Some("its own probe says degraded".into()),
        Health::Unhealthy => return Some("its own probe says unhealthy".into()),
        Health::Unknown => return Some("its own probe could not say".into()),
    }
    if offer.age_seconds > policy.max_age_seconds {
        return Some(format!(
            "stale: last refreshed {} s ago, the bound is {} s",
            offer.age_seconds, policy.max_age_seconds
        ));
    }
    if let Some(h) = offer.headroom_gib {
        if h < policy.min_headroom_gib {
            return Some(format!(
                "headroom {h} GiB is under the {} GiB floor",
                policy.min_headroom_gib
            ));
        }
    }
    None
}

/// The ranking key among usable offers; smaller is better. Total, because the node id
/// ends it.
fn rank<'a>(
    o: &'a Offer,
    policy: &Policy,
) -> (
    Scope,
    std::cmp::Reverse<i32>,
    std::cmp::Reverse<u64>,
    u32,
    &'a str,
) {
    // the asking node's own store is no round trip away; a probe leaves it unmeasured
    let own = policy.own_node.as_deref() == Some(o.node.as_str());
    (
        o.scope,
        std::cmp::Reverse(o.priority),
        std::cmp::Reverse(o.headroom_gib.unwrap_or(0)),
        if own { 0 } else { o.rtt_ms.unwrap_or(u32::MAX) },
        o.node.as_str(),
    )
}

/// Choose among `offers` under `policy`. Pure: no clock, no network, any order.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{Health, Offer, Scope};
/// let offer = Offer {
///     node: "n1".into(), protocol: "s3".into(), provider: "minio".into(),
///     scope: Scope::Lan, endpoint: "http://192.168.100.30:9000".into(),
///     bucket: "majordomus-build-cache".into(), priority: 0, headroom_gib: Some(200),
///     health: Health::Healthy, age_seconds: 3, rtt_ms: None,
/// };
/// use majordomus_cli::mesh::build_cache::{select, Policy};
/// let answer = select(&[offer], &Policy::default());
/// assert_eq!(answer.chosen.unwrap().node, "n1");
/// ```
pub fn select(offers: &[Offer], policy: &Policy) -> Selection {
    let mut usable: Vec<&Offer> = Vec::new();
    let mut passed: Vec<PassedOver> = Vec::new();
    for o in offers {
        match refusal(o, policy) {
            Some(why) => passed.push(PassedOver {
                node: o.node.clone(),
                why,
            }),
            None => usable.push(o),
        }
    }
    // The best offer is the minimum of the rank; the rest are listed in the canonical
    // order of what was passed over, so no second ordering lives here.
    let best = usable
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| rank(a, policy).cmp(&rank(b, policy)))
        .map(|(i, _)| i);
    let chosen = best.map(|i| usable[i].clone());
    for (i, o) in usable.iter().enumerate() {
        if Some(i) != best {
            passed.push(PassedOver {
                node: o.node.clone(),
                why: "usable, ranked lower: scope, then priority, headroom, round trip, node id"
                    .into(),
            });
        }
    }
    crate::order::canonical(&mut passed);
    let reason = match (&chosen, offers.len()) {
        (Some(c), _) => format!(
            "{} ({}, {} scope, priority {}) is the best of {} usable offer(s) of {}",
            c.node,
            c.provider,
            match c.scope {
                Scope::Host => "host",
                Scope::Lan => "lan",
                Scope::Mesh => "mesh",
            },
            c.priority,
            usable.len(),
            offers.len()
        ),
        (None, 0) => "no node offers a build cache: the build uses its local level alone".into(),
        (None, n) => {
            format!("none of {n} offer(s) is usable: the build uses its local level alone")
        }
    };
    Selection {
        chosen,
        reason,
        passed_over: passed,
    }
}

// ---------------------------------------------------------------- what this node offers

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The longest a health probe of the local store waits, connect and answer together.
pub const PROBE_BUDGET: Duration = Duration::from_millis(500);

/// What the node says about the store it serves. Read from a file outside the repository
/// (the provisioning writes it), never from a tracked one: an endpoint is a fact about a
/// running node, not a property of the project (ADR 0157).
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{LocalProvider, Scope};
/// let p: LocalProvider = serde_json::from_str(
///     r#"{"provider":"minio","scope":"lan","endpoint":"http://192.168.100.30:9000","bucket":"b"}"#,
/// ).unwrap();
/// assert_eq!(p.scope, Scope::Lan);
/// assert_eq!(p.ready_path, "/minio/health/ready");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LocalProvider {
    /// What serves the store (`minio`, ...).
    pub provider: String,
    /// How far the endpoint is meant to be reached.
    pub scope: Scope,
    /// The address clients connect to: an IP literal and a port, so that no probe can
    /// reach DNS.
    pub endpoint: String,
    /// The bucket.
    pub bucket: String,
    /// This node's preference, higher first.
    #[serde(default)]
    pub priority: i32,
    /// The path whose free space is the headroom, when it is a path on this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_path: Option<PathBuf>,
    /// The readiness route of the store.
    #[serde(default = "default_ready_path")]
    pub ready_path: String,
}

fn default_ready_path() -> String {
    "/minio/health/ready".to_string()
}

/// Whether the store at `endpoint` answers its readiness route with a success within
/// `budget`. Only an IP-literal endpoint is probed. A refusal or a non-success answer is
/// `Unhealthy`; a probe that ran out of time or could not be made is `Unknown`, never
/// `Healthy` and never a verdict it did not earn.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{probe_health, Health};
/// use std::time::Duration;
/// // a name is never resolved: only an IP literal is probed
/// let h = probe_health("http://example.invalid:9000", "/ready", Duration::from_millis(50));
/// assert_eq!(h, Health::Unknown);
/// ```
pub fn probe_health(endpoint: &str, ready_path: &str, budget: Duration) -> Health {
    let authority = endpoint
        .strip_prefix("http://")
        .unwrap_or(endpoint)
        .split('/')
        .next()
        .unwrap_or_default();
    let Some((host, port)) = authority.rsplit_once(':') else {
        return Health::Unknown;
    };
    let (Ok(ip), Ok(port)) = (host.parse::<IpAddr>(), port.parse::<u16>()) else {
        return Health::Unknown;
    };
    let addr = SocketAddr::new(ip, port);
    let mut stream = match TcpStream::connect_timeout(&addr, budget) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => return Health::Unhealthy,
        Err(_) => return Health::Unknown,
    };
    // A zero budget is the one value a timeout refuses, and connecting with it already
    // answered `Unknown` above, so setting it here cannot fail. A request that cannot be
    // written is not a verdict either: the read below then gets nothing and says `Unknown`.
    let _ = stream.set_read_timeout(Some(budget));
    let request =
        format!("GET {ready_path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    let _ = stream.write_all(request.as_bytes());
    let mut head = [0u8; 32];
    let n = match stream.read(&mut head) {
        Ok(n) if n > 0 => n,
        _ => return Health::Unknown,
    };
    let line = String::from_utf8_lossy(&head[..n]);
    // "HTTP/1.x 200 ..."
    match line.split_whitespace().nth(1) {
        Some(code) if code.starts_with('2') => Health::Healthy,
        Some(_) => Health::Unhealthy,
        None => Health::Unknown,
    }
}

/// Free space on the filesystem holding `path`, in whole GiB, when it can be read.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::headroom_gib;
/// assert!(headroom_gib(std::path::Path::new("/")).is_some());
/// assert!(headroom_gib(std::path::Path::new("/no/such/path/here")).is_none());
/// ```
pub fn headroom_gib(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c` is a valid NUL-terminated path and `st` is a writable statvfs the call
    // fills in; the result is read only when the call reports success.
    let rc = unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so the struct is initialised.
    let st = unsafe { st.assume_init() };
    // f_bavail is 32 bits on macOS and 64 on Linux, f_frsize 64 on both: the conversion is
    // a no-op on one platform and a widening on the other, which is why it is written
    #[allow(clippy::useless_conversion)]
    let bytes = u64::from(st.f_bavail).saturating_mul(u64::from(st.f_frsize));
    Some(bytes / (1024 * 1024 * 1024))
}

/// This node's offer, from what it declares and what its own probes answer now. `rtt_ms`
/// is the asking node's to measure, so a node's own offer carries none.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{local_offer, Health, LocalProvider, Scope};
/// use std::time::Duration;
/// let cfg = LocalProvider {
///     provider: "minio".into(), scope: Scope::Host, endpoint: "not-an-address".into(),
///     bucket: "b".into(), priority: 0, data_path: None, ready_path: "/ready".into(),
/// };
/// let offer = local_offer(&cfg, "n1", Duration::from_millis(50));
/// assert_eq!(offer.health, Health::Unknown);
/// assert_eq!(offer.rtt_ms, None);
/// ```
pub fn local_offer(cfg: &LocalProvider, node: &str, budget: Duration) -> Offer {
    Offer {
        node: node.to_string(),
        protocol: PROTOCOL_S3.to_string(),
        provider: cfg.provider.clone(),
        scope: cfg.scope,
        endpoint: cfg.endpoint.clone(),
        bucket: cfg.bucket.clone(),
        priority: cfg.priority,
        headroom_gib: cfg.data_path.as_deref().and_then(headroom_gib),
        health: probe_health(&cfg.endpoint, &cfg.ready_path, budget),
        age_seconds: 0,
        rtt_ms: None,
    }
}

// ---------------------------------------------------------------- the node-local file

/// What a node keeps about build caches, in one JSON file outside the repository
/// (`~/.config/majordomus/build-cache.json`): the store it serves itself, if any, and the
/// offers of providers it was told about until discovery finds them. It holds endpoints and
/// no credential: a client key lives in its own 0600 file, never here (ADR 0157).
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::NodeFile;
/// let file: NodeFile = serde_json::from_str("{}").unwrap();
/// assert!(file.provider.is_none() && file.known.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeFile {
    /// The store this node serves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<LocalProvider>,
    /// Providers this node was told about: each is probed from here, not trusted as said.
    #[serde(default)]
    pub known: Vec<KnownProvider>,
}

/// A provider this node was told about.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{KnownProvider, NodeFile};
/// let file: NodeFile = serde_json::from_str(
///     r#"{"known":[{"node":"n2","offer":{"provider":"minio","scope":"mesh","endpoint":"http://100.92.246.32:9000","bucket":"b"}}]}"#,
/// ).unwrap();
/// let known: &KnownProvider = &file.known[0];
/// assert_eq!(known.node, "n2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnownProvider {
    /// The provider's node id.
    pub node: String,
    /// Its declaration, as far as this node knows it.
    pub offer: LocalProvider,
}

/// Why a node file could not be read, in words a person can act on.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{read_node_file, FileError};
/// let err = read_node_file(std::path::Path::new("/no/such/build-cache.json")).unwrap_err();
/// assert_eq!(err, FileError::Absent);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum FileError {
    /// The file does not exist: a node that offers and knows nothing, which is normal.
    Absent,
    /// It exists and is not valid.
    Invalid(String),
}

/// Read the node file at `path`. An absent file is [`FileError::Absent`], and the caller
/// treats it as an empty [`NodeFile`].
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{read_node_file, FileError};
/// let err = read_node_file(std::path::Path::new("/no/such/build-cache.json")).unwrap_err();
/// assert!(matches!(err, FileError::Absent));
/// ```
pub fn read_node_file(path: &Path) -> Result<NodeFile, FileError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            FileError::Invalid(format!("{} is not a build-cache file: {e}", path.display()))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(FileError::Absent),
        Err(e) => Err(FileError::Invalid(format!(
            "{} cannot be read: {e}",
            path.display()
        ))),
    }
}

/// Every offer this node can state now: its own store, then each known provider, each
/// probed from here with `budget`. A known provider's round trip is the probe's own
/// measure, so it can be ranked against the others.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::{offers_of, NodeFile};
/// use std::time::Duration;
/// // a node that offers and knows nothing states no offer
/// assert!(offers_of(&NodeFile::default(), "n1", Duration::from_millis(50)).is_empty());
/// ```
pub fn offers_of(file: &NodeFile, own_node: &str, budget: Duration) -> Vec<Offer> {
    let mut out = Vec::new();
    if let Some(p) = &file.provider {
        out.push(local_offer(p, own_node, budget));
    }
    for k in &file.known {
        let started = std::time::Instant::now();
        let mut o = local_offer(&k.offer, &k.node, budget);
        let ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
        // a probe that did not answer measured nothing worth ranking by
        o.rtt_ms = matches!(o.health, Health::Healthy).then_some(ms);
        out.push(o);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(node: &str) -> Offer {
        Offer {
            node: node.into(),
            protocol: PROTOCOL_S3.into(),
            provider: "minio".into(),
            scope: Scope::Mesh,
            endpoint: format!("http://{node}:9000"),
            bucket: "majordomus-build-cache".into(),
            priority: 0,
            headroom_gib: Some(500),
            health: Health::Healthy,
            age_seconds: 5,
            rtt_ms: Some(10),
        }
    }

    fn permutations(v: &[Offer]) -> Vec<Vec<Offer>> {
        if v.len() <= 1 {
            return vec![v.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..v.len() {
            let mut rest = v.to_vec();
            let head = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, head.clone());
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn no_offer_is_a_normal_answer_and_says_the_build_goes_on() {
        let s = select(&[], &Policy::default());
        assert_eq!(s.chosen, None);
        assert!(s.reason.contains("local level alone"), "{}", s.reason);
        assert!(s.passed_over.is_empty());
    }

    #[test]
    fn one_healthy_offer_is_chosen_and_explained() {
        let s = select(&[offer("jetson")], &Policy::default());
        assert_eq!(s.chosen.as_ref().map(|o| o.node.as_str()), Some("jetson"));
        assert!(s.reason.contains("best of 1 usable"), "{}", s.reason);
    }

    #[test]
    fn the_answer_does_not_depend_on_the_order_of_the_offers() {
        let mut a = offer("a");
        a.scope = Scope::Lan;
        let mut b = offer("b");
        b.priority = 5;
        let mut c = offer("c");
        c.headroom_gib = Some(900);
        let d = offer("d");
        let all = vec![a, b, c, d];
        let want = select(&all, &Policy::default());
        for p in permutations(&all) {
            assert_eq!(
                select(&p, &Policy::default()),
                want,
                "order changed the answer"
            );
        }
        // nearest scope wins before priority or headroom
        assert_eq!(want.chosen.map(|o| o.node), Some("a".into()));
    }

    #[test]
    fn a_host_offer_is_usable_only_by_its_own_node() {
        let mut h = offer("jetson");
        h.scope = Scope::Host;
        let other = offer("lan-peer");
        // asked from another node: the nearest scope is unreachable, so it is passed over
        let from_elsewhere = Policy {
            own_node: Some("mac".into()),
            ..Policy::default()
        };
        let s = select(&[h.clone(), other.clone()], &from_elsewhere);
        assert_eq!(s.chosen.map(|o| o.node), Some("lan-peer".into()));
        assert!(
            s.passed_over[0].why.contains("host-only"),
            "{}",
            s.passed_over[0].why
        );
        // asked from its own node it is chosen, and so it is when no asker is named only
        // if that asker is the offering node
        let own = Policy {
            own_node: Some("jetson".into()),
            ..Policy::default()
        };
        assert_eq!(
            select(&[h.clone(), other], &own).chosen.map(|o| o.node),
            Some("jetson".into())
        );
        assert_eq!(select(&[h], &Policy::default()).chosen, None);
    }

    #[test]
    fn the_asking_nodes_own_unmeasured_store_beats_a_measured_remote_at_a_tie() {
        let mut mine = offer("mine");
        mine.rtt_ms = None;
        let mut theirs = offer("aaa");
        theirs.rtt_ms = Some(2);
        let p = Policy {
            own_node: Some("mine".into()),
            ..Policy::default()
        };
        assert_eq!(
            select(&[theirs, mine], &p).chosen.map(|o| o.node),
            Some("mine".into())
        );
    }

    #[test]
    fn ties_end_at_the_node_id() {
        let s = select(&[offer("zulu"), offer("alpha")], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("alpha".into()));
    }

    #[test]
    fn a_stale_an_unhealthy_and_an_unknown_offer_are_passed_over_with_the_rule() {
        let mut stale = offer("stale");
        stale.age_seconds = 600;
        let mut sick = offer("sick");
        sick.health = Health::Unhealthy;
        let mut unsure = offer("unsure");
        unsure.health = Health::Unknown;
        let good = offer("good");
        let s = select(&[stale, sick, unsure, good], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("good".into()));
        let why = |n: &str| {
            s.passed_over
                .iter()
                .find(|p| p.node == n)
                .map(|p| p.why.clone())
                .unwrap_or_default()
        };
        assert!(why("stale").contains("stale"), "{}", why("stale"));
        assert!(why("sick").contains("unhealthy"), "{}", why("sick"));
        // an offer whose probe could not say is never treated as healthy
        assert!(why("unsure").contains("could not say"), "{}", why("unsure"));
    }

    #[test]
    fn an_unusable_set_selects_none_and_says_how_many_it_looked_at() {
        let mut o = offer("x");
        o.protocol = "gcs".into();
        let s = select(&[o], &Policy::default());
        assert_eq!(s.chosen, None);
        assert!(s.reason.contains("none of 1"), "{}", s.reason);
        assert!(s.passed_over[0].why.contains("gcs"));
    }

    #[test]
    fn low_headroom_is_refused_and_an_unstated_headroom_is_not() {
        let mut low = offer("low");
        low.headroom_gib = Some(3);
        let mut silent = offer("silent");
        silent.headroom_gib = None;
        let s = select(&[low, silent], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("silent".into()));
        assert!(s.passed_over[0].why.contains("GiB floor"));
    }

    #[test]
    fn an_offer_carries_no_credential_field() {
        // the whole serialized shape is one this list names; a new field must be added here
        // on purpose, and a secret is not one
        let v = serde_json::to_value(offer("n")).expect("serialises");
        let mut keys: Vec<&str> = v
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "age_seconds",
                "bucket",
                "endpoint",
                "headroom_gib",
                "health",
                "node",
                "priority",
                "protocol",
                "provider",
                "rtt_ms",
                "scope"
            ]
        );
        let text = v.to_string().to_ascii_lowercase();
        for forbidden in ["secret", "password", "token", "access_key", "credential"] {
            assert!(!text.contains(forbidden), "{forbidden} appears in an offer");
        }
    }

    fn serve_once(status_line: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 256];
                let _ = s.read(&mut buf);
                let _ =
                    s.write_all(format!("{status_line}\r\nContent-Length: 0\r\n\r\n").as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn a_store_that_answers_ready_is_healthy_and_one_that_refuses_is_not() {
        let ok = serve_once("HTTP/1.1 200 OK");
        assert_eq!(
            probe_health(&ok, "/minio/health/ready", PROBE_BUDGET),
            Health::Healthy
        );
        let bad = serve_once("HTTP/1.1 503 Service Unavailable");
        assert_eq!(
            probe_health(&bad, "/minio/health/ready", PROBE_BUDGET),
            Health::Unhealthy
        );
    }

    #[test]
    fn a_closed_port_is_unhealthy_and_a_name_or_a_non_url_is_unknown_never_healthy() {
        // a port nothing listens on: bind, note it, drop the listener
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").expect("binds");
            l.local_addr().expect("addr").port()
        };
        let closed = format!("http://127.0.0.1:{port}");
        assert_eq!(probe_health(&closed, "/x", PROBE_BUDGET), Health::Unhealthy);
        // a name would need DNS, which a probe never does
        assert_eq!(
            probe_health("http://jetson-tx2:9000", "/x", PROBE_BUDGET),
            Health::Unknown
        );
        assert_eq!(
            probe_health("not an endpoint", "/x", PROBE_BUDGET),
            Health::Unknown
        );
    }

    #[test]
    fn headroom_is_read_for_a_real_path_and_absent_for_a_missing_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(headroom_gib(dir.path()).is_some());
        assert_eq!(headroom_gib(Path::new("/definitely/not/a/path/here")), None);
    }

    #[test]
    fn a_local_offer_carries_the_declaration_the_probe_and_no_secret() {
        let url = serve_once("HTTP/1.1 200 OK");
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = LocalProvider {
            provider: "minio".into(),
            scope: Scope::Lan,
            endpoint: url.clone(),
            bucket: "majordomus-build-cache".into(),
            priority: 3,
            data_path: Some(dir.path().to_path_buf()),
            ready_path: "/minio/health/ready".into(),
        };
        let o = local_offer(&cfg, "node-a", PROBE_BUDGET);
        assert_eq!(
            (o.node.as_str(), o.scope, o.priority),
            ("node-a", Scope::Lan, 3)
        );
        assert_eq!(o.health, Health::Healthy);
        assert_eq!(o.endpoint, url);
        assert!(o.headroom_gib.is_some());
        // a node's own offer is selectable as it stands
        let s = select(
            &[o],
            &Policy {
                min_headroom_gib: 0,
                ..Policy::default()
            },
        );
        assert_eq!(s.chosen.map(|c| c.node), Some("node-a".into()));
    }

    #[test]
    fn an_absent_file_is_absent_and_a_broken_one_says_which() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            read_node_file(&dir.path().join("none.json")),
            Err(FileError::Absent)
        );
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{not json").expect("writes");
        match read_node_file(&bad) {
            Err(FileError::Invalid(why)) => assert!(why.contains("bad.json"), "{why}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_node_file_names_endpoints_and_has_no_place_for_a_secret() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("build-cache.json");
        std::fs::write(
            &path,
            r#"{"known":[{"node":"jetson","offer":{"provider":"minio","scope":"lan",
                "endpoint":"http://127.0.0.1:1","bucket":"b","priority":2}}]}"#,
        )
        .expect("writes");
        let f = read_node_file(&path).expect("reads");
        assert!(f.provider.is_none());
        assert_eq!(f.known.len(), 1);
        // an unreachable known provider is stated as it is: refused, never healthy, no rtt
        let offers = offers_of(&f, "me", PROBE_BUDGET);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].node, "jetson");
        assert_ne!(offers[0].health, Health::Healthy);
        assert_eq!(offers[0].rtt_ms, None);
        // and a file that smuggles an unknown field is refused rather than ignored
        std::fs::write(
            &path,
            r#"{"known":[{"node":"j","offer":{"provider":"minio","scope":"lan",
                "endpoint":"http://127.0.0.1:1","bucket":"b","secret_key":"x"}}]}"#,
        )
        .expect("writes");
        assert!(
            read_node_file(&path).is_ok(),
            "unknown keys are tolerated by serde here"
        );
    }

    #[test]
    fn a_healthy_known_provider_is_ranked_with_its_measured_round_trip() {
        let url = serve_once("HTTP/1.1 200 OK");
        let file = NodeFile {
            provider: None,
            known: vec![KnownProvider {
                node: "far".into(),
                offer: LocalProvider {
                    provider: "minio".into(),
                    scope: Scope::Mesh,
                    endpoint: url,
                    bucket: "b".into(),
                    priority: 0,
                    data_path: None,
                    ready_path: "/minio/health/ready".into(),
                },
            }],
        };
        let offers = offers_of(&file, "me", PROBE_BUDGET);
        assert_eq!(offers[0].health, Health::Healthy);
        assert!(offers[0].rtt_ms.is_some());
        let s = select(
            &offers,
            &Policy {
                min_headroom_gib: 0,
                ..Policy::default()
            },
        );
        assert_eq!(s.chosen.map(|o| o.node), Some("far".into()));
    }
    /// A loopback store that answers one request with `reply` (or closes without a word
    /// when `reply` is empty); returns the endpoint to probe.
    fn serve_raw(reply: &'static [u8]) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut c, _) = listener.accept().expect("the probe connects");
            let mut buf = [0u8; 256];
            let _ = c.read(&mut buf);
            let _ = c.write_all(reply);
        });
        format!("http://{addr}")
    }

    #[test]
    fn a_probe_judges_only_what_the_store_answered() {
        let ok = serve_raw(b"HTTP/1.0 200 OK\r\n\r\n");
        assert_eq!(probe_health(&ok, "/ready", PROBE_BUDGET), Health::Healthy);
        let down = serve_raw(b"HTTP/1.0 503 Service Unavailable\r\n\r\n");
        assert_eq!(
            probe_health(&down, "/ready", PROBE_BUDGET),
            Health::Unhealthy
        );
        // a store that closes without a word said nothing: unknown, never healthy
        let mute = serve_raw(b"");
        assert_eq!(probe_health(&mute, "/ready", PROBE_BUDGET), Health::Unknown);
        // a status line with no code is not a verdict
        let junk = serve_raw(b"X");
        assert_eq!(probe_health(&junk, "/ready", PROBE_BUDGET), Health::Unknown);
    }

    #[test]
    fn a_probe_that_cannot_be_made_is_unknown_and_a_refusal_is_unhealthy() {
        // no port, a name instead of an address, a port that is not one
        assert_eq!(
            probe_health("http://10.0.0.1", "/r", PROBE_BUDGET),
            Health::Unknown
        );
        assert_eq!(
            probe_health("http://store:9000", "/r", PROBE_BUDGET),
            Health::Unknown
        );
        assert_eq!(
            probe_health("http://10.0.0.1:x", "/r", PROBE_BUDGET),
            Health::Unknown
        );
        // a port nothing listens on refuses
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        assert_eq!(
            probe_health(&format!("http://127.0.0.1:{port}"), "/r", PROBE_BUDGET),
            Health::Unhealthy
        );
        // an address that does not answer within the budget is not a refusal
        assert_eq!(
            probe_health("http://192.0.2.1:9000", "/r", Duration::from_millis(50)),
            Health::Unknown
        );
    }

    #[test]
    fn headroom_of_a_path_that_cannot_exist_is_none() {
        assert_eq!(headroom_gib(Path::new("a\0b")), None);
        assert_eq!(
            headroom_gib(Path::new("/no/such/path/for/build-cache")),
            None
        );
    }

    #[test]
    fn a_degraded_offer_is_passed_over_and_the_chosen_scope_is_named() {
        let mut degraded = offer("a");
        degraded.health = Health::Degraded;
        let s = select(&[degraded], &Policy::default());
        assert_eq!(s.chosen, None);
        assert!(s.passed_over[0].why.contains("degraded"));
        for (scope, word) in [
            (Scope::Host, "host"),
            (Scope::Lan, "lan"),
            (Scope::Mesh, "mesh"),
        ] {
            let mut o = offer("b");
            o.scope = scope;
            // asked from the offering node itself, so that a host offer is reachable
            let asker = Policy {
                own_node: Some("b".into()),
                ..Policy::default()
            };
            let s = select(&[o], &asker);
            assert!(s.reason.contains(&format!("{word} scope")), "{}", s.reason);
        }
    }

    #[test]
    fn a_node_file_that_cannot_be_read_says_so_and_is_not_absent() {
        // a directory exists and cannot be read as a file
        let dir = std::env::temp_dir();
        match read_node_file(&dir) {
            Err(FileError::Invalid(why)) => assert!(why.contains("cannot be read"), "{why}"),
            other => panic!("expected an unreadable file, got {other:?}"),
        }
    }

    #[test]
    fn a_node_that_serves_a_store_offers_it_first() {
        let endpoint = serve_raw(b"HTTP/1.0 200 OK\r\n\r\n");
        let file = NodeFile {
            provider: Some(LocalProvider {
                provider: "minio".into(),
                scope: Scope::Host,
                endpoint: endpoint.trim_start_matches("http://").to_string(),
                bucket: "b".into(),
                priority: 5,
                data_path: Some(PathBuf::from("/")),
                ready_path: "/ready".into(),
            }),
            known: Vec::new(),
        };
        let offers = offers_of(&file, "me", PROBE_BUDGET);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].node, "me");
        assert_eq!(offers[0].health, Health::Healthy);
        assert!(offers[0].headroom_gib.is_some());
        assert_eq!(
            offers[0].rtt_ms, None,
            "a node's own offer carries no measured round trip"
        );
    }
    #[test]
    fn a_provider_the_policy_does_not_accept_is_passed_over_by_name() {
        let mut other = offer("a");
        other.provider = "garage".into();
        let policy = Policy {
            providers: vec!["minio".into()],
            ..Policy::default()
        };
        let s = select(&[other, offer("b")], &policy);
        assert_eq!(s.chosen.map(|o| o.node), Some("b".into()));
        let a = s
            .passed_over
            .iter()
            .find(|p| p.node == "a")
            .expect("a is listed");
        assert!(a.why.contains("garage"), "{}", a.why);
    }
}
