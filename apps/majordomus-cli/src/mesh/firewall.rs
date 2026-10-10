//! The host firewall, as the mesh needs it: what inbound traffic the declaration implies
//! this machine must admit, which firewall this host runs, the commands that admit it,
//! what the firewall says now, and what the kernel logged it dropping.
//!
//! The declaration says who may link; the host firewall says what reaches the socket at
//! all, and the two drift silently. On 2026-10-08 lundra, a declared hub, ran ufw with
//! default deny and no rule for the hub port or the multicast group: every registration
//! and every advertisement from the LAN was dropped at the kernel (187 of them in a week
//! of `[UFW BLOCK]` lines) while `mesh doctor` reported every check holding, because every
//! check it ran was a fact of the process and none was a fact of the host. This module
//! makes the admission a derived requirement with a verdict — a `firewall` check of the
//! doctor — instead of a thing an operator remembers.
//!
//! Pure where it can be: [`plan`] is a function of the declaration and this machine's
//! addresses; [`detect`] of the platform and what is on the path; [`render`] of the two;
//! and every judgement of a firewall's own output is a function of that text. Only
//! [`observe`], [`kernel_blocks`] and [`apply`] touch the host, and each says when it
//! could not.
//!
//! What this is not: a firewall manager. It admits the ports the mesh declared, from the
//! private networks the declaration names, and touches no other rule; and it never opens
//! anything on a public address: no rule is planned without a private network to admit from
//! (the multicast group excepted), and the declaration case refuses a public address first.
//!
//! ```
//! use std::net::Ipv4Addr;
//! use majordomus_cli::mesh::firewall::{plan, Role};
//! use majordomus_cli::mesh::MeshConfig;
//!
//! let config: MeshConfig = serde_json::from_value(serde_json::json!({
//!     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
//!     "rendezvous": { "endpoints": ["http://192.168.7.10:8791", "http://100.64.1.2:8791"] },
//! })).unwrap();
//! // this machine is the hub at 192.168.7.10: it must admit the group and the hub port
//! let p = plan(Some(&config), &[Ipv4Addr::new(192, 168, 7, 10)], None);
//! assert_eq!(p.networks, ["100.64.0.0/10", "192.168.7.0/24"]);
//! assert!(p.rules.iter().any(|r| r.role == Role::Multicast && r.port == 7741));
//! assert!(p.rules.iter().any(|r| r.role == Role::Hub && r.port == 8791));
//! // another machine of the same fleet is no hub and admits only the group
//! let p = plan(Some(&config), &[Ipv4Addr::new(192, 168, 7, 11)], None);
//! assert!(p.rules.iter().all(|r| r.role == Role::Multicast));
//! ```

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::config::{BroadcastMode, MeshConfig};

/// The transport protocol of a rule: discovery is datagrams, registration and the link
/// protocol are connections. Serialised as the word a firewall front takes.
///
/// ```
/// use majordomus_cli::mesh::firewall::Protocol;
///
/// assert_eq!(serde_json::to_value(Protocol::Udp).unwrap(), "udp");
/// assert_eq!(serde_json::to_value(Protocol::Tcp).unwrap(), "tcp");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// Datagrams: discovery.
    Udp,
    /// Connections: the hub's registrations and the link protocol.
    Tcp,
}

impl Protocol {
    fn as_str(self) -> &'static str {
        match self {
            Protocol::Udp => "udp",
            Protocol::Tcp => "tcp",
        }
    }
}

/// Why a rule exists: which part of the declaration implied it. The role is what a person
/// reads beside the rule and what a verdict names, and it is serialised in snake case.
///
/// ```
/// use majordomus_cli::mesh::firewall::Role;
///
/// assert_eq!(serde_json::to_value(Role::Hub).unwrap(), "hub");
/// let back: Role = serde_json::from_value(serde_json::json!("multicast")).unwrap();
/// assert_eq!(back, Role::Multicast);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The declared multicast group: every runtime of the segment advertises to it.
    Multicast,
    /// The declared broadcast port, when broadcast is a permitted fallback.
    Broadcast,
    /// This machine is a declared rendezvous hub: the fleet registers here.
    Hub,
    /// This machine's server is dialed by peers on the link protocol.
    Link,
}

/// One inbound admission the mesh needs on this host: a protocol and a port, the group it is
/// addressed to when it is multicast, the networks it may come from, and the sentence that
/// says why. A rule is derived, never written by hand; [`FirewallRule::label`] is how a
/// verdict names it.
///
/// ```
/// use majordomus_cli::mesh::firewall::{FirewallRule, Protocol, Role};
///
/// let rule = FirewallRule {
///     role: Role::Hub,
///     protocol: Protocol::Tcp,
///     port: 8791,
///     destination: None,
///     sources: vec!["192.168.7.0/24".into()],
///     reason: "this machine is a declared rendezvous hub".into(),
/// };
/// assert_eq!(rule.label(), "tcp/8791 from 192.168.7.0/24");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FirewallRule {
    /// Why.
    pub role: Role,
    /// The transport protocol.
    pub protocol: Protocol,
    /// The destination port.
    pub port: u16,
    /// The destination address when it is a multicast group; `None` for this host's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    /// The networks the traffic may come from, as CIDR; empty admits any source, which is
    /// only ever the case for the multicast group, whose TTL keeps it on the segment.
    pub sources: Vec<String>,
    /// The sentence a person reads beside the rule.
    pub reason: String,
}

impl FirewallRule {
    /// `tcp/8791 from 192.168.7.0/24, 100.64.0.0/10` — one line for a verdict: the
    /// protocol and port, the group when there is one, and the sources or `any`.
    ///
    /// ```
    /// use majordomus_cli::mesh::firewall::{FirewallRule, Protocol, Role};
    ///
    /// let group = FirewallRule {
    ///     role: Role::Multicast,
    ///     protocol: Protocol::Udp,
    ///     port: 7741,
    ///     destination: Some("239.255.77.77".into()),
    ///     sources: vec![],
    ///     reason: String::new(),
    /// };
    /// assert_eq!(group.label(), "udp/7741 to 239.255.77.77 from any");
    /// ```
    pub fn label(&self) -> String {
        let to = match &self.destination {
            Some(group) => format!("{}/{} to {group}", self.protocol.as_str(), self.port),
            None => format!("{}/{}", self.protocol.as_str(), self.port),
        };
        if self.sources.is_empty() {
            format!("{to} from any")
        } else {
            format!("{to} from {}", self.sources.join(", "))
        }
    }
}

/// What the declaration implies this host must admit: the rules, the networks they admit
/// from, the hub ports this machine is listed for, and the addresses the plan saw. The
/// answer of [`plan`]; an empty `rules` is "nothing is needed", which is the answer for no
/// declaration and for a disabled one.
///
/// ```
/// use majordomus_cli::mesh::firewall::{plan, FirewallPlan};
///
/// let nothing: FirewallPlan = plan(None, &[], None);
/// assert!(nothing.rules.is_empty() && nothing.hub_ports.is_empty() && nothing.networks.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FirewallPlan {
    /// This machine's addresses, as the plan saw them.
    pub local_addresses: Vec<String>,
    /// The private networks peers come from: the enclosing ranges of every address the
    /// declaration names, or of this machine's own when it names none.
    pub networks: Vec<String>,
    /// The ports of the declared hubs this machine is.
    pub hub_ports: Vec<u16>,
    /// The admissions, in a fixed order.
    pub rules: Vec<FirewallRule>,
}

/// The private range an address belongs to, as the source network of a rule; `None` for a
/// public address, which the declaration case refuses and no rule is derived for.
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::enclosing;
///
/// assert_eq!(enclosing(Ipv4Addr::new(192, 168, 100, 10)).as_deref(), Some("192.168.100.0/24"));
/// assert_eq!(enclosing(Ipv4Addr::new(100, 92, 246, 32)).as_deref(), Some("100.64.0.0/10"));
/// assert_eq!(enclosing(Ipv4Addr::new(10, 3, 2, 1)).as_deref(), Some("10.0.0.0/8"));
/// assert_eq!(enclosing(Ipv4Addr::new(172, 20, 0, 1)).as_deref(), Some("172.16.0.0/12"));
/// assert_eq!(enclosing(Ipv4Addr::new(8, 8, 8, 8)), None);
/// ```
pub fn enclosing(ip: Ipv4Addr) -> Option<String> {
    let [a, b, c, _] = ip.octets();
    match (a, b) {
        (10, _) => Some("10.0.0.0/8".into()),
        (172, 16..=31) => Some("172.16.0.0/12".into()),
        (192, 168) => Some(format!("192.168.{c}.0/24")),
        (100, 64..=127) => Some("100.64.0.0/10".into()),
        _ => None,
    }
}

/// `http://192.168.7.10:8791` → the address and the port; a host name, a scheme the mesh
/// does not speak, or a missing port is `None`.
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::parse_endpoint;
///
/// assert_eq!(parse_endpoint("http://192.168.7.10:8791"), Some((Ipv4Addr::new(192, 168, 7, 10), 8791)));
/// assert_eq!(parse_endpoint("http://192.168.7.10:8791/"), Some((Ipv4Addr::new(192, 168, 7, 10), 8791)));
/// assert_eq!(parse_endpoint("192.168.7.10:8791"), Some((Ipv4Addr::new(192, 168, 7, 10), 8791)));
/// assert_eq!(parse_endpoint("http://hub.example:8791"), None);
/// assert_eq!(parse_endpoint("http://192.168.7.10"), None);
/// ```
pub fn parse_endpoint(endpoint: &str) -> Option<(Ipv4Addr, u16)> {
    let rest = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))
        .unwrap_or(endpoint);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port) = authority.rsplit_once(':')?;
    Some((host.parse().ok()?, port.parse().ok()?))
}

/// Derive the admissions from the declaration (or its absence), this machine's addresses
/// and, when its server listens beyond loopback, the server's port.
///
/// No declaration, or a disabled one, needs nothing: the plan is empty and that is the
/// answer. Otherwise: the multicast group's port when multicast is on; the broadcast
/// port when broadcast is a permitted fallback, from the networks it names; every declared
/// hub port whose address is this machine's; and the server's port, from the fleet's
/// networks, when one is given — a server on loopback is dialed by nobody and needs no
/// rule.
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::{plan, Role};
/// use majordomus_cli::mesh::MeshConfig;
///
/// let config: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
///     "cooperation": { "seeds": ["http://10.9.8.7:8741"] },
/// })).unwrap();
/// // no hub here, a server beyond loopback on 8741: the group's port and the link port
/// let p = plan(Some(&config), &[Ipv4Addr::new(10, 9, 8, 1)], Some(8741));
/// let roles: Vec<Role> = p.rules.iter().map(|r| r.role).collect();
/// assert_eq!(roles, [Role::Multicast, Role::Link]);
/// assert_eq!(p.rules[1].sources, ["10.0.0.0/8"]);
/// ```
pub fn plan(
    config: Option<&MeshConfig>,
    local: &[Ipv4Addr],
    server_port: Option<u16>,
) -> FirewallPlan {
    let local_addresses: Vec<String> = local.iter().map(ToString::to_string).collect();
    let Some(config) = config.filter(|c| c.enabled) else {
        return FirewallPlan {
            local_addresses,
            networks: Vec::new(),
            hub_ports: Vec::new(),
            rules: Vec::new(),
        };
    };

    // The networks peers come from: what the declaration names, else what this machine is on.
    let declared: Vec<Ipv4Addr> = config
        .rendezvous
        .endpoints
        .iter()
        .chain(config.cooperation.seeds.iter())
        .filter_map(|e| parse_endpoint(e).map(|(ip, _)| ip))
        .collect();
    let basis: &[Ipv4Addr] = if declared.is_empty() {
        local
    } else {
        &declared
    };
    // A set rather than a Vec and a sort: ordered and free of repeats by construction, and
    // `order-check` counts every sort site in the crate against a baseline it may not exceed.
    let networks: Vec<String> = basis
        .iter()
        .filter_map(|ip| enclosing(*ip))
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .collect();

    let mut rules = Vec::new();
    if config.multicast.enabled {
        rules.push(FirewallRule {
            role: Role::Multicast,
            protocol: Protocol::Udp,
            port: config.multicast.port,
            destination: Some(config.multicast.group.clone()),
            sources: Vec::new(),
            reason: format!(
                "every runtime of the segment advertises to {} on UDP port {}; a datagram to the group that the firewall drops is a peer this machine never hears",
                config.multicast.group, config.multicast.port
            ),
        });
    }
    if config.broadcast.mode != BroadcastMode::Disabled {
        let mut sources = config.broadcast.networks.clone();
        if sources.is_empty() {
            sources.clone_from(&networks);
        }
        // Only the multicast group is admitted from any source. A rule with no source
        // renders as "from anywhere", and with no private network to name, that is a port
        // opened to the internet: a decision for the operator, never one derived here.
        if !sources.is_empty() {
            rules.push(FirewallRule {
                role: Role::Broadcast,
                protocol: Protocol::Udp,
                port: config.broadcast.port,
                destination: None,
                sources,
                reason: format!(
                    "broadcast is a permitted fallback on UDP port {}",
                    config.broadcast.port
                ),
            });
        }
    }

    let hub_ports: std::collections::BTreeSet<u16> = config
        .rendezvous
        .endpoints
        .iter()
        .filter_map(|e| parse_endpoint(e))
        .filter(|(ip, _)| local.contains(ip))
        .map(|(_, port)| port)
        .collect();
    // The TCP ports are admitted from the fleet's private networks and from nowhere else.
    // When none can be derived — a hub declared on a public address, a host with no private
    // address — no TCP rule is planned: the ports stay in `hub_ports` for a reader, and
    // opening one to every source stays the operator's act.
    let private = !networks.is_empty();
    for port in hub_ports.iter().filter(|_| private) {
        rules.push(FirewallRule {
            role: Role::Hub,
            protocol: Protocol::Tcp,
            port: *port,
            destination: None,
            sources: networks.clone(),
            reason: format!(
                "this machine is a declared rendezvous hub on TCP port {port}: every server of the fleet registers here, and the hub is how machines on different segments find each other"
            ),
        });
    }
    if let Some(port) = server_port.filter(|_| private) {
        if !hub_ports.contains(&port) {
            rules.push(FirewallRule {
                role: Role::Link,
                protocol: Protocol::Tcp,
                port,
                destination: None,
                sources: networks.clone(),
                reason: format!(
                    "this checkout's server listens beyond loopback on TCP port {port}: a peer dials it there on the link protocol"
                ),
            });
        }
    }

    FirewallPlan {
        local_addresses,
        networks,
        hub_ports: hub_ports.into_iter().collect(),
        rules,
    }
}

