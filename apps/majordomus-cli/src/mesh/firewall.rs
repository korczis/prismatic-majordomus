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
//! anything on a public address, because the declaration case refuses one first.
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
    let mut networks: Vec<String> = basis.iter().filter_map(|ip| enclosing(*ip)).collect();
    networks.sort();
    networks.dedup();

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

    let mut hub_ports: Vec<u16> = config
        .rendezvous
        .endpoints
        .iter()
        .filter_map(|e| parse_endpoint(e))
        .filter(|(ip, _)| local.contains(ip))
        .map(|(_, port)| port)
        .collect();
    hub_ports.sort_unstable();
    hub_ports.dedup();
    for port in &hub_ports {
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
    if let Some(port) = server_port {
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
        hub_ports,
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
    if plan.rules.is_empty() || std::env::consts::OS != "linux" {
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
    let plan = plan(config, local, server_port);
    let backend = detect_here();
    let executable = std::env::current_exe().ok();
    let commands = render(backend, &plan, executable.as_deref());
    let observation = if plan.rules.is_empty() {
        FirewallObservation::of(
            FirewallObservationState::Present,
            "the mesh needs no inbound admission on this host: no enabled declaration, or nothing declared that reaches this machine",
        )
    } else {
        observe(backend, &plan, executable.as_deref())
    };
    let blocked_recently = kernel_blocks(&plan);
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
        platform: std::env::consts::OS.to_string(),
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
    if !is_root() {
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
    let observation = observe(before.backend, &before.plan, exe);
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
        let p = plan(Some(&c), &[Ipv4Addr::new(203, 0, 113, 5)], None);
        assert!(p.networks.is_empty());
        let hub = p.rules.iter().find(|r| r.role == Role::Hub).unwrap();
        assert!(
            hub.sources.is_empty(),
            "no private network to admit from: {hub:?}"
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
}