/// The firewall front this host runs, as far as the executable can tell without privilege:
/// decided by [`detect`] from the platform and the path, and the thing [`render`],
/// [`observe`] and [`apply`] are specific to.
///
/// ```
/// use majordomus_cli::mesh::firewall::Backend;
///
/// assert_eq!(serde_json::to_value(Backend::ApplicationFirewall).unwrap(), "application_firewall");
/// assert_eq!(Backend::None.as_str(), "none");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Uncomplicated Firewall, the Debian and Ubuntu front: `ufw` is on the path.
    Ufw,
    /// nftables without ufw in front: `nft` is on the path.
    Nftables,
    /// macOS: the application firewall (`socketfilterfw`), which admits executables, not ports.
    ApplicationFirewall,
    /// No firewall front this executable knows; nothing is observed and nothing is applied.
    None,
}

impl Backend {
    /// The word a person reads in a report or a doctor line, which is the tool's own name
    /// where there is one and a description where the tool is a system service.
    ///
    /// ```
    /// use majordomus_cli::mesh::firewall::Backend;
    ///
    /// assert_eq!(Backend::Ufw.as_str(), "ufw");
    /// assert_eq!(Backend::Nftables.as_str(), "nftables");
    /// assert_eq!(Backend::ApplicationFirewall.as_str(), "macOS application firewall");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Ufw => "ufw",
            Backend::Nftables => "nftables",
            Backend::ApplicationFirewall => "macOS application firewall",
            Backend::None => "none",
        }
    }
}

/// Which backend a platform with these executables on its path runs. Pure: the platform
/// and the lookup are given, so the decision is testable on any machine.
///
/// ```
/// use majordomus_cli::mesh::firewall::{detect, Backend};
///
/// assert_eq!(detect("linux", &|name| name == "ufw" || name == "nft"), Backend::Ufw);
/// assert_eq!(detect("linux", &|name| name == "nft"), Backend::Nftables);
/// assert_eq!(detect("linux", &|_| false), Backend::None);
/// assert_eq!(detect("macos", &|_| false), Backend::ApplicationFirewall);
/// assert_eq!(detect("windows", &|_| true), Backend::None);
/// ```
pub fn detect(os: &str, on_path: &dyn Fn(&str) -> bool) -> Backend {
    match os {
        "macos" => Backend::ApplicationFirewall,
        "linux" if on_path("ufw") => Backend::Ufw,
        "linux" if on_path("nft") => Backend::Nftables,
        _ => Backend::None,
    }
}

/// [`detect`] for this process: its platform, and the path it would run the tool from —
/// `PATH`, and the sbin directories a service manager's path has and a login shell's often
/// lacks, because `ufw` and `nft` live there.
///
/// ```
/// use majordomus_cli::mesh::firewall::{detect, detect_here, Backend};
///
/// let here = detect_here();
/// // the same decision `detect` makes for this platform with a real path lookup
/// let looked_up = detect(std::env::consts::OS, &|name| {
///     std::env::var_os("PATH").map_or(false, |p| {
///         std::env::split_paths(&p)
///             .chain(["/usr/sbin", "/sbin", "/usr/local/sbin"].map(std::path::PathBuf::from))
///             .any(|d| d.join(name).is_file())
///     })
/// });
/// assert_eq!(here, looked_up);
/// ```
pub fn detect_here() -> Backend {
    detect(std::env::consts::OS, &|name| find_tool(name).is_some())
}

/// Where the tool is, if anywhere.
fn find_tool(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in ["/usr/sbin", "/sbin", "/usr/local/sbin"] {
        dirs.push(PathBuf::from(extra));
    }
    dirs.into_iter()
        .map(|d| d.join(name))
        .find(|candidate| candidate.is_file())
}

/// The macOS application firewall's tool.
const SOCKETFILTERFW: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";

/// One command that admits a rule: the rule's label, its argument vector exactly as
/// [`apply`] executes it, and the same quoted as one line a person pastes into a root
/// shell. The two are projections of one vector, so what the report shows is what runs.
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::{plan, render, AdmitCommand, Backend};
/// use majordomus_cli::mesh::MeshConfig;
///
/// let config: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
/// })).unwrap();
/// let p = plan(Some(&config), &[Ipv4Addr::new(10, 0, 0, 2)], None);
/// let commands: Vec<AdmitCommand> = render(Backend::Ufw, &p, None);
/// assert_eq!(commands[0].argv[0], "ufw");
/// assert_eq!(commands[0].rule, "udp/7741 to 239.255.77.77 from any");
/// assert_eq!(commands[0].line, "ufw allow in proto udp to 239.255.77.77 port 7741 comment 'majordomus mesh: multicast discovery'");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdmitCommand {
    /// The rule it admits.
    pub rule: String,
    /// The program and its arguments, as executed.
    pub argv: Vec<String>,
    /// The same, quoted for a shell.
    pub line: String,
}

fn shell_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%,".contains(c))
    {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

fn command(rule: &FirewallRule, argv: Vec<String>) -> AdmitCommand {
    let line = argv
        .iter()
        .map(|w| shell_quote(w))
        .collect::<Vec<_>>()
        .join(" ");
    AdmitCommand {
        rule: rule.label(),
        argv,
        line,
    }
}

/// The comment every rule this module writes carries, so that the rules it admitted can be
/// told from an operator's own.
pub const COMMENT_PREFIX: &str = "majordomus mesh";

fn comment(rule: &FirewallRule) -> String {
    let what = match rule.role {
        Role::Multicast => "multicast discovery",
        Role::Broadcast => "broadcast discovery",
        Role::Hub => "rendezvous hub",
        Role::Link => "link protocol",
    };
    format!("{COMMENT_PREFIX}: {what}")
}

/// The commands that admit the plan on the backend. ufw and nftables take one rule per
/// source network; the macOS application firewall admits an executable, so it takes the
/// executable the server runs as, once, whatever the ports — and nothing, when no
/// executable is given.
///
/// ```
/// use std::net::Ipv4Addr;
/// use std::path::Path;
/// use majordomus_cli::mesh::firewall::{plan, render, Backend};
/// use majordomus_cli::mesh::MeshConfig;
///
/// let config: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
///     "rendezvous": { "endpoints": ["http://192.168.7.10:8791"] },
/// })).unwrap();
/// let p = plan(Some(&config), &[Ipv4Addr::new(192, 168, 7, 10)], None);
/// let lines: Vec<String> = render(Backend::Ufw, &p, None).into_iter().map(|c| c.line).collect();
/// assert_eq!(lines, [
///     "ufw allow in proto udp to 239.255.77.77 port 7741 comment 'majordomus mesh: multicast discovery'",
///     "ufw allow in proto tcp from 192.168.7.0/24 to any port 8791 comment 'majordomus mesh: rendezvous hub'",
/// ]);
/// let mac = render(Backend::ApplicationFirewall, &p, Some(Path::new("/opt/majordomus")));
/// assert_eq!(mac.len(), 2);
/// assert!(mac[0].line.ends_with("--add /opt/majordomus"));
/// assert!(mac[1].line.ends_with("--unblockapp /opt/majordomus"));
/// assert!(render(Backend::None, &p, None).is_empty());
/// ```
pub fn render(
    backend: Backend,
    plan: &FirewallPlan,
    executable: Option<&Path>,
) -> Vec<AdmitCommand> {
    let mut out = Vec::new();
    match backend {
        Backend::Ufw => {
            for rule in &plan.rules {
                let proto = rule.protocol.as_str().to_string();
                let port = rule.port.to_string();
                let note = comment(rule);
                if rule.sources.is_empty() {
                    let mut argv = vec![
                        "ufw".into(),
                        "allow".into(),
                        "in".into(),
                        "proto".into(),
                        proto,
                    ];
                    argv.push("to".into());
                    argv.push(rule.destination.clone().unwrap_or_else(|| "any".into()));
                    argv.extend(["port".into(), port, "comment".into(), note]);
                    out.push(command(rule, argv));
                } else {
                    for source in &rule.sources {
                        let argv = vec![
                            "ufw".into(),
                            "allow".into(),
                            "in".into(),
                            "proto".into(),
                            proto.clone(),
                            "from".into(),
                            source.clone(),
                            "to".into(),
                            rule.destination.clone().unwrap_or_else(|| "any".into()),
                            "port".into(),
                            port.clone(),
                            "comment".into(),
                            note.clone(),
                        ];
                        out.push(command(rule, argv));
                    }
                }
            }
        }
        Backend::Nftables => {
            for rule in &plan.rules {
                let proto = rule.protocol.as_str().to_string();
                let port = rule.port.to_string();
                let note = comment(rule);
                let sources: Vec<Option<&String>> = if rule.sources.is_empty() {
                    vec![None]
                } else {
                    rule.sources.iter().map(Some).collect()
                };
                for source in sources {
                    let mut argv: Vec<String> = ["nft", "add", "rule", "inet", "filter", "input"]
                        .iter()
                        .map(|w| w.to_string())
                        .collect();
                    if let Some(source) = source {
                        argv.extend(["ip".into(), "saddr".into(), source.clone()]);
                    }
                    if let Some(group) = &rule.destination {
                        argv.extend(["ip".into(), "daddr".into(), group.clone()]);
                    }
                    argv.extend([
                        proto.clone(),
                        "dport".into(),
                        port.clone(),
                        "accept".into(),
                        "comment".into(),
                        note.clone(),
                    ]);
                    out.push(command(rule, argv));
                }
            }
        }
        Backend::ApplicationFirewall => {
            if let (Some(exe), Some(rule)) = (executable, plan.rules.first()) {
                let exe = exe.display().to_string();
                out.push(command(
                    rule,
                    vec![SOCKETFILTERFW.into(), "--add".into(), exe.clone()],
                ));
                out.push(command(
                    rule,
                    vec![SOCKETFILTERFW.into(), "--unblockapp".into(), exe],
                ));
            }
        }
        Backend::None => {}
    }
    out
}

/// What the host's firewall says about the plan, as one word. Only `missing` is a failure
/// on its own; `unobservable` is an absence that the kernel log may still turn into one.
///
/// ```
/// use majordomus_cli::mesh::firewall::FirewallObservationState;
///
/// assert_eq!(serde_json::to_value(FirewallObservationState::Unobservable).unwrap(), "unobservable");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FirewallObservationState {
    /// No backend to ask.
    NoBackend,
    /// The firewall is off: nothing filters, nothing is needed.
    Inactive,
    /// Every rule of the plan is admitted.
    Present,
    /// At least one rule of the plan is not admitted.
    Missing,
    /// The firewall could not be asked from this process — it needs root.
    Unobservable,
}

/// The firewall's own word, judged against the plan: the state, the evidence behind it,
/// and the labels of the rules not admitted when the state is `missing`.
///
/// ```
/// use majordomus_cli::mesh::firewall::{judge_ufw, plan, FirewallObservation, FirewallObservationState};
///
/// let o: FirewallObservation = judge_ufw("Status: inactive\n", &plan(None, &[], None));
/// assert_eq!(o.state, FirewallObservationState::Inactive);
/// assert!(o.missing.is_empty() && o.detail.contains("inactive"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FirewallObservation {
    /// The verdict.
    pub state: FirewallObservationState,
    /// The evidence: the status line, the rules found, or the refusal.
    pub detail: String,
    /// The rules not admitted, by label, when `state` is `missing`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

impl FirewallObservation {
    fn of(state: FirewallObservationState, detail: impl Into<String>) -> Self {
        FirewallObservation {
            state,
            detail: detail.into(),
            missing: Vec::new(),
        }
    }
}

/// Judge `ufw status` output against the plan. Pure.
///
/// A rule is admitted when, for each of its sources (or once, for any source), a line
/// allows its `port/proto` inbound from that network or from anywhere, to its group when
/// it has one.
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::{judge_ufw, plan, FirewallObservationState};
/// use majordomus_cli::mesh::MeshConfig;
///
/// let config: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
///     "rendezvous": { "endpoints": ["http://192.168.7.10:8791", "http://100.64.1.2:8791"] },
/// })).unwrap();
/// let p = plan(Some(&config), &[Ipv4Addr::new(192, 168, 7, 10)], None);
///
/// let before = "Status: active\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW IN    Anywhere\n";
/// let o = judge_ufw(before, &p);
/// assert_eq!(o.state, FirewallObservationState::Missing);
/// assert_eq!(o.missing.len(), 2);
///
/// let after = "Status: active\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW IN    Anywhere\n239.255.77.77 7741/udp     ALLOW IN    Anywhere                   # majordomus mesh: multicast discovery\n8791/tcp                   ALLOW IN    192.168.7.0/24             # majordomus mesh: rendezvous hub\n8791/tcp                   ALLOW IN    100.64.0.0/10              # majordomus mesh: rendezvous hub\n";
/// assert_eq!(judge_ufw(after, &p).state, FirewallObservationState::Present);
///
/// // `ufw status` without `verbose` prints the action as `ALLOW`; it is the same rule
/// let terse = "Status: active\n239.255.77.77 7741/udp     ALLOW       Anywhere\n8791/tcp                   ALLOW       192.168.7.0/24\n8791/tcp                   ALLOW       100.64.0.0/10\n";
/// assert_eq!(judge_ufw(terse, &p).state, FirewallObservationState::Present);
///
/// // a wider admission counts: anywhere covers every network
/// let wide = "Status: active\n239.255.77.77 7741/udp     ALLOW IN    Anywhere\n8791/tcp                   ALLOW IN    Anywhere\n";
/// assert_eq!(judge_ufw(wide, &p).state, FirewallObservationState::Present);
///
/// assert_eq!(judge_ufw("Status: inactive\n", &p).state, FirewallObservationState::Inactive);
/// assert_eq!(judge_ufw("ERROR: You need to be root to run this script\n", &p).state, FirewallObservationState::Unobservable);
/// ```
pub fn judge_ufw(status: &str, plan: &FirewallPlan) -> FirewallObservation {
    let lines: Vec<&str> = status.lines().map(str::trim).collect();
    if lines.iter().any(|l| l.contains("need to be root")) {
        return FirewallObservation::of(
            FirewallObservationState::Unobservable,
            "ufw status needs root; run `sudo majordomus mesh firewall` to observe the rules",
        );
    }
    if lines.iter().any(|l| l.starts_with("Status: inactive")) {
        return FirewallObservation::of(
            FirewallObservationState::Inactive,
            "ufw is inactive: nothing filters inbound traffic on this host",
        );
    }
    if !lines.iter().any(|l| l.starts_with("Status: active")) {
        return FirewallObservation::of(
            FirewallObservationState::Unobservable,
            format!(
                "ufw status was not understood: {}",
                lines.first().copied().unwrap_or("(empty)")
            ),
        );
    }
    let allows: Vec<(String, String)> = lines
        .iter()
        .filter_map(|l| {
            // `ufw status` prints `ALLOW`; `ufw status verbose` prints `ALLOW IN`. Both
            // are the same rule; `ALLOW OUT` and `ALLOW FWD` admit nothing inbound.
            let (to, rest) = l.split_once("ALLOW IN").or_else(|| {
                l.split_once("ALLOW ")
                    .filter(|(_, r)| !r.starts_with("OUT") && !r.starts_with("FWD"))
            })?;
            let from = rest
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            Some((to.split_whitespace().collect::<Vec<_>>().join(" "), from))
        })
        .collect();
    let admitted = |rule: &FirewallRule, source: Option<&str>| {
        let token = format!("{}/{}", rule.port, rule.protocol.as_str());
        allows.iter().any(|(to, from)| {
            let to_matches = match &rule.destination {
                Some(group) => *to == format!("{group} {token}"),
                None => *to == token,
            };
            let from_matches = match source {
                None => true,
                Some(network) => from == "Anywhere" || from == network,
            };
            to_matches && from_matches
        })
    };
    judge_with(plan, |rule| {
        if rule.sources.is_empty() {
            admitted(rule, None)
        } else {
            rule.sources.iter().all(|s| admitted(rule, Some(s)))
        }
    })
}

/// Judge `nft list ruleset` output against the plan: a rule is admitted when a line
/// accepts its `dport` (to its group, when it has one; from each source, when it has any).
///
/// ```
/// use std::net::Ipv4Addr;
/// use majordomus_cli::mesh::firewall::{judge_nft, plan, FirewallObservationState};
/// use majordomus_cli::mesh::MeshConfig;
///
/// let config: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
/// })).unwrap();
/// let p = plan(Some(&config), &[Ipv4Addr::new(10, 0, 0, 5)], None);
/// assert_eq!(judge_nft("table inet filter {\n chain input {\n  ip daddr 239.255.77.77 udp dport 7741 accept\n }\n}\n", &p).state, FirewallObservationState::Present);
/// assert_eq!(judge_nft("table inet filter {\n}\n", &p).state, FirewallObservationState::Missing);
/// assert_eq!(judge_nft("", &p).state, FirewallObservationState::Inactive);
/// assert_eq!(judge_nft("Operation not permitted", &p).state, FirewallObservationState::Unobservable);
/// ```
pub fn judge_nft(ruleset: &str, plan: &FirewallPlan) -> FirewallObservation {
    if ruleset.contains("not permitted") || ruleset.contains("Permission denied") {
        return FirewallObservation::of(
            FirewallObservationState::Unobservable,
            "nft list ruleset needs root; run `sudo majordomus mesh firewall` to observe the rules",
        );
    }
    if ruleset.trim().is_empty() {
        return FirewallObservation::of(
            FirewallObservationState::Inactive,
            "the nftables ruleset is empty: nothing filters inbound traffic on this host",
        );
    }
    let accepts: Vec<&str> = ruleset
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("accept"))
        .collect();
    judge_with(plan, |rule| {
        let dport = format!("{} dport {}", rule.protocol.as_str(), rule.port);
        let line_admits = |l: &&str, source: Option<&str>| {
            l.contains(&dport)
                && rule
                    .destination
                    .as_ref()
                    .is_none_or(|g| l.contains(&format!("daddr {g}")))
                && source.is_none_or(|s| !l.contains("saddr") || l.contains(s))
        };
        if rule.sources.is_empty() {
            accepts.iter().any(|l| line_admits(l, None))
        } else {
            rule.sources
                .iter()
                .all(|s| accepts.iter().any(|l| line_admits(l, Some(s))))
        }
    })
}

/// Judge the macOS application firewall against the executable the server runs as: off is
/// inactive; on, the executable must be listed as allowed.
///
/// ```
/// use std::path::Path;
/// use majordomus_cli::mesh::firewall::{judge_application_firewall, FirewallObservationState};
///
/// let exe = Path::new("/opt/majordomus");
/// assert_eq!(judge_application_firewall("Firewall is disabled. (State = 0)", "", exe).state, FirewallObservationState::Inactive);
/// let apps = "ALF: total number of apps = 1 \n\n1 :  /opt/majordomus \n \t ( Allow incoming connections ) \n";
/// assert_eq!(judge_application_firewall("Firewall is enabled. (State = 1)", apps, exe).state, FirewallObservationState::Present);
/// let other = "1 :  /opt/other \n \t ( Allow incoming connections ) \n";
/// assert_eq!(judge_application_firewall("Firewall is enabled. (State = 1)", other, exe).state, FirewallObservationState::Missing);
/// let blocked = "1 :  /opt/majordomus \n \t ( Block incoming connections ) \n";
/// assert_eq!(judge_application_firewall("Firewall is enabled. (State = 1)", blocked, exe).state, FirewallObservationState::Missing);
/// ```
pub fn judge_application_firewall(
    global_state: &str,
    apps: &str,
    executable: &Path,
) -> FirewallObservation {
    if global_state.contains("disabled") || global_state.contains("State = 0") {
        return FirewallObservation::of(
            FirewallObservationState::Inactive,
            "the application firewall is off: nothing filters inbound connections on this host",
        );
    }
    let exe = executable.display().to_string();
    let lines: Vec<&str> = apps.lines().map(str::trim).collect();
    let allowed = lines.iter().enumerate().any(|(i, l)| {
        l.ends_with(&exe)
            && lines
                .get(i + 1)
                .is_some_and(|next| next.contains("Allow incoming"))
    });
    if allowed {
        FirewallObservation::of(
            FirewallObservationState::Present,
            format!("the application firewall allows incoming connections to {exe}"),
        )
    } else {
        FirewallObservation {
            state: FirewallObservationState::Missing,
            detail: format!(
                "the application firewall is on and does not allow incoming connections to {exe}"
            ),
            missing: vec![format!("incoming connections to {exe}")],
        }
    }
}

fn judge_with(
    plan: &FirewallPlan,
    admitted: impl Fn(&FirewallRule) -> bool,
) -> FirewallObservation {
    let missing: Vec<String> = plan
        .rules
        .iter()
        .filter(|r| !admitted(r))
        .map(FirewallRule::label)
        .collect();
    if missing.is_empty() {
        FirewallObservation::of(
            FirewallObservationState::Present,
            format!(
                "every rule of the plan is admitted ({} rule(s))",
                plan.rules.len()
            ),
        )
    } else {
        FirewallObservation {
            state: FirewallObservationState::Missing,
            detail: format!(
                "{} of {} rule(s) not admitted: {}",
                missing.len(),
                plan.rules.len(),
                missing.join("; ")
            ),
            missing,
        }
    }
}

/// How this module asks a host tool: the program and its arguments in; everything it
/// printed, or the reason it could not run, out. [`run`] is the one that reaches the host.
/// The judgement around the asking takes the asker as an argument, so that every branch of
/// it is exercised by a test that answers from a table — without a firewall, without root
/// and on any platform — while the public functions pass [`run`] and nothing else.
type Runner<'a> = &'a dyn Fn(&str, &[&str]) -> std::result::Result<String, String>;

fn run(program: &str, args: &[&str]) -> std::result::Result<String, String> {
    let program = find_tool(program).unwrap_or_else(|| PathBuf::from(program));
    match Command::new(&program).args(args).output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            Ok(text)
        }
        Err(e) => Err(format!("{} could not run: {e}", program.display())),
    }
}

/// Ask the host's firewall about the plan. The judgement is pure; this is the asking, and
/// with no backend there is nothing to ask.
///
/// ```
/// use majordomus_cli::mesh::firewall::{observe, plan, Backend, FirewallObservationState};
///
/// let o = observe(Backend::None, &plan(None, &[], None), None);
/// assert_eq!(o.state, FirewallObservationState::NoBackend);
/// // the application firewall admits an executable, and without one it cannot be judged
/// let o = observe(Backend::ApplicationFirewall, &plan(None, &[], None), None);
/// assert_eq!(o.state, FirewallObservationState::Unobservable);
/// ```
pub fn observe(
    backend: Backend,
    plan: &FirewallPlan,
    executable: Option<&Path>,
) -> FirewallObservation {
    observe_with(backend, plan, executable, &run)
}

/// [`observe`], asking through `run`.
fn observe_with(
    backend: Backend,
    plan: &FirewallPlan,
    executable: Option<&Path>,
    run: Runner<'_>,
) -> FirewallObservation {
    match backend {
        Backend::None => FirewallObservation::of(
            FirewallObservationState::NoBackend,
            "no firewall front this executable knows (ufw, nft, the macOS application firewall) is on this host; nothing is observed",
        ),
        Backend::Ufw => match run("ufw", &["status"]) {
            Ok(text) => judge_ufw(&text, plan),
            Err(e) => FirewallObservation::of(FirewallObservationState::Unobservable, e),
        },
        Backend::Nftables => match run("nft", &["list", "ruleset"]) {
            Ok(text) => judge_nft(&text, plan),
            Err(e) => FirewallObservation::of(FirewallObservationState::Unobservable, e),
        },
        Backend::ApplicationFirewall => {
            let Some(exe) = executable else {
                return FirewallObservation::of(
                    FirewallObservationState::Unobservable,
                    "the application firewall admits an executable, and none was given",
                );
            };
            match (
                run(SOCKETFILTERFW, &["--getglobalstate"]),
                run(SOCKETFILTERFW, &["--listapps"]),
            ) {
                (Ok(state), Ok(apps)) => judge_application_firewall(&state, &apps, exe),
                (Err(e), _) | (_, Err(e)) => FirewallObservation::of(FirewallObservationState::Unobservable, e),
            }
        }
    }
}

/// Count, per port of the plan, the `[UFW BLOCK]` lines of a kernel log that name it as
/// the destination. Pure.
///
/// ```
/// use majordomus_cli::mesh::firewall::count_blocks;
///
/// let log = "[UFW BLOCK] IN=eth0 SRC=192.168.7.11 DST=239.255.77.77 PROTO=UDP SPT=7741 DPT=7741 LEN=430\n\
///            [UFW BLOCK] IN=eth0 SRC=192.168.7.11 DST=192.168.7.10 PROTO=TCP SPT=57168 DPT=8791 WINDOW=65535\n\
///            [UFW BLOCK] IN=eth0 SRC=192.168.7.11 DST=192.168.7.10 PROTO=TCP SPT=57169 DPT=8791 WINDOW=65535\n\
///            [UFW BLOCK] IN=eth0 SRC=1.2.3.4 DST=192.168.7.10 PROTO=TCP SPT=1 DPT=22 WINDOW=1\n\
///            perf: interrupt took too long\n";
/// let counts = count_blocks(log, &[7741, 8791]);
/// assert_eq!(counts.get("7741"), Some(&1));
/// assert_eq!(counts.get("8791"), Some(&2));
/// assert_eq!(counts.get("22"), None);
/// ```
pub fn count_blocks(log: &str, ports: &[u16]) -> BTreeMap<String, u64> {
    let mut counts: BTreeMap<String, u64> = ports.iter().map(|p| (p.to_string(), 0)).collect();
    for line in log.lines().filter(|l| l.contains("[UFW BLOCK]")) {
        for port in ports {
            let token = format!("DPT={port} ");
            if line.contains(&token) || line.trim_end().ends_with(&format!("DPT={port}")) {
                *counts.entry(port.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// The window [`kernel_blocks`] reads, in seconds: long enough to hold several
/// advertisements (every 15 s by default) and registrations (every 30 s) from any peer
/// the firewall drops, short enough that a rule just admitted is seen to work within
/// minutes instead of an hour.
pub const BLOCK_WINDOW_SECONDS: u64 = 300;

/// What the kernel logged the firewall dropping toward the plan's ports in the last
/// [`BLOCK_WINDOW_SECONDS`], per port; `None` when the log cannot be read from this
/// process (no `journalctl`, or no membership of the groups that may read it), which is
/// an absence, not a verdict. An empty plan asks nothing.
///
/// ```
/// use majordomus_cli::mesh::firewall::{kernel_blocks, plan};
///
/// assert!(kernel_blocks(&plan(None, &[], None)).is_none());
/// ```
pub fn kernel_blocks(plan: &FirewallPlan) -> Option<BTreeMap<String, u64>> {
    kernel_blocks_with(plan, std::env::consts::OS, &run)
}

/// [`kernel_blocks`] for the platform `os`, reading the log through `run`.
fn kernel_blocks_with(
    plan: &FirewallPlan,
    os: &str,
    run: Runner<'_>,
) -> Option<BTreeMap<String, u64>> {
    if plan.rules.is_empty() || os != "linux" {
        return None;
    }
    let ports: Vec<u16> = plan.rules.iter().map(|r| r.port).collect();
    let since = format!("-{BLOCK_WINDOW_SECONDS}s");
    let text = run(
        "journalctl",
        &["-k", "--since", &since, "-o", "cat", "-q", "--no-pager"],
    )
    .ok()?;
    if text.contains("No journal files") || text.contains("Permission denied") {
        return None;
    }
    Some(count_blocks(&text, &ports))
}

/// The whole report: the plan, the backend, the commands, the firewall's word and the
/// kernel's, with one verdict sentence and one `ok`. What `mesh.firewall` answers and
/// what the doctor's `firewall` check reads.
///
/// ```
/// use majordomus_cli::mesh::firewall::{report, FirewallReport, BLOCK_WINDOW_SECONDS};
///
/// let r: FirewallReport = report(None, &[], None);
/// assert!(r.ok && r.commands.is_empty());
/// assert_eq!(r.window_seconds, BLOCK_WINDOW_SECONDS);
/// assert_eq!(r.platform, std::env::consts::OS);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FirewallReport {
    /// Whether the host admits the mesh as far as it can be observed: nothing missing, and
    /// nothing logged as dropped.
    pub ok: bool,
    /// The platform (`linux`, `macos`, …).
    pub platform: String,
    /// The firewall front found.
    pub backend: Backend,
    /// The executable the server runs as, which is what the macOS firewall admits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// What the declaration implies this host must admit.
    pub plan: FirewallPlan,
    /// The commands that admit it on this backend; what `apply` runs.
    pub commands: Vec<AdmitCommand>,
    /// The firewall's own word about the plan.
    pub observation: FirewallObservation,
    /// The window `blocked_recently` covers, in seconds.
    pub window_seconds: u64,
    /// Drops the kernel logged toward the plan's ports within the window, per port;
    /// absent when the log could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_recently: Option<BTreeMap<String, u64>>,
    /// The sentence a person reads.
    pub verdict: String,
}

/// Build the report for this host from the declaration, the addresses and the server's
/// port, asking the firewall and the kernel log — and asking neither when the plan is
/// empty, because nothing is needed.
///
/// ```
/// use majordomus_cli::mesh::firewall::{report, FirewallObservationState};
///
/// let r = report(None, &[], None);
/// assert_eq!(r.observation.state, FirewallObservationState::Present);
/// assert!(r.blocked_recently.is_none());
/// assert!(r.verdict.contains("needs no inbound admission"));
/// ```
pub fn report(
    config: Option<&MeshConfig>,
    local: &[Ipv4Addr],
    server_port: Option<u16>,
) -> FirewallReport {
    let host = Host {
        os: std::env::consts::OS,
        backend: detect_here(),
        executable: std::env::current_exe().ok(),
        run: &run,
    };
    report_with(config, local, server_port, host)
}

/// The host a report is built on: its platform, the firewall front found there, the
/// executable the server runs as, and how its tools are asked. [`report`] fills it from
/// this process; a test fills it with a platform and answers of its own, which is how the
/// verdict of every backend and every observation is held on a machine that has neither.
struct Host<'a> {
    os: &'a str,
    backend: Backend,
    executable: Option<PathBuf>,
    run: Runner<'a>,
}

/// [`report`] on the host given.
fn report_with(
    config: Option<&MeshConfig>,
    local: &[Ipv4Addr],
    server_port: Option<u16>,
    host: Host<'_>,
) -> FirewallReport {
    let Host {
        os,
        backend,
        executable,
        run,
    } = host;
    let plan = plan(config, local, server_port);
    let commands = render(backend, &plan, executable.as_deref());
    let observation = if plan.rules.is_empty() {
        FirewallObservation::of(
            FirewallObservationState::Present,
            "the mesh needs no inbound admission on this host: no enabled declaration, or nothing declared that reaches this machine",
        )
    } else {
        observe_with(backend, &plan, executable.as_deref(), run)
    };
    let blocked_recently = kernel_blocks_with(&plan, os, run);
    let dropped: u64 = blocked_recently
        .as_ref()
        .map(|m| m.values().sum())
        .unwrap_or(0);
    let minutes = BLOCK_WINDOW_SECONDS / 60;
    let ok = observation.state != FirewallObservationState::Missing && dropped == 0;
    let verdict = match (observation.state, dropped) {
        (FirewallObservationState::Missing, _) => format!(
            "the host firewall does not admit the mesh: {}; run `sudo majordomus mesh firewall apply`",
            observation.detail
        ),
        (FirewallObservationState::Unobservable, n) if n > 0 => format!(
            "the kernel logged {n} inbound datagram(s) or connection(s) toward the mesh's ports dropped in the last {minutes} minutes, and the firewall could not be asked without root: run `sudo majordomus mesh firewall apply`"
        ),
        (_, n) if n > 0 => format!(
            "the kernel logged {n} inbound datagram(s) or connection(s) toward the mesh's ports dropped in the last {minutes} minutes although the firewall admits the mesh on paper: a rule above the mesh's, or another filter, still drops them"
        ),
        (FirewallObservationState::Present, _) if plan.rules.is_empty() => observation.detail.clone(),
        (FirewallObservationState::Present, _) => format!(
            "the host firewall admits the mesh: {}",
            observation.detail
        ),
        (FirewallObservationState::Inactive, _) => observation.detail.clone(),
        (FirewallObservationState::NoBackend, _) => observation.detail.clone(),
        (FirewallObservationState::Unobservable, _) => format!(
            "not observed: {}; {}",
            observation.detail,
            match &blocked_recently {
                Some(_) => format!(
                    "the kernel log shows nothing dropped toward the mesh's ports in the last {minutes} minutes"
                ),
                None => "the kernel log could not be read either".to_string(),
            }
        ),
    };
    FirewallReport {
        ok,
        platform: os.to_string(),
        backend,
        executable: executable.map(|p| p.display().to_string()),
        plan,
        commands,
        observation,
        window_seconds: BLOCK_WINDOW_SECONDS,
        blocked_recently,
        verdict,
    }
}

/// One command [`apply`] ran: the line, whether it exited 0 without printing an error,
/// and everything it printed, so a refusal by the tool is read in the report and not in
/// a log.
///
/// ```
/// use majordomus_cli::mesh::firewall::Applied;
///
/// let a: Applied = serde_json::from_value(serde_json::json!({
///     "line": "ufw allow in proto udp to 239.255.77.77 port 7741", "ok": true, "output": "Rule added",
/// })).unwrap();
/// assert!(a.ok && a.output == "Rule added");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Applied {
    /// The line that ran.
    pub line: String,
    /// Whether it exited 0.
    pub ok: bool,
    /// What it printed.
    pub output: String,
}

/// What [`apply`] did: the refusal when nothing ran, every command that ran with its
/// outcome, the firewall's word afterwards, and the report the commands came from. `ok`
/// is the firewall's verdict after the run, never the commands' exit codes alone.
///
/// ```
/// use majordomus_cli::mesh::firewall::{apply, report, FirewallApplyReport};
///
/// let a: FirewallApplyReport = apply(report(None, &[], None));
/// assert!(!a.ok && a.applied.is_empty());
/// assert!(a.refused.as_deref().unwrap().contains("nothing to apply"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FirewallApplyReport {
    /// Whether every command ran and the firewall admits the plan afterwards.
    pub ok: bool,
    /// The refusal, when nothing ran: not root, or no backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
    /// The commands, in order, with their exit and output.
    pub applied: Vec<Applied>,
    /// The firewall's word after the commands ran.
    pub observation: FirewallObservation,
    /// The report the commands came from.
    pub before: FirewallReport,
}

#[cfg(unix)]
fn is_root() -> bool {
    // SAFETY: geteuid reads the process's effective user id and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_root() -> bool {
    true
}

/// Run the report's commands on this host. Refuses — runs nothing — without root and
/// without a backend; every command that runs is recorded with its exit and its output,
/// and the firewall is asked again afterwards so the answer is its word, not the
/// commands'.
///
/// ```
/// use majordomus_cli::mesh::firewall::{apply, report};
///
/// // an empty plan is refused before any privilege is needed, and nothing runs
/// let a = apply(report(None, &[], None));
/// assert!(a.refused.is_some() && a.applied.is_empty());
/// assert_eq!(a.observation, a.before.observation);
/// ```
pub fn apply(before: FirewallReport) -> FirewallApplyReport {
    apply_with(before, is_root(), &run)
}

/// [`apply`] with the privilege and the asker given: `root` is whether this process may
/// change the firewall, and `run` is what runs each command and asks the firewall after.
fn apply_with(before: FirewallReport, root: bool, run: Runner<'_>) -> FirewallApplyReport {
    let refuse = |before: FirewallReport, reason: String| FirewallApplyReport {
        ok: false,
        refused: Some(reason),
        applied: Vec::new(),
        observation: before.observation.clone(),
        before,
    };
    if before.backend == Backend::None {
        return refuse(
            before,
            "no firewall front this executable knows is on this host; nothing to apply".into(),
        );
    }
    if before.commands.is_empty() {
        return refuse(
            before,
            "the plan needs no admission on this host; nothing to apply".into(),
        );
    }
    if !root {
        return refuse(
            before,
            "applying firewall rules needs root: run `sudo majordomus mesh firewall apply` (sudo -E keeps the repository and state paths of the invoking user)".into(),
        );
    }
    let mut applied = Vec::new();
    let mut all_ok = true;
    for command in &before.commands {
        let program = command.argv.first().cloned().unwrap_or_default();
        let args: Vec<&str> = command.argv.iter().skip(1).map(String::as_str).collect();
        let (ok, output) = match run(&program, &args) {
            Ok(text) => (!text.contains("ERROR"), text),
            Err(e) => (false, e),
        };
        all_ok &= ok;
        applied.push(Applied {
            line: command.line.clone(),
            ok,
            output: output.trim().to_string(),
        });
    }
    let exe = before.executable.as_deref().map(Path::new);
    let observation = observe_with(before.backend, &before.plan, exe, run);
    let ok = all_ok
        && matches!(
            observation.state,
            FirewallObservationState::Present | FirewallObservationState::Inactive
        );
    FirewallApplyReport {
        ok,
        refused: None,
        applied,
        observation,
        before,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(json: serde_json::Value) -> MeshConfig {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn no_declaration_and_a_disabled_one_need_nothing() {
        let p = plan(None, &[Ipv4Addr::new(192, 168, 1, 2)], Some(8741));
        assert!(p.rules.is_empty() && p.networks.is_empty());
        let off = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": false,
            "rendezvous": { "endpoints": ["http://192.168.1.2:8791"] },
        }));
        let p = plan(Some(&off), &[Ipv4Addr::new(192, 168, 1, 2)], Some(8741));
        assert!(p.rules.is_empty(), "a disabled mesh opens nothing: {p:?}");
    }

    #[test]
    fn the_networks_come_from_the_declaration_and_fall_back_to_this_machine() {
        let declared = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "rendezvous": { "endpoints": ["http://10.1.2.3:8791", "http://100.65.22.118:8791"] },
            "cooperation": { "seeds": ["http://192.168.50.4:8741"] },
        }));
        let p = plan(Some(&declared), &[Ipv4Addr::new(172, 17, 0, 1)], None);
        assert_eq!(
            p.networks,
            ["10.0.0.0/8", "100.64.0.0/10", "192.168.50.0/24"]
        );
        let bare = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
        }));
        let p = plan(
            Some(&bare),
            &[Ipv4Addr::new(172, 17, 0, 1), Ipv4Addr::new(192, 168, 9, 9)],
            Some(8741),
        );
        assert_eq!(p.networks, ["172.16.0.0/12", "192.168.9.0/24"]);
        let link = p
            .rules
            .iter()
            .find(|r| r.role == Role::Link)
            .expect("a server beyond loopback is dialed");
        assert_eq!((link.protocol, link.port), (Protocol::Tcp, 8741));
        assert_eq!(link.sources, p.networks);
    }

    #[test]
    fn a_hub_port_that_is_also_the_server_port_is_one_rule() {
        let c = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "multicast": { "enabled": false },
            "rendezvous": { "endpoints": ["http://192.168.1.2:8791", "http://192.168.1.2:8791"] },
        }));
        let p = plan(Some(&c), &[Ipv4Addr::new(192, 168, 1, 2)], Some(8791));
        assert_eq!(p.hub_ports, [8791]);
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].role, Role::Hub);
        assert_eq!(p.rules[0].label(), "tcp/8791 from 192.168.1.0/24");
    }

    #[test]
    fn a_public_hub_address_derives_no_network() {
        let c = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "rendezvous": { "endpoints": ["http://203.0.113.5:8791"] },
        }));
        let p = plan(Some(&c), &[Ipv4Addr::new(203, 0, 113, 5)], Some(8741));
        assert!(p.networks.is_empty());
        assert_eq!(p.hub_ports, [8791], "the port is still named for a reader");
        // No private network to admit from, so no TCP rule at all: one with no source would
        // render as a port opened to every address, and this plan never derives that.
        assert!(
            p.rules
                .iter()
                .all(|r| r.role == Role::Multicast && r.protocol == Protocol::Udp),
            "only the group's own port is admitted from any source: {:?}",
            p.rules
        );
    }

    #[test]
    fn broadcast_with_no_network_to_admit_from_is_not_planned() {
        // Broadcast is on, it names no network, and the one declared address is public: there
        // is nothing to admit from, and a rule without a source would admit everybody.
        let c = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "multicast": { "enabled": false },
            "broadcast": { "mode": "explicit", "port": 7742 },
            "rendezvous": { "endpoints": ["http://203.0.113.5:8791"] },
        }));
        let p = plan(Some(&c), &[Ipv4Addr::new(203, 0, 113, 5)], Some(8741));
        assert!(p.networks.is_empty());
        assert!(p.rules.is_empty(), "nothing is derived: {:?}", p.rules);
    }

    #[test]
    fn a_rule_is_rendered_as_it_is_given_so_the_plan_is_what_keeps_a_port_closed() {
        // `render` is faithful to the rule it is handed: one with no source and no group is
        // written "to any". That is why `plan` never derives such a rule for a TCP or a
        // broadcast port, and this is the only place one exists.
        let open = FirewallPlan {
            local_addresses: Vec::new(),
            networks: Vec::new(),
            hub_ports: vec![8791],
            rules: vec![FirewallRule {
                role: Role::Hub,
                protocol: Protocol::Tcp,
                port: 8791,
                destination: None,
                sources: Vec::new(),
                reason: "hand-built".into(),
            }],
        };
        let ufw = render(Backend::Ufw, &open, None);
        assert_eq!(ufw.len(), 1, "{ufw:#?}");
        assert!(
            ufw[0]
                .line
                .starts_with("ufw allow in proto tcp to any port 8791 comment"),
            "{}",
            ufw[0].line
        );
    }

    #[test]
    fn broadcast_takes_its_own_networks_or_the_fleets() {
        let own = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "multicast": { "enabled": false },
            "broadcast": { "mode": "explicit", "port": 7742, "networks": ["192.168.1.0/24"] },
        }));
        let p = plan(Some(&own), &[Ipv4Addr::new(10, 0, 0, 1)], None);
        let b = &p.rules[0];
        assert_eq!(
            (b.role, b.protocol, b.port),
            (Role::Broadcast, Protocol::Udp, 7742)
        );
        assert_eq!(b.sources, ["192.168.1.0/24"]);
    }

    #[test]
    fn the_rendered_commands_quote_their_comments_and_name_every_source() {
        let c = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "rendezvous": { "endpoints": ["http://192.168.1.2:8791", "http://100.64.0.9:8791"] },
        }));
        let p = plan(Some(&c), &[Ipv4Addr::new(192, 168, 1, 2)], None);
        let ufw = render(Backend::Ufw, &p, None);
        assert_eq!(ufw.len(), 3, "{ufw:#?}");
        assert!(ufw
            .iter()
            .all(|c| c.argv.last().unwrap().starts_with(COMMENT_PREFIX)));
        assert!(
            ufw[1].line.contains("from 100.64.0.0/10")
                && ufw[2].line.contains("from 192.168.1.0/24")
        );
        let nft = render(Backend::Nftables, &p, None);
        assert_eq!(nft.len(), 3);
        assert!(nft[0].line.starts_with(
            "nft add rule inet filter input ip daddr 239.255.77.77 udp dport 7741 accept comment"
        ));
        assert!(nft[1]
            .line
            .contains("ip saddr 100.64.0.0/10 tcp dport 8791 accept"));
        assert!(
            render(Backend::ApplicationFirewall, &p, None).is_empty(),
            "no executable, nothing to admit"
        );
    }

    #[test]
    fn the_report_of_an_empty_plan_is_ok_without_asking_anything() {
        let r = report(None, &[], None);
        assert!(r.ok);
        assert_eq!(r.observation.state, FirewallObservationState::Present);
        assert!(r.commands.is_empty());
        assert!(r.blocked_recently.is_none());
    }

    #[test]
    fn apply_refuses_before_it_runs_anything_it_should_not() {
        let r = report(None, &[], None);
        let a = apply(r);
        assert!(!a.ok && a.applied.is_empty());
        assert!(a.refused.is_some());
    }

    #[test]
    fn ufw_judgement_requires_each_source_unless_anywhere_covers_it() {
        let c = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "multicast": { "enabled": false },
            "rendezvous": { "endpoints": ["http://192.168.1.2:8791", "http://100.64.0.9:8791"] },
        }));
        let p = plan(Some(&c), &[Ipv4Addr::new(192, 168, 1, 2)], None);
        let one = "Status: active\n8791/tcp                   ALLOW IN    192.168.1.0/24\n";
        let o = judge_ufw(one, &p);
        assert_eq!(o.state, FirewallObservationState::Missing, "{o:?}");
        let both = "Status: active\n8791/tcp                   ALLOW IN    192.168.1.0/24\n8791/tcp                   ALLOW IN    100.64.0.0/10\n";
        assert_eq!(judge_ufw(both, &p).state, FirewallObservationState::Present);
        // a rule on another port, or outbound, admits nothing
        let other = "Status: active\n8790/tcp                   ALLOW IN    Anywhere\n8791/tcp                   ALLOW OUT   Anywhere\n";
        assert_eq!(
            judge_ufw(other, &p).state,
            FirewallObservationState::Missing
        );
    }

    #[test]
    fn blocks_are_counted_only_toward_the_plans_ports() {
        let log = "[UFW BLOCK] IN=e SRC=1.1.1.1 DST=2.2.2.2 PROTO=TCP DPT=8791\n[UFW BLOCK] IN=e DPT=87911 LEN=1\n";
        let counts = count_blocks(log, &[8791]);
        assert_eq!(counts["8791"], 1);
    }

    // ------------------------------------------------------------ the fixtures below
    //
    // Everything from here on is held without a firewall: the fleet is the one of the
    // incident this module answers, the firewall's words are texts, and the host is a
    // table. No test below runs a firewall tool, and none depends on which one — if any —
    // the machine running the suite has.

    /// The LAN hub of [`fleet`].
    const HUB: Ipv4Addr = Ipv4Addr::new(192, 168, 7, 10);

    /// Two hubs, one on the LAN and one on the tailnet, and the multicast group on.
    fn fleet() -> MeshConfig {
        config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "rendezvous": { "endpoints": ["http://192.168.7.10:8791", "http://100.64.1.2:8791"] },
        }))
    }

    /// What the LAN hub of [`fleet`] must admit: the group, and its port from both networks.
    fn hub_plan() -> FirewallPlan {
        plan(Some(&fleet()), &[HUB], None)
    }

    const GROUP_LABEL: &str = "udp/7741 to 239.255.77.77 from any";
    const HUB_LABEL: &str = "tcp/8791 from 100.64.0.0/10, 192.168.7.0/24";

    /// ufw on, with nothing of the mesh admitted: the host of the incident.
    const UFW_BARE: &str = concat!(
        "Status: active\n",
        "\n",
        "To                         Action      From\n",
        "--                         ------      ----\n",
        "22/tcp                     ALLOW IN    Anywhere\n",
    );

    /// ufw on, after `apply`: every rule of [`hub_plan`] admitted, each with its comment.
    const UFW_ADMITTED: &str = concat!(
        "Status: active\n",
        "\n",
        "To                         Action      From\n",
        "--                         ------      ----\n",
        "22/tcp                     ALLOW IN    Anywhere\n",
        "239.255.77.77 7741/udp     ALLOW IN    Anywhere                   ",
        "# majordomus mesh: multicast discovery\n",
        "8791/tcp                   ALLOW IN    192.168.7.0/24             ",
        "# majordomus mesh: rendezvous hub\n",
        "8791/tcp                   ALLOW IN    100.64.0.0/10              ",
        "# majordomus mesh: rendezvous hub\n",
    );

    /// What ufw answers a process that is not root.
    const UFW_ROOT: &str = "ERROR: You need to be root to run this script\n";

    /// An nftables ruleset that admits every rule of [`hub_plan`].
    const NFT_ADMITTED: &str = concat!(
        "table inet filter {\n",
        " chain input {\n",
        "  ip daddr 239.255.77.77 udp dport 7741 accept\n",
        "  ip saddr 192.168.7.0/24 tcp dport 8791 accept\n",
        "  ip saddr 100.64.0.0/10 tcp dport 8791 accept\n",
        " }\n",
        "}\n",
    );

    /// A kernel log of the window with nothing of the firewall's in it.
    const JOURNAL_QUIET: &str = "perf: interrupt took too long\n";

    /// A kernel log of the window with one drop toward each port of [`hub_plan`].
    const JOURNAL_DROPS: &str = concat!(
        "[UFW BLOCK] IN=eth0 OUT= SRC=192.168.7.11 DST=239.255.77.77 ",
        "PROTO=UDP SPT=7741 DPT=7741 LEN=430\n",
        "[UFW BLOCK] IN=eth0 OUT= SRC=192.168.7.11 DST=192.168.7.10 ",
        "PROTO=TCP SPT=57168 DPT=8791 WINDOW=65535\n",
        "[UFW BLOCK] IN=eth0 OUT= SRC=203.0.113.9 DST=192.168.7.10 ",
        "PROTO=TCP SPT=40000 DPT=22 WINDOW=1024\n",
    );

    const FIREWALL_ON: &str = "Firewall is enabled. (State = 1)";
    const EXECUTABLE: &str = "/opt/majordomus";
    const APPS_ALLOWED: &str = concat!(
        "ALF: total number of apps = 1 \n",
        "\n",
        "1 :  /opt/majordomus \n",
        " \t ( Allow incoming connections ) \n",
    );
    const APPS_OTHER: &str = "1 :  /opt/other \n \t ( Allow incoming connections ) \n";

    /// A host that answers from a table. Each entry is a fragment of a command line and
    /// what the tool prints for the command carrying it; a command no entry names is a
    /// tool this host does not have, answered the way [`run`] answers for one. Every
    /// command is recorded, so a test holds what was asked — and that nothing was.
    struct Script {
        answers: Vec<(&'static str, &'static str)>,
        asked: std::cell::RefCell<Vec<String>>,
    }

    impl Script {
        fn new(answers: &[(&'static str, &'static str)]) -> Self {
            Script {
                answers: answers.to_vec(),
                asked: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn run(&self, program: &str, args: &[&str]) -> std::result::Result<String, String> {
            let mut words = vec![program];
            words.extend_from_slice(args);
            let line = words.join(" ");
            self.asked.borrow_mut().push(line.clone());
            self.answers
                .iter()
                .find(|(fragment, _)| line.contains(fragment))
                .map(|(_, printed)| printed.to_string())
                .ok_or_else(|| format!("{program} could not run: no such tool on this host"))
        }

        fn asked(&self) -> Vec<String> {
            self.asked.borrow().clone()
        }
    }

    /// The report of [`fleet`]'s LAN hub on the host a test describes.
    fn hub_report(
        os: &str,
        backend: Backend,
        executable: Option<&str>,
        host: &Script,
    ) -> FirewallReport {
        let host = Host {
            os,
            backend,
            executable: executable.map(PathBuf::from),
            run: &|program, args| host.run(program, args),
        };
        report_with(Some(&fleet()), &[HUB], None, host)
    }

    // ------------------------------------------------------------ the plan and its words

    #[test]
    fn a_rule_is_labelled_by_its_protocol_its_port_its_group_and_its_sources() {
        let p = hub_plan();
        assert_eq!(p.rules[0].label(), GROUP_LABEL);
        assert_eq!(p.rules[1].label(), HUB_LABEL);
        let lone = FirewallRule {
            role: Role::Link,
            protocol: Protocol::Tcp,
            port: 8741,
            destination: None,
            sources: Vec::new(),
            reason: String::new(),
        };
        assert_eq!(lone.label(), "tcp/8741 from any");
        // a rule without a group does not carry the member at all: an absent destination
        // is this host's own address, and `null` would read as a group nobody named
        let wire = serde_json::to_value(&lone).unwrap();
        assert!(wire.get("destination").is_none(), "{wire}");
        assert_eq!(wire["role"], "link");
        assert_eq!(serde_json::from_value::<FirewallRule>(wire).unwrap(), lone);
    }

    #[test]
    fn every_private_range_encloses_its_addresses_and_a_public_address_has_none() {
        let range = |a, b, c, d| enclosing(Ipv4Addr::new(a, b, c, d));
        assert_eq!(
            range(192, 168, 100, 10).as_deref(),
            Some("192.168.100.0/24")
        );
        assert_eq!(range(100, 92, 246, 32).as_deref(), Some("100.64.0.0/10"));
        assert_eq!(range(10, 3, 2, 1).as_deref(), Some("10.0.0.0/8"));
        assert_eq!(range(172, 20, 0, 1).as_deref(), Some("172.16.0.0/12"));
        // the edges of each range, and the public addresses right beside them
        assert_eq!(range(172, 16, 0, 1).as_deref(), Some("172.16.0.0/12"));
        assert_eq!(range(172, 31, 255, 254).as_deref(), Some("172.16.0.0/12"));
        assert_eq!(range(100, 64, 0, 1).as_deref(), Some("100.64.0.0/10"));
        assert_eq!(range(100, 127, 255, 254).as_deref(), Some("100.64.0.0/10"));
        for public in [
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::new(172, 15, 0, 1),
            Ipv4Addr::new(172, 32, 0, 1),
            Ipv4Addr::new(100, 63, 0, 1),
            Ipv4Addr::new(100, 128, 0, 1),
            Ipv4Addr::new(192, 169, 0, 1),
        ] {
            assert_eq!(enclosing(public), None, "{public} is not a private address");
        }
    }

    #[test]
    fn an_endpoint_is_an_address_and_a_port_whatever_scheme_or_path_surrounds_them() {
        let hub = Some((HUB, 8791));
        assert_eq!(parse_endpoint("http://192.168.7.10:8791"), hub);
        assert_eq!(parse_endpoint("https://192.168.7.10:8791"), hub);
        assert_eq!(parse_endpoint("192.168.7.10:8791"), hub);
        assert_eq!(parse_endpoint("http://192.168.7.10:8791/"), hub);
        assert_eq!(
            parse_endpoint("http://192.168.7.10:8791/api/v1?x=1#top"),
            hub
        );
        assert_eq!(parse_endpoint("http://192.168.7.10:8791?x=1"), hub);
        assert_eq!(parse_endpoint("http://192.168.7.10:8791#top"), hub);
        // a name is not an address this machine can be compared with, and a firewall
        // rule is not derived from a resolver's answer
        assert_eq!(parse_endpoint("http://hub.example:8791"), None);
        assert_eq!(parse_endpoint("http://192.168.7.10"), None);
        assert_eq!(parse_endpoint("http://192.168.7.10:http"), None);
        assert_eq!(parse_endpoint("http://192.168.7.10:99999"), None);
        assert_eq!(parse_endpoint(""), None);
    }

    #[test]
    fn a_declared_hub_admits_the_group_and_its_port_and_every_other_machine_only_the_group() {
        let p = hub_plan();
        assert_eq!(p.local_addresses, ["192.168.7.10"]);
        assert_eq!(p.networks, ["100.64.0.0/10", "192.168.7.0/24"]);
        assert_eq!(p.hub_ports, [8791]);
        let group = &p.rules[0];
        assert_eq!(
            (group.role, group.protocol, group.port),
            (Role::Multicast, Protocol::Udp, 7741)
        );
        assert_eq!(group.destination.as_deref(), Some("239.255.77.77"));
        assert!(group.sources.is_empty() && group.reason.contains("239.255.77.77"));
        let hub = &p.rules[1];
        assert_eq!(
            (hub.role, hub.protocol, hub.port),
            (Role::Hub, Protocol::Tcp, 8791)
        );
        assert_eq!(hub.sources, p.networks);
        assert!(hub.reason.contains("TCP port 8791"), "{}", hub.reason);
        // another machine of the same fleet is no hub: the networks are the fleet's all
        // the same, because they are what its peers come from
        let other = plan(Some(&fleet()), &[Ipv4Addr::new(192, 168, 7, 11)], None);
        assert!(other.hub_ports.is_empty());
        assert!(other.rules.iter().all(|r| r.role == Role::Multicast));
        assert_eq!(other.networks, p.networks);
    }

    #[test]
    fn a_server_beyond_loopback_is_admitted_on_its_port_from_the_networks_of_the_seeds() {
        let seeded = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "cooperation": { "seeds": ["http://10.9.8.7:8741"] },
        }));
        let p = plan(Some(&seeded), &[Ipv4Addr::new(10, 9, 8, 1)], Some(8741));
        let roles: Vec<Role> = p.rules.iter().map(|r| r.role).collect();
        assert_eq!(roles, [Role::Multicast, Role::Link]);
        assert_eq!(p.rules[1].sources, ["10.0.0.0/8"]);
        assert!(p.rules[1].reason.contains("TCP port 8741"));
        // on loopback nobody dials it, and no port is given: no rule
        let quiet = plan(Some(&seeded), &[Ipv4Addr::new(10, 9, 8, 1)], None);
        assert!(quiet.rules.iter().all(|r| r.role != Role::Link));
    }

    #[test]
    fn broadcast_that_names_no_network_of_its_own_is_admitted_from_the_fleets() {
        let auto = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "multicast": { "enabled": false },
            "broadcast": { "mode": "auto" },
            "rendezvous": { "endpoints": ["http://192.168.7.10:8791"] },
        }));
        let p = plan(Some(&auto), &[Ipv4Addr::new(192, 168, 7, 11)], None);
        assert_eq!(p.rules.len(), 1, "{p:?}");
        let b = &p.rules[0];
        assert_eq!(
            (b.role, b.protocol, b.port),
            (Role::Broadcast, Protocol::Udp, 7741)
        );
        assert_eq!(b.destination, None);
        assert_eq!(b.sources, ["192.168.7.0/24"]);
        assert!(b.reason.contains("UDP port 7741"), "{}", b.reason);
    }

    #[test]
    fn every_word_a_report_carries_is_the_one_a_reader_of_the_json_matches_on() {
        let word = |v: serde_json::Value| v.as_str().map(str::to_string);
        assert_eq!(
            word(serde_json::json!(Protocol::Udp)).as_deref(),
            Some("udp")
        );
        assert_eq!(
            word(serde_json::json!(Protocol::Tcp)).as_deref(),
            Some("tcp")
        );
        assert_eq!(word(serde_json::json!(Role::Hub)).as_deref(), Some("hub"));
        assert_eq!(
            word(serde_json::json!(Role::Broadcast)).as_deref(),
            Some("broadcast")
        );
        let back: Role = serde_json::from_value(serde_json::json!("multicast")).unwrap();
        assert_eq!(back, Role::Multicast);
        assert_eq!(
            word(serde_json::json!(Backend::ApplicationFirewall)).as_deref(),
            Some("application_firewall")
        );
        assert_eq!(
            word(serde_json::json!(FirewallObservationState::NoBackend)).as_deref(),
            Some("no_backend")
        );
        assert_eq!(
            word(serde_json::json!(FirewallObservationState::Unobservable)).as_deref(),
            Some("unobservable")
        );
        let ran: Applied = serde_json::from_value(serde_json::json!({
            "line": "ufw allow in proto udp to 239.255.77.77 port 7741",
            "ok": true,
            "output": "Rule added",
        }))
        .unwrap();
        assert!(ran.ok && ran.output == "Rule added" && ran.line.starts_with("ufw allow"));
    }

    // ------------------------------------------------------------ the backend

    #[test]
    fn a_backend_is_named_by_its_tool_or_described_where_it_is_a_system_service() {
        assert_eq!(Backend::Ufw.as_str(), "ufw");
        assert_eq!(Backend::Nftables.as_str(), "nftables");
        assert_eq!(
            Backend::ApplicationFirewall.as_str(),
            "macOS application firewall"
        );
        assert_eq!(Backend::None.as_str(), "none");
    }

    #[test]
    fn the_backend_is_decided_by_the_platform_and_by_what_is_on_the_path() {
        assert_eq!(
            detect("linux", &|name| name == "ufw" || name == "nft"),
            Backend::Ufw,
            "ufw is a front of nftables: where both are, ufw is the one to ask"
        );
        assert_eq!(detect("linux", &|name| name == "ufw"), Backend::Ufw);
        assert_eq!(detect("linux", &|name| name == "nft"), Backend::Nftables);
        assert_eq!(detect("linux", &|_| false), Backend::None);
        assert_eq!(
            detect("macos", &|_| false),
            Backend::ApplicationFirewall,
            "the application firewall is part of the system, not a tool on the path"
        );
        assert_eq!(detect("macos", &|_| true), Backend::ApplicationFirewall);
        assert_eq!(detect("windows", &|_| true), Backend::None);
        assert_eq!(detect("freebsd", &|_| true), Backend::None);
    }

    #[test]
    fn this_process_detects_what_its_platform_and_its_path_decide() {
        let looked_up = detect(std::env::consts::OS, &|name| find_tool(name).is_some());
        assert_eq!(detect_here(), looked_up);
        assert_eq!(find_tool("majordomus-no-such-firewall-tool"), None);
    }

    // ------------------------------------------------------------ the commands

    #[test]
    fn a_word_is_quoted_for_a_shell_only_when_the_shell_would_read_it_otherwise() {
        assert_eq!(shell_quote("ufw"), "ufw");
        assert_eq!(shell_quote("192.168.7.0/24"), "192.168.7.0/24");
        assert_eq!(shell_quote(SOCKETFILTERFW), SOCKETFILTERFW);
        assert_eq!(
            shell_quote("majordomus mesh: rendezvous hub"),
            "'majordomus mesh: rendezvous hub'"
        );
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(reboot)"), "'$(reboot)'");
    }

    #[test]
    fn every_role_is_admitted_with_the_comment_that_names_it_on_ufw_and_on_nftables() {
        let all = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "broadcast": { "mode": "auto" },
            "rendezvous": { "endpoints": ["http://192.168.7.10:8791"] },
        }));
        let p = plan(Some(&all), &[HUB], Some(8741));
        let roles: Vec<Role> = p.rules.iter().map(|r| r.role).collect();
        assert_eq!(
            roles,
            [Role::Multicast, Role::Broadcast, Role::Hub, Role::Link]
        );
        let comments = [
            "majordomus mesh: multicast discovery",
            "majordomus mesh: broadcast discovery",
            "majordomus mesh: rendezvous hub",
            "majordomus mesh: link protocol",
        ];

        let ufw = render(Backend::Ufw, &p, None);
        let lines: Vec<&str> = ufw.iter().map(|c| c.line.as_str()).collect();
        assert_eq!(
            lines,
            [
                "ufw allow in proto udp to 239.255.77.77 port 7741 comment 'majordomus mesh: multicast discovery'",
                "ufw allow in proto udp from 192.168.7.0/24 to any port 7741 comment 'majordomus mesh: broadcast discovery'",
                "ufw allow in proto tcp from 192.168.7.0/24 to any port 8791 comment 'majordomus mesh: rendezvous hub'",
                "ufw allow in proto tcp from 192.168.7.0/24 to any port 8741 comment 'majordomus mesh: link protocol'",
            ]
        );
        for (command, (rule, comment)) in ufw.iter().zip(p.rules.iter().zip(comments)) {
            assert_eq!(command.rule, rule.label());
            assert_eq!(command.argv.last().map(String::as_str), Some(comment));
            // the line is the vector quoted, word for word: what is shown is what runs
            let quoted: Vec<String> = command.argv.iter().map(|w| shell_quote(w)).collect();
            assert_eq!(command.line, quoted.join(" "));
        }

        let nft = render(Backend::Nftables, &p, None);
        assert_eq!(nft.len(), 4, "{nft:#?}");
        let heads = [
            "nft add rule inet filter input ip daddr 239.255.77.77 udp dport 7741 accept comment ",
            "nft add rule inet filter input ip saddr 192.168.7.0/24 udp dport 7741 accept comment ",
            "nft add rule inet filter input ip saddr 192.168.7.0/24 tcp dport 8791 accept comment ",
            "nft add rule inet filter input ip saddr 192.168.7.0/24 tcp dport 8741 accept comment ",
        ];
        for ((command, head), (rule, comment)) in
            nft.iter().zip(heads).zip(p.rules.iter().zip(comments))
        {
            assert!(command.line.starts_with(head), "{}", command.line);
            assert_eq!(command.rule, rule.label());
            assert_eq!(command.argv.last().map(String::as_str), Some(comment));
        }
    }

    #[test]
    fn a_rule_from_two_networks_is_one_command_per_network_and_no_backend_is_no_command() {
        let p = hub_plan();
        let lines: Vec<String> = render(Backend::Ufw, &p, None)
            .into_iter()
            .map(|c| c.line)
            .collect();
        assert_eq!(
            lines,
            [
                "ufw allow in proto udp to 239.255.77.77 port 7741 comment 'majordomus mesh: multicast discovery'",
                "ufw allow in proto tcp from 100.64.0.0/10 to any port 8791 comment 'majordomus mesh: rendezvous hub'",
                "ufw allow in proto tcp from 192.168.7.0/24 to any port 8791 comment 'majordomus mesh: rendezvous hub'",
            ]
        );
        let nft = render(Backend::Nftables, &p, None);
        assert_eq!(nft.len(), 3);
        assert!(nft[1].argv.contains(&"100.64.0.0/10".to_string()));
        assert!(nft[2].argv.contains(&"192.168.7.0/24".to_string()));
        assert!(render(Backend::None, &p, None).is_empty());
        assert!(render(Backend::None, &p, Some(Path::new(EXECUTABLE))).is_empty());
    }

    #[test]
    fn the_application_firewall_admits_the_executable_once_whatever_the_ports() {
        let p = hub_plan();
        let exe = Path::new(EXECUTABLE);
        let mac = render(Backend::ApplicationFirewall, &p, Some(exe));
        let argv: Vec<Vec<&str>> = mac
            .iter()
            .map(|c| c.argv.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            argv,
            [
                [SOCKETFILTERFW, "--add", EXECUTABLE],
                [SOCKETFILTERFW, "--unblockapp", EXECUTABLE],
            ]
        );
        assert!(mac[0].line.ends_with("--add /opt/majordomus"));
        assert!(mac[1].line.ends_with("--unblockapp /opt/majordomus"));
        assert!(mac.iter().all(|c| c.rule == GROUP_LABEL));
        // nothing to admit, nothing to run — with an executable or without
        let nothing = plan(None, &[HUB], None);
        assert!(render(Backend::ApplicationFirewall, &nothing, Some(exe)).is_empty());
        assert!(render(Backend::ApplicationFirewall, &p, None).is_empty());
    }

    // ------------------------------------------------------------ the firewall's own word

    #[test]
    fn ufw_is_judged_present_missing_inactive_or_unobservable_from_its_status_alone() {
        let p = hub_plan();
        let before = judge_ufw(UFW_BARE, &p);
        assert_eq!(before.state, FirewallObservationState::Missing);
        assert_eq!(before.missing, [GROUP_LABEL, HUB_LABEL]);
        assert_eq!(
            before.detail,
            format!("2 of 2 rule(s) not admitted: {GROUP_LABEL}; {HUB_LABEL}")
        );

        let after = judge_ufw(UFW_ADMITTED, &p);
        assert_eq!(after.state, FirewallObservationState::Present);
        assert!(after.missing.is_empty());
        assert_eq!(
            after.detail,
            "every rule of the plan is admitted (2 rule(s))"
        );

        let off = judge_ufw("Status: inactive\n", &p);
        assert_eq!(off.state, FirewallObservationState::Inactive);
        assert!(off.missing.is_empty() && off.detail.contains("inactive"));

        let unasked = judge_ufw(UFW_ROOT, &p);
        assert_eq!(unasked.state, FirewallObservationState::Unobservable);
        assert!(unasked.detail.contains("sudo majordomus mesh firewall"));
    }

    #[test]
    fn a_ufw_status_nobody_understands_is_unobservable_and_quotes_what_it_said() {
        let p = hub_plan();
        let odd = judge_ufw("ufw: command not found\nsecond line\n", &p);
        assert_eq!(odd.state, FirewallObservationState::Unobservable);
        assert_eq!(
            odd.detail,
            "ufw status was not understood: ufw: command not found"
        );
        let silent = judge_ufw("", &p);
        assert_eq!(silent.state, FirewallObservationState::Unobservable);
        assert_eq!(silent.detail, "ufw status was not understood: (empty)");
    }

    #[test]
    fn ufw_admits_by_the_terse_action_and_by_anywhere_and_never_by_an_outbound_rule() {
        let p = hub_plan();
        // `ufw status` without `verbose` prints the action as `ALLOW`; it is the same rule
        let terse = concat!(
            "Status: active\n",
            "239.255.77.77 7741/udp     ALLOW       Anywhere\n",
            "8791/tcp                   ALLOW       192.168.7.0/24\n",
            "8791/tcp                   ALLOW       100.64.0.0/10\n",
        );
        assert_eq!(
            judge_ufw(terse, &p).state,
            FirewallObservationState::Present
        );
        // a wider admission counts: anywhere covers every network
        let wide = concat!(
            "Status: active\n",
            "239.255.77.77 7741/udp     ALLOW IN    Anywhere\n",
            "8791/tcp                   ALLOW IN    Anywhere\n",
        );
        assert_eq!(judge_ufw(wide, &p).state, FirewallObservationState::Present);
        // outbound and forwarded traffic is not what reaches this host's socket, a denial
        // is not an admission, and the group's port to another address is not the group's
        let elsewhere = concat!(
            "Status: active\n",
            "7741/udp                   ALLOW IN    Anywhere\n",
            "239.255.77.77 7741/udp     ALLOW OUT   Anywhere\n",
            "239.255.77.77 7741/udp     DENY IN     Anywhere\n",
            "8791/tcp                   ALLOW OUT   Anywhere\n",
            "8791/tcp                   ALLOW FWD   Anywhere\n",
            "8791/tcp                   DENY        192.168.7.0/24\n",
        );
        let o = judge_ufw(elsewhere, &p);
        assert_eq!(o.state, FirewallObservationState::Missing);
        assert_eq!(o.missing, [GROUP_LABEL, HUB_LABEL]);
        // one network of two is the rule still missing, named whole
        let half = concat!(
            "Status: active\n",
            "239.255.77.77 7741/udp     ALLOW IN    Anywhere\n",
            "8791/tcp                   ALLOW IN    192.168.7.0/24\n",
        );
        let o = judge_ufw(half, &p);
        assert_eq!(o.missing, [HUB_LABEL]);
        assert_eq!(
            o.detail,
            format!("1 of 2 rule(s) not admitted: {HUB_LABEL}")
        );
        // an empty plan is admitted by any active firewall: there is nothing to drop
        let nothing = judge_ufw(UFW_BARE, &plan(None, &[], None));
        assert_eq!(nothing.state, FirewallObservationState::Present);
    }

    #[test]
    fn nftables_is_judged_present_missing_inactive_or_unobservable_from_its_ruleset_alone() {
        let p = hub_plan();
        assert_eq!(
            judge_nft(NFT_ADMITTED, &p).state,
            FirewallObservationState::Present
        );
        let empty_table = judge_nft("table inet filter {\n}\n", &p);
        assert_eq!(empty_table.state, FirewallObservationState::Missing);
        assert_eq!(empty_table.missing, [GROUP_LABEL, HUB_LABEL]);
        for nothing in ["", "  \n\n"] {
            let o = judge_nft(nothing, &p);
            assert_eq!(o.state, FirewallObservationState::Inactive);
            assert!(o.detail.contains("ruleset is empty"), "{}", o.detail);
        }
        for refusal in [
            "Operation not permitted (you must be root)",
            "netlink: Error: cache initialization failed: Permission denied",
        ] {
            let o = judge_nft(refusal, &p);
            assert_eq!(o.state, FirewallObservationState::Unobservable);
            assert!(o.detail.contains("sudo majordomus mesh firewall"));
        }
    }

    #[test]
    fn an_nftables_accept_covers_a_source_it_names_or_every_source_when_it_names_none() {
        let p = hub_plan();
        // an accept that names no source admits every one of them
        let any_source = concat!(
            "  ip daddr 239.255.77.77 udp dport 7741 accept\n",
            "  tcp dport 8791 accept\n",
        );
        assert_eq!(
            judge_nft(any_source, &p).state,
            FirewallObservationState::Present
        );
        // one network of two admitted is the rule still missing
        let half = concat!(
            "  ip daddr 239.255.77.77 udp dport 7741 accept\n",
            "  ip saddr 192.168.7.0/24 tcp dport 8791 accept\n",
        );
        let o = judge_nft(half, &p);
        assert_eq!(o.state, FirewallObservationState::Missing);
        assert_eq!(o.missing, [HUB_LABEL]);
        // the port to another address is not the group, and a drop is not an accept
        let elsewhere = concat!(
            "  udp dport 7741 accept\n",
            "  ip saddr 192.168.7.0/24 tcp dport 8791 drop\n",
            "  ip saddr 100.64.0.0/10 tcp dport 8791 drop\n",
            "  tcp dport 22 accept\n",
        );
        let o = judge_nft(elsewhere, &p);
        assert_eq!(o.missing, [GROUP_LABEL, HUB_LABEL]);
    }

    #[test]
    fn the_application_firewall_is_judged_by_whether_the_executable_is_allowed() {
        let exe = Path::new(EXECUTABLE);
        for off in ["Firewall is disabled. (State = 0)", "(State = 0)"] {
            let o = judge_application_firewall(off, "", exe);
            assert_eq!(o.state, FirewallObservationState::Inactive, "{off}");
            assert!(o.missing.is_empty());
        }
        let allowed = judge_application_firewall(FIREWALL_ON, APPS_ALLOWED, exe);
        assert_eq!(allowed.state, FirewallObservationState::Present);
        assert!(allowed.detail.ends_with(EXECUTABLE), "{}", allowed.detail);

        let blocked = "1 :  /opt/majordomus \n \t ( Block incoming connections ) \n";
        // listed last with no verdict line after it is not listed as allowed either
        let cut_short = "1 :  /opt/majordomus";
        for apps in [APPS_OTHER, blocked, cut_short, ""] {
            let o = judge_application_firewall(FIREWALL_ON, apps, exe);
            assert_eq!(o.state, FirewallObservationState::Missing, "{apps:?}");
            assert_eq!(o.missing, ["incoming connections to /opt/majordomus"]);
            assert!(o.detail.contains("does not allow"), "{}", o.detail);
        }
    }

    // ------------------------------------------------------------ the asking

    #[test]
    fn each_backend_is_asked_its_own_question_and_judged_by_its_own_answer() {
        let p = hub_plan();
        let host = Script::new(&[
            ("ufw status", UFW_ADMITTED),
            ("nft list ruleset", "table inet filter {\n}\n"),
        ]);
        let ufw = observe_with(Backend::Ufw, &p, None, &|program, args| {
            host.run(program, args)
        });
        assert_eq!(ufw.state, FirewallObservationState::Present);
        let nft = observe_with(Backend::Nftables, &p, None, &|program, args| {
            host.run(program, args)
        });
        assert_eq!(nft.state, FirewallObservationState::Missing);
        assert_eq!(host.asked(), ["ufw status", "nft list ruleset"]);
    }

    #[test]
    fn a_firewall_tool_that_cannot_run_is_unobservable_and_the_reason_is_the_evidence() {
        let p = hub_plan();
        let host = Script::new(&[]);
        for (backend, tool) in [(Backend::Ufw, "ufw"), (Backend::Nftables, "nft")] {
            let o = observe_with(backend, &p, None, &|program, args| host.run(program, args));
            assert_eq!(o.state, FirewallObservationState::Unobservable);
            assert_eq!(
                o.detail,
                format!("{tool} could not run: no such tool on this host")
            );
        }
        // the application firewall is two questions, and either failing is the same absence
        let exe = Path::new(EXECUTABLE);
        for answered in [
            ("--getglobalstate", FIREWALL_ON),
            ("--listapps", APPS_ALLOWED),
        ] {
            let host = Script::new(&[answered]);
            let o = observe_with(
                Backend::ApplicationFirewall,
                &p,
                Some(exe),
                &|program, args| host.run(program, args),
            );
            assert_eq!(o.state, FirewallObservationState::Unobservable, "{o:?}");
            assert!(o.detail.contains("could not run"), "{}", o.detail);
        }
    }

    #[test]
    fn the_application_firewall_is_asked_its_state_and_its_list_for_the_executable_given() {
        let p = hub_plan();
        let host = Script::new(&[
            ("--getglobalstate", FIREWALL_ON),
            ("--listapps", APPS_ALLOWED),
        ]);
        let o = observe_with(
            Backend::ApplicationFirewall,
            &p,
            Some(Path::new(EXECUTABLE)),
            &|program, args| host.run(program, args),
        );
        assert_eq!(o.state, FirewallObservationState::Present, "{o:?}");
        assert_eq!(
            host.asked(),
            [
                format!("{SOCKETFILTERFW} --getglobalstate"),
                format!("{SOCKETFILTERFW} --listapps"),
            ]
        );
        // it admits an executable, and without one there is nothing to ask about
        let unasked = Script::new(&[]);
        let o = observe_with(Backend::ApplicationFirewall, &p, None, &|program, args| {
            unasked.run(program, args)
        });
        assert_eq!(o.state, FirewallObservationState::Unobservable);
        assert!(o.detail.contains("none was given"), "{}", o.detail);
        let o = observe_with(Backend::None, &p, None, &|program, args| {
            unasked.run(program, args)
        });
        assert_eq!(o.state, FirewallObservationState::NoBackend);
        assert!(unasked.asked().is_empty(), "{:?}", unasked.asked());
    }

    #[test]
    fn observing_without_a_backend_or_without_an_executable_asks_nothing_of_this_host() {
        // The two answers of the real `observe` that run no tool, on any platform.
        let nothing = plan(None, &[], None);
        let o = observe(Backend::None, &nothing, None);
        assert_eq!(o.state, FirewallObservationState::NoBackend);
        let o = observe(Backend::ApplicationFirewall, &hub_plan(), None);
        assert_eq!(o.state, FirewallObservationState::Unobservable);
    }

    #[test]
    fn a_host_tool_answers_with_everything_it_printed_and_a_missing_one_with_the_reason() {
        // git is the one tool every machine that builds this crate has, and asking it for
        // its version changes nothing anywhere.
        let version = run("git", &["--version"]).unwrap();
        assert!(version.starts_with("git version"), "{version}");
        // What a tool says on standard error is part of its answer: a refusal is read
        // there, and `apply` records it instead of leaving it in a log.
        let refused = run("git", &["--majordomus-no-such-option"]).unwrap();
        assert!(refused.contains("--majordomus-no-such-option"), "{refused}");
        let absent = run("majordomus-no-such-firewall-tool", &[]).unwrap_err();
        assert!(
            absent.starts_with("majordomus-no-such-firewall-tool could not run: "),
            "{absent}"
        );
    }

    // ------------------------------------------------------------ the kernel's word

    #[test]
    fn a_drop_is_counted_for_the_port_it_names_wherever_the_port_stands_in_the_line() {
        let counts = count_blocks(JOURNAL_DROPS, &[7741, 8791]);
        assert_eq!(counts.get("7741"), Some(&1));
        assert_eq!(counts.get("8791"), Some(&1));
        assert_eq!(counts.get("22"), None, "port 22 is not the mesh's");
        // the port as the last word of the line, with and without trailing space, twice
        let tail = "[UFW BLOCK] IN=e DPT=8791\n[UFW BLOCK] IN=e DPT=8791  \nDPT=8791 unrelated\n";
        let counts = count_blocks(tail, &[7741, 8791]);
        assert_eq!(counts["8791"], 2);
        assert_eq!(
            counts["7741"], 0,
            "a port nothing was dropped toward counts zero"
        );
        assert!(count_blocks(JOURNAL_DROPS, &[]).is_empty());
    }

    #[test]
    fn the_kernel_log_is_read_on_linux_for_a_plan_with_rules_and_nowhere_else() {
        let host = Script::new(&[("journalctl", JOURNAL_DROPS)]);
        let nothing = plan(None, &[], None);
        let read = |plan: &FirewallPlan, os: &str| {
            kernel_blocks_with(plan, os, &|program, args| host.run(program, args))
        };
        assert_eq!(read(&nothing, "linux"), None, "an empty plan asks nothing");
        assert_eq!(read(&hub_plan(), "macos"), None);
        assert!(host.asked().is_empty(), "{:?}", host.asked());

        let counts = read(&hub_plan(), "linux").unwrap();
        assert_eq!(counts.len(), 2);
        assert_eq!((counts["7741"], counts["8791"]), (1, 1));
        assert_eq!(
            host.asked(),
            [format!(
                "journalctl -k --since -{BLOCK_WINDOW_SECONDS}s -o cat -q --no-pager"
            )]
        );
        // the real one, on this platform: an empty plan is never a question
        assert!(kernel_blocks(&nothing).is_none());
    }

    #[test]
    fn a_kernel_log_that_cannot_be_read_is_an_absence_and_never_a_count_of_zero() {
        let p = hub_plan();
        let absent = Script::new(&[]);
        let none = kernel_blocks_with(&p, "linux", &|program, args| absent.run(program, args));
        assert_eq!(none, None, "no journalctl on this host");
        for refusal in [
            "No journal files were found.\n",
            "Failed to open journal: Permission denied\n",
        ] {
            let host = Script::new(&[("journalctl", refusal)]);
            let none = kernel_blocks_with(&p, "linux", &|program, args| host.run(program, args));
            assert_eq!(none, None, "{refusal}");
        }
        let quiet = Script::new(&[("journalctl", JOURNAL_QUIET)]);
        let zero = kernel_blocks_with(&p, "linux", &|program, args| quiet.run(program, args));
        assert_eq!(
            zero.map(|m| m.values().sum::<u64>()),
            Some(0),
            "a log that was read and holds no drop is a count, and the count is zero"
        );
    }

    // ------------------------------------------------------------ the report

    #[test]
    fn a_rule_observed_missing_fails_the_report_and_the_verdict_says_how_to_admit_it() {
        let host = Script::new(&[("ufw status", UFW_BARE), ("journalctl", JOURNAL_QUIET)]);
        let r = hub_report("linux", Backend::Ufw, None, &host);
        assert!(!r.ok);
        assert_eq!((r.platform.as_str(), r.backend), ("linux", Backend::Ufw));
        assert_eq!(r.executable, None);
        assert_eq!(r.window_seconds, BLOCK_WINDOW_SECONDS);
        assert_eq!(r.observation.state, FirewallObservationState::Missing);
        assert_eq!(r.observation.missing, [GROUP_LABEL, HUB_LABEL]);
        assert_eq!(
            r.verdict,
            format!(
                "the host firewall does not admit the mesh: {}; run `sudo majordomus mesh firewall apply`",
                r.observation.detail
            )
        );
        // the commands are the plan's, rendered for the backend found
        assert_eq!(r.commands, render(Backend::Ufw, &r.plan, None));
        assert_eq!(r.commands.len(), 3);
        let dropped = r.blocked_recently.as_ref().unwrap();
        assert_eq!((dropped["7741"], dropped["8791"]), (0, 0));
    }

    #[test]
    fn a_logged_drop_fails_the_report_whatever_the_firewall_says_or_cannot_say() {
        let unasked = Script::new(&[("ufw status", UFW_ROOT), ("journalctl", JOURNAL_DROPS)]);
        let r = hub_report("linux", Backend::Ufw, None, &unasked);
        assert!(!r.ok);
        assert_eq!(r.observation.state, FirewallObservationState::Unobservable);
        assert!(
            r.verdict.starts_with("the kernel logged 2 inbound")
                && r.verdict.contains("in the last 5 minutes")
                && r.verdict.contains("could not be asked without root")
                && r.verdict
                    .ends_with("run `sudo majordomus mesh firewall apply`"),
            "{}",
            r.verdict
        );

        // admitted on paper and dropped all the same: another rule, or another filter
        let on_paper = Script::new(&[("ufw status", UFW_ADMITTED), ("journalctl", JOURNAL_DROPS)]);
        let r = hub_report("linux", Backend::Ufw, None, &on_paper);
        assert!(!r.ok);
        assert_eq!(r.observation.state, FirewallObservationState::Present);
        assert!(
            r.verdict.starts_with("the kernel logged 2 inbound")
                && r.verdict
                    .contains("although the firewall admits the mesh on paper"),
            "{}",
            r.verdict
        );
    }

    #[test]
    fn a_firewall_that_admits_the_plan_or_filters_nothing_holds_with_its_own_words() {
        let admitted = Script::new(&[("ufw status", UFW_ADMITTED), ("journalctl", JOURNAL_QUIET)]);
        let r = hub_report("linux", Backend::Ufw, None, &admitted);
        assert!(r.ok);
        assert_eq!(
            r.verdict,
            "the host firewall admits the mesh: every rule of the plan is admitted (2 rule(s))"
        );
        assert_eq!(
            admitted.asked().len(),
            2,
            "the firewall once, the log once: {:?}",
            admitted.asked()
        );

        let off = Script::new(&[("ufw status", "Status: inactive\n")]);
        let r = hub_report("linux", Backend::Ufw, None, &off);
        assert!(r.ok);
        assert_eq!(r.observation.state, FirewallObservationState::Inactive);
        assert_eq!(r.verdict, r.observation.detail);
        assert!(r.verdict.contains("inactive"), "{}", r.verdict);

        // nftables without ufw in front, asked and judged the same way
        let nft = Script::new(&[("nft list ruleset", NFT_ADMITTED)]);
        let r = hub_report("linux", Backend::Nftables, None, &nft);
        assert!(r.ok);
        assert_eq!(r.commands, render(Backend::Nftables, &r.plan, None));
    }

    #[test]
    fn a_host_without_a_known_firewall_holds_and_says_that_nothing_was_observed() {
        let host = Script::new(&[]);
        let r = hub_report("freebsd", Backend::None, None, &host);
        assert!(r.ok);
        assert_eq!(r.platform, "freebsd");
        assert_eq!(r.observation.state, FirewallObservationState::NoBackend);
        assert_eq!(r.verdict, r.observation.detail);
        assert!(r.verdict.contains("nothing is observed"), "{}", r.verdict);
        assert!(r.commands.is_empty() && r.blocked_recently.is_none());
        assert_eq!(r.plan, hub_plan(), "the plan is derived all the same");
        assert!(host.asked().is_empty(), "{:?}", host.asked());
    }

    #[test]
    fn a_firewall_that_cannot_be_asked_holds_and_says_what_the_kernel_log_showed() {
        let quiet = Script::new(&[("ufw status", UFW_ROOT), ("journalctl", JOURNAL_QUIET)]);
        let r = hub_report("linux", Backend::Ufw, None, &quiet);
        assert!(r.ok, "an absence is not a verdict: {}", r.verdict);
        assert!(
            r.verdict.starts_with("not observed: ufw status needs root")
                && r.verdict.ends_with(
                    "the kernel log shows nothing dropped toward the mesh's ports in the last 5 minutes"
                ),
            "{}",
            r.verdict
        );

        let blind = Script::new(&[("ufw status", UFW_ROOT)]);
        let r = hub_report("linux", Backend::Ufw, None, &blind);
        assert!(r.ok);
        assert!(r.blocked_recently.is_none());
        assert!(
            r.verdict
                .ends_with("the kernel log could not be read either"),
            "{}",
            r.verdict
        );
    }

    #[test]
    fn an_empty_plan_is_reported_as_needing_nothing_on_any_host_and_asks_none_of_them() {
        let host = Script::new(&[]);
        for (os, backend) in [
            ("linux", Backend::Ufw),
            ("linux", Backend::Nftables),
            ("macos", Backend::ApplicationFirewall),
            ("freebsd", Backend::None),
        ] {
            let on = Host {
                os,
                backend,
                executable: Some(PathBuf::from(EXECUTABLE)),
                run: &|program, args| host.run(program, args),
            };
            let r = report_with(None, &[HUB], Some(8741), on);
            assert!(r.ok, "{os}: {}", r.verdict);
            assert_eq!(r.plan.local_addresses, ["192.168.7.10"]);
            assert!(r.plan.rules.is_empty() && r.commands.is_empty());
            assert_eq!(r.observation.state, FirewallObservationState::Present);
            assert_eq!(r.verdict, r.observation.detail);
            assert!(r.verdict.contains("needs no inbound admission"));
            assert!(r.blocked_recently.is_none());
            assert_eq!(r.executable.as_deref(), Some(EXECUTABLE));
        }
        assert!(host.asked().is_empty(), "{:?}", host.asked());
    }

    #[test]
    fn the_report_of_a_mac_names_the_executable_it_asked_the_application_firewall_about() {
        let host = Script::new(&[
            ("--getglobalstate", FIREWALL_ON),
            ("--listapps", APPS_ALLOWED),
        ]);
        let r = hub_report(
            "macos",
            Backend::ApplicationFirewall,
            Some(EXECUTABLE),
            &host,
        );
        assert!(r.ok, "{}", r.verdict);
        assert_eq!(r.executable.as_deref(), Some(EXECUTABLE));
        assert_eq!(r.observation.state, FirewallObservationState::Present);
        assert_eq!(r.commands.len(), 2);
        assert!(r.blocked_recently.is_none(), "there is no ufw log on a Mac");
        assert_eq!(
            host.asked().len(),
            2,
            "its state and its list, and no kernel log: {:?}",
            host.asked()
        );

        // without an executable it cannot be judged, and with no log either that is said
        let unasked = Script::new(&[]);
        let r = hub_report("macos", Backend::ApplicationFirewall, None, &unasked);
        assert!(r.ok);
        assert!(r.commands.is_empty());
        assert_eq!(
            r.verdict,
            "not observed: the application firewall admits an executable, and none was given; the kernel log could not be read either"
        );
        assert!(unasked.asked().is_empty(), "{:?}", unasked.asked());
    }

    // ------------------------------------------------------------ apply

    /// The report of the incident: ufw on, nothing of the mesh admitted.
    fn dropping() -> FirewallReport {
        let host = Script::new(&[("ufw status", UFW_BARE), ("journalctl", JOURNAL_QUIET)]);
        hub_report("linux", Backend::Ufw, None, &host)
    }

    #[test]
    fn apply_refuses_without_a_backend_without_commands_and_without_root_and_runs_nothing() {
        let host = Script::new(&[("ufw", "Rule added\n")]);
        let apply_as = |before: FirewallReport, root: bool| {
            apply_with(before, root, &|program, args| host.run(program, args))
        };

        let nowhere = hub_report("freebsd", Backend::None, None, &Script::new(&[]));
        let a = apply_as(nowhere, true);
        assert!(!a.ok && a.applied.is_empty());
        assert_eq!(
            a.refused.as_deref(),
            Some("no firewall front this executable knows is on this host; nothing to apply")
        );

        let on = Host {
            os: "linux",
            backend: Backend::Ufw,
            executable: None,
            run: &|program, args| host.run(program, args),
        };
        let a = apply_as(report_with(None, &[HUB], None, on), true);
        assert!(!a.ok && a.applied.is_empty());
        assert_eq!(
            a.refused.as_deref(),
            Some("the plan needs no admission on this host; nothing to apply")
        );

        let a = apply_as(dropping(), false);
        assert!(!a.ok && a.applied.is_empty());
        let refusal = a.refused.as_deref().unwrap();
        assert!(
            refusal.starts_with("applying firewall rules needs root"),
            "{refusal}"
        );
        // a refusal changed nothing, so the firewall's word is the one it had
        assert_eq!(a.observation, a.before.observation);
        assert_eq!(a.observation.state, FirewallObservationState::Missing);

        assert!(host.asked().is_empty(), "{:?}", host.asked());
    }

    #[test]
    fn as_root_every_command_runs_as_rendered_and_the_verdict_is_the_firewalls_afterwards() {
        let before = dropping();
        let rendered: Vec<String> = before.commands.iter().map(|c| c.argv.join(" ")).collect();
        let lines: Vec<String> = before.commands.iter().map(|c| c.line.clone()).collect();
        let host = Script::new(&[("ufw allow", "Rule added\n"), ("ufw status", UFW_ADMITTED)]);
        let a = apply_with(before, true, &|program, args| host.run(program, args));
        assert!(a.ok);
        assert_eq!(a.refused, None);
        assert_eq!(a.applied.len(), 3);
        assert!(a.applied.iter().all(|c| c.ok && c.output == "Rule added"));
        let ran: Vec<&str> = a.applied.iter().map(|c| c.line.as_str()).collect();
        assert_eq!(ran, lines);
        assert_eq!(a.observation.state, FirewallObservationState::Present);
        assert_eq!(
            a.before.observation.state,
            FirewallObservationState::Missing,
            "the report the commands came from is kept as it was"
        );
        // exactly the rendered vectors, in order, and then the question again
        let mut expected = rendered;
        expected.push("ufw status".to_string());
        assert_eq!(host.asked(), expected);
    }

    #[test]
    fn a_command_the_tool_refuses_fails_the_run_even_when_the_firewall_then_admits_the_plan() {
        // The group's command is refused by the tool; the hub's cannot run at all.
        let host = Script::new(&[
            ("proto udp", "ERROR: Invalid syntax\n"),
            ("ufw status", UFW_ADMITTED),
        ]);
        let a = apply_with(dropping(), true, &|program, args| host.run(program, args));
        assert!(!a.ok, "a refused command is never a run that held");
        assert_eq!(a.refused, None, "it ran: that is not a refusal to run");
        assert_eq!(a.observation.state, FirewallObservationState::Present);
        assert!(!a.applied[0].ok);
        assert_eq!(a.applied[0].output, "ERROR: Invalid syntax");
        for failed in &a.applied[1..] {
            assert!(!failed.ok);
            assert_eq!(
                failed.output,
                "ufw could not run: no such tool on this host"
            );
        }
    }

    #[test]
    fn commands_that_all_ran_do_not_hold_while_the_firewall_still_drops_the_mesh() {
        let still = Script::new(&[("ufw allow", "Rule added\n"), ("ufw status", UFW_BARE)]);
        let a = apply_with(dropping(), true, &|program, args| still.run(program, args));
        assert!(a.applied.iter().all(|c| c.ok));
        assert_eq!(a.observation.state, FirewallObservationState::Missing);
        assert!(!a.ok, "the verdict is the firewall's, not the exit codes'");

        // a firewall switched off meanwhile filters nothing, and that holds
        let off = Script::new(&[
            ("ufw allow", "Rules updated\n"),
            ("ufw status", "Status: inactive\n"),
        ]);
        let a = apply_with(dropping(), true, &|program, args| off.run(program, args));
        assert_eq!(a.observation.state, FirewallObservationState::Inactive);
        assert!(a.ok);

        // and one that cannot be asked afterwards has not been seen to admit anything
        let blind = Script::new(&[("ufw allow", "Rule added\n"), ("ufw status", UFW_ROOT)]);
        let a = apply_with(dropping(), true, &|program, args| blind.run(program, args));
        assert_eq!(a.observation.state, FirewallObservationState::Unobservable);
        assert!(!a.ok);
    }

    #[test]
    fn the_application_firewall_is_applied_for_the_executable_the_report_names() {
        let listed_other = Script::new(&[
            ("--getglobalstate", FIREWALL_ON),
            ("--listapps", APPS_OTHER),
        ]);
        let before = hub_report(
            "macos",
            Backend::ApplicationFirewall,
            Some(EXECUTABLE),
            &listed_other,
        );
        assert!(!before.ok);
        assert_eq!(
            before.observation.missing,
            ["incoming connections to /opt/majordomus"]
        );

        let host = Script::new(&[
            (
                "--add",
                "Application at path ( /opt/majordomus ) added to firewall\n",
            ),
            (
                "--unblockapp",
                "Incoming connection to the application is permitted\n",
            ),
            ("--getglobalstate", FIREWALL_ON),
            ("--listapps", APPS_ALLOWED),
        ]);
        let a = apply_with(before, true, &|program, args| host.run(program, args));
        assert!(a.ok, "{a:?}");
        assert_eq!(a.applied.len(), 2);
        assert_eq!(a.observation.state, FirewallObservationState::Present);
        assert_eq!(
            host.asked(),
            [
                format!("{SOCKETFILTERFW} --add {EXECUTABLE}"),
                format!("{SOCKETFILTERFW} --unblockapp {EXECUTABLE}"),
                format!("{SOCKETFILTERFW} --getglobalstate"),
                format!("{SOCKETFILTERFW} --listapps"),
            ]
        );
    }

    #[test]
    fn applying_an_empty_plan_on_this_host_is_refused_before_anything_could_run() {
        // The real `apply` over the real, empty report: whatever firewall this machine
        // runs and whoever runs the suite — root included — there is no command to run.
        let a = apply(report(None, &[], None));
        assert!(!a.ok && a.applied.is_empty());
        let refusal = a.refused.as_deref().unwrap();
        assert!(refusal.ends_with("nothing to apply"), "{refusal}");
        assert_eq!(a.observation, a.before.observation);
        assert_eq!(a.before.platform, std::env::consts::OS);
        assert_eq!(a.before.backend, detect_here());
    }
}
