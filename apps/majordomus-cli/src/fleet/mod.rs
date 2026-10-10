//! The fleet: the machines that run this repository's mesh, and the one operation that
//! brings every one of them to the repository's latest stable release (ADR 0121, docs/FLEET.md).
//!
//! The declaration (`.ai/repo/fleet/<id>.yaml`, kind `fleet-declaration`) names each machine's mesh
//! node, the ssh destinations that reach it and the hub it serves. From it:
//!
//! - [`plan`] says what a rollout would do, reaching nothing;
//! - [`status`] asks every machine what it runs;
//! - [`rollout`] installs the release on every machine with the published installer,
//!   fast-forwards each hub's checkout, writes and restarts each hub's service, waits for
//!   the hub to answer at the version, and asks the hubs whether the mesh sees every hub.
//!
//! Machines are visited in parallel and reported in the declaration's order. A machine
//! that cannot be reached, or whose step is declined, stops there and is reported; the
//! others go on. The machine whose mesh node is this one's own is reached without ssh.
//!
//! Every program that runs on a machine is fixed in [`scripts`] and handed the
//! declaration's values as arguments; ssh runs with `BatchMode=yes`, so a destination that
//! would ask for a password is unreachable rather than a prompt.

pub mod scripts;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::process::Command;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Object;

/// The kind a fleet declaration is.
pub const KIND: &str = "fleet-declaration";
/// The format version this executable reads.
pub const SCHEMA_VERSION: &str = "fleet/v1";
/// The port a hub listens on when its declaration names none.
pub const DEFAULT_HUB_PORT: u16 = 8791;
/// The name a hub's service carries, as a systemd unit and as a launchd label.
pub const SYSTEMD_UNIT: &str = "majordomus-hub.service";
/// The launchd label of a hub's agent.
pub const LAUNCHD_LABEL: &str = "dev.majordomus.hub";

const REACH_BUDGET: Duration = Duration::from_secs(15);
const PROBE_BUDGET: Duration = Duration::from_secs(30);
const INSTALL_BUDGET: Duration = Duration::from_secs(300);
const CHECKOUT_BUDGET: Duration = Duration::from_secs(300);
const SERVICE_BUDGET: Duration = Duration::from_secs(60);
/// How long a restarted hub has to answer at the version.
const VERIFY_SECONDS: u64 = 90;
/// How long the hubs have, after the last one is verified, to see every hub.
const MESH_SECONDS: u64 = 75;

// ---------------------------------------------------------------- declaration

/// A fleet declaration as committed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// `fleet/v1`.
    pub schema: String,
    /// `fleet`.
    pub kind: String,
    /// The declaration's identity.
    pub id: String,
    /// Where a hub's checkout is cloned from when the machine has none.
    #[serde(default)]
    pub repository: Option<String>,
    /// Every machine, in the order a rollout reports them.
    pub machines: Vec<MachineDeclaration>,
}

/// One machine of a fleet declaration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineDeclaration {
    /// Its name in the fleet.
    pub id: String,
    /// How a person names it.
    #[serde(default)]
    pub title: Option<String>,
    /// Its mesh node id.
    pub node: String,
    /// The ssh destinations that reach it, in the order they are tried.
    pub ssh: Vec<String>,
    /// The hub it serves.
    #[serde(default)]
    pub hub: Option<HubDeclaration>,
}

/// The hub a machine serves.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HubDeclaration {
    /// Its checkout, relative to the machine's home directory.
    pub checkout: String,
    /// Its port.
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    DEFAULT_HUB_PORT
}

impl Declaration {
    /// Read a declaration from its indexed object, and refuse one that says something this
    /// executable would have to guess at.
    pub fn parse(object: &Object) -> Result<Declaration, String> {
        let d: Declaration = serde_json::from_value(object.metadata.clone())
            .map_err(|e| format!("{}: {e}", object.uri))?;
        d.validate().map_err(|e| format!("{}: {e}", object.uri))?;
        Ok(d)
    }

    /// The invariants the schema cannot state.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA_VERSION {
            return Err(format!("schema {} is not {SCHEMA_VERSION}", self.schema));
        }
        if self.kind != KIND {
            return Err(format!("kind {} is not {KIND}", self.kind));
        }
        if self.machines.is_empty() {
            return Err("a fleet names at least one machine".into());
        }
        let mut ids = BTreeSet::new();
        let mut nodes = BTreeSet::new();
        for m in &self.machines {
            if !ids.insert(m.id.as_str()) {
                return Err(format!("machine {} is declared twice", m.id));
            }
            if m.node.len() != 32 || !m.node.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!(
                    "machine {}: node {} is not 32 hex characters",
                    m.id, m.node
                ));
            }
            if !nodes.insert(m.node.as_str()) {
                return Err(format!("node {} is declared for two machines", m.node));
            }
            if m.ssh.is_empty() {
                return Err(format!("machine {} names no ssh destination", m.id));
            }
            for dest in &m.ssh {
                if dest.starts_with('-') || dest.chars().any(|c| c.is_whitespace()) {
                    return Err(format!(
                        "machine {}: {dest} is not an ssh destination",
                        m.id
                    ));
                }
                if let Some(ip) = address_of(dest) {
                    if !is_private(ip) {
                        return Err(format!(
                            "machine {}: {dest} is a public address; a fleet is reached over the private network or the tailnet",
                            m.id
                        ));
                    }
                }
            }
            if let Some(hub) = &m.hub {
                if hub.checkout.is_empty()
                    || hub.checkout.starts_with('/')
                    || hub.checkout.split('/').any(|p| p == ".." || p.is_empty())
                {
                    return Err(format!(
                        "machine {}: hub checkout {} is not a path inside the home directory",
                        m.id, hub.checkout
                    ));
                }
                if hub.port < 1024 {
                    return Err(format!(
                        "machine {}: hub port {} is privileged",
                        m.id, hub.port
                    ));
                }
                if self.repository.is_none() {
                    return Err(format!(
                        "machine {} serves a hub, and the fleet names no repository to clone its checkout from",
                        m.id
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The address an ssh destination names, when it names one rather than a host alias.
fn address_of(dest: &str) -> Option<IpAddr> {
    let host = dest.rsplit('@').next().unwrap_or(dest);
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

/// Is this an address of a private network, a tailnet (100.64.0.0/10) or this machine?
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

// ---------------------------------------------------------------- what the operation needs

/// What a rollout needs besides the declaration: the release and how it is installed.
#[derive(Debug, Clone)]
pub struct Release {
    /// The version every machine is brought to.
    pub version: String,
    /// Whether the version was asked for, rather than taken from this executable: only then
    /// does a machine with a newer one installed get this one.
    pub pinned: bool,
    /// The installer's URL.
    pub installer: String,
    /// Where the installer puts the launchers, as the distribution model writes it
    /// (`$HOME/.local/bin`).
    pub install_dir: String,
    /// Where it puts the versioned trees (`$HOME/.local/share/majordomus`).
    pub prefix: String,
    /// Whether servers running an older installed tree are left running.
    pub keep_servers: bool,
}

impl Release {
    /// The install directory relative to a home directory, as the programs take it.
    fn install_dir_in_home(&self) -> String {
        in_home(&self.install_dir)
    }

    /// The prefix relative to a home directory.
    fn prefix_in_home(&self) -> String {
        in_home(&self.prefix)
    }
}

fn in_home(path: &str) -> String {
    path.trim_start_matches("$HOME")
        .trim_start_matches("${HOME}")
        .trim_start_matches('/')
        .to_string()
}

/// The node this machine is in the mesh, read from its identity; `None` when it has none
/// and cannot make one, in which case every machine is reached over ssh.
pub fn local_node() -> Option<String> {
    let path = crate::mesh::identity::default_identity_path()?;
    crate::mesh::identity::NodeIdentity::load_or_create(&path)
        .ok()
        .map(|i| i.public.node_id.as_str().to_string())
}

// ---------------------------------------------------------------- views

/// The hub a machine serves, as every answer shows it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HubView {
    /// Its checkout, relative to the machine's home directory.
    pub checkout: String,
    /// Its port.
    pub port: u16,
}

/// What a rollout would do on one machine.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MachinePlan {
    /// The machine's name in the fleet.
    pub id: String,
    /// How a person names it.
    pub title: Option<String>,
    /// Its mesh node id.
    pub node: String,
    /// Whether it is this machine, reached without ssh.
    pub local: bool,
    /// The ssh destinations, in the order they are tried.
    pub destinations: Vec<String>,
    /// The hub it serves.
    pub hub: Option<HubView>,
    /// The steps, in order.
    pub steps: Vec<Step>,
}

/// What `fleet plan` answers.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Plan {
    /// The declaration read.
    pub declaration: String,
    /// The version every machine would be brought to.
    pub version: String,
    /// The installer that would install it.
    pub installer: String,
    /// This machine's mesh node, when it has one.
    pub local_node: Option<String>,
    /// Every machine, in the declaration's order.
    pub machines: Vec<MachinePlan>,
}

/// One step of a rollout on one machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Find a destination that answers.
    Reach,
    /// Ask what the machine is and runs.
    Probe,
    /// Install the release with the published installer.
    Install,
    /// Fast-forward the hub's checkout.
    Checkout,
    /// Write and (re)start the hub's service.
    Service,
    /// Wait for the hub, or the launcher, to answer at the version.
    Verify,
    /// Restart every server still running an installed tree of another version.
    Servers,
}

/// How a step went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// It held without changing anything.
    Ok,
    /// It changed the machine.
    Changed,
    /// It had nothing to do.
    Skipped,
    /// It was declined, and the detail says why; the machine was left as the step found it.
    Refused,
    /// It failed.
    Failed,
}

/// One step's outcome.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StepOutcome {
    /// Which step.
    pub step: Step,
    /// How it went.
    pub status: StepStatus,
    /// What it found or did.
    pub detail: String,
}

/// Where one machine ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MachineVerdict {
    /// It runs the version, and its hub, if it serves one, answers at it.
    Converged,
    /// No destination answered.
    Unreachable,
    /// A step was declined or failed; the steps say which.
    Failed,
}

/// One machine's rollout.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MachineOutcome {
    /// The machine's name in the fleet.
    pub id: String,
    /// Its mesh node id.
    pub node: String,
    /// Whether it is this machine.
    pub local: bool,
    /// The destination that answered (`local` for this machine).
    pub reached_by: Option<String>,
    /// `uname -s` and `-m`.
    pub platform: Option<String>,
    /// The version installed before.
    pub before: Option<String>,
    /// The version installed after.
    pub after: Option<String>,
    /// Every step taken, in order.
    pub steps: Vec<StepOutcome>,
    /// Where it ended.
    pub verdict: MachineVerdict,
}

impl MachineOutcome {
    fn record(&mut self, step: Step, status: StepStatus, detail: String) {
        self.steps.push(StepOutcome {
            step,
            status,
            detail,
        });
    }
}

/// One hub's machine, as the mesh sees it after the rollout.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Sighting {
    /// The machine's name in the fleet.
    pub id: String,
    /// Its mesh node id.
    pub node: String,
    /// The machines whose hubs see this node present.
    pub seen_by: Vec<String>,
    /// The versions its runtimes advertise, as those hubs see them.
    pub versions: Vec<String>,
}

/// Where the fleet ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FleetVerdict {
    /// Every machine converged, and every hub is seen by every other converged hub.
    Converged,
    /// Some did; the machines say which did not.
    Partial,
    /// None did.
    Failed,
}

/// What `fleet rollout` answers.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Rollout {
    /// The declaration read.
    pub declaration: String,
    /// The version the machines were brought to.
    pub version: String,
    /// Every machine asked, in the declaration's order.
    pub machines: Vec<MachineOutcome>,
    /// Every converged hub, as the converged hubs' meshes see it.
    pub mesh: Vec<Sighting>,
    /// Where the fleet ended.
    pub verdict: FleetVerdict,
}

/// What one machine's hub is.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HubStatus {
    /// Its checkout, relative to the home directory.
    pub checkout: String,
    /// Its port.
    pub port: u16,
    /// Whether the checkout exists.
    pub present: bool,
    /// The checkout's branch.
    pub branch: Option<String>,
    /// Its commit.
    pub head: Option<String>,
    /// Whether it has uncommitted changes to tracked files.
    pub dirty: Option<bool>,
    /// The version the hub answers at on the port, when it answers.
    pub answering: Option<String>,
}

/// What one machine runs.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MachineStatus {
    /// The machine's name in the fleet.
    pub id: String,
    /// Its mesh node id.
    pub node: String,
    /// Whether it is this machine.
    pub local: bool,
    /// The destination that answered.
    pub reached_by: Option<String>,
    /// `uname -s` and `-m`.
    pub platform: Option<String>,
    /// The version the launcher runs.
    pub installed: Option<String>,
    /// Its hub.
    pub hub: Option<HubStatus>,
    /// Every `majordomus serve` running there, as its command line.
    pub servers: Vec<String>,
    /// Why it could not be asked.
    pub error: Option<String>,
}

/// What `fleet status` answers.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Status {
    /// The declaration read.
    pub declaration: String,
    /// The version this executable is.
    pub version: String,
    /// Every machine, in the declaration's order.
    pub machines: Vec<MachineStatus>,
}

// ---------------------------------------------------------------- running programs

/// What a program answered.
#[derive(Debug, Clone, Default)]
pub struct Ran {
    /// Its exit code; `None` when it could not be started or ran out of time.
    pub code: Option<i32>,
    /// Its `key=value` lines, the last of each key kept.
    pub values: BTreeMap<String, String>,
    /// Every `server=` line, in order.
    pub servers: Vec<String>,
    /// Every `log=` line, in order.
    pub log: Vec<String>,
    /// What it wrote on stderr, trimmed.
    pub stderr: String,
}

impl Ran {
    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// Why it did not succeed, in one line.
    fn why(&self) -> String {
        if let Some(r) = self.get("refused") {
            return r.to_string();
        }
        let mut parts: Vec<String> = self.log.clone();
        if !self.stderr.is_empty() {
            parts.push(self.stderr.lines().last().unwrap_or_default().to_string());
        }
        match (self.code, parts.is_empty()) {
            (None, _) => "did not finish in time, or could not be started".into(),
            (Some(c), true) => format!("exited {c}"),
            (Some(c), false) => format!("exited {c}: {}", parts.join(" | ")),
        }
    }
}

/// How a machine is reached.
#[derive(Debug, Clone)]
pub enum Transport {
    /// This machine: the program runs here.
    Local,
    /// Another one, through this ssh destination.
    Ssh(String),
}

impl Transport {
    fn label(&self) -> String {
        match self {
            Transport::Local => "local".into(),
            Transport::Ssh(d) => d.clone(),
        }
    }

    /// Run one of [`scripts`] with its arguments, and give up after `budget`.
    pub fn run(&self, program: &str, args: &[&str], budget: Duration) -> Ran {
        let mut cmd = match self {
            Transport::Local => {
                let mut c = Command::new("sh");
                c.arg("-c").arg(program).arg("majordomus-fleet").args(args);
                c
            }
            Transport::Ssh(dest) => {
                // ssh joins its arguments into one line the remote login shell parses, so
                // the line is composed here, every word quoted
                let mut line = format!("sh -c {} majordomus-fleet", quote(program));
                for a in args {
                    line.push(' ');
                    line.push_str(&quote(a));
                }
                let mut c = Command::new("ssh");
                c.args([
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "ConnectTimeout=6",
                    "-o",
                    "ServerAliveInterval=15",
                    "--",
                    dest,
                    &line,
                ]);
                c
            }
        };
        match crate::environment::probe::bounded_output(&mut cmd, budget) {
            None => Ran::default(),
            Some(out) => {
                let mut ran = Ran {
                    code: out.status.code(),
                    stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
                    ..Ran::default()
                };
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let Some((k, v)) = line.split_once('=') else {
                        continue;
                    };
                    match k {
                        "server" => ran.servers.push(v.to_string()),
                        "log" => ran.log.push(v.to_string()),
                        _ => {
                            ran.values.insert(k.to_string(), v.to_string());
                        }
                    }
                }
                ran
            }
        }
    }
}

/// A word quoted for a POSIX shell.
pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// The first destination that answers, or why none did.
fn reach(machine: &MachineDeclaration, local: bool) -> Result<Transport, String> {
    if local {
        return Ok(Transport::Local);
    }
    let mut why = Vec::new();
    for dest in &machine.ssh {
        let t = Transport::Ssh(dest.clone());
        let ran = t.run("echo reached=yes", &[], REACH_BUDGET);
        if ran.code == Some(0) && ran.get("reached") == Some("yes") {
            return Ok(t);
        }
        let reason = if ran.stderr.is_empty() {
            ran.why()
        } else {
            ran.stderr.lines().last().unwrap_or_default().to_string()
        };
        why.push(format!("{dest}: {reason}"));
    }
    Err(why.join("; "))
}

// ---------------------------------------------------------------- plan

/// What a rollout would do, reaching nothing.
pub fn plan(declaration: &Declaration, uri: &str, release: &Release) -> Plan {
    let here = local_node();
    Plan {
        declaration: uri.to_string(),
        version: release.version.clone(),
        installer: release.installer.clone(),
        machines: declaration
            .machines
            .iter()
            .map(|m| {
                let mut steps = vec![Step::Reach, Step::Probe, Step::Install];
                if m.hub.is_some() {
                    steps.extend([Step::Checkout, Step::Service]);
                }
                steps.push(Step::Verify);
                if !release.keep_servers {
                    steps.push(Step::Servers);
                }
                MachinePlan {
                    id: m.id.clone(),
                    title: m.title.clone(),
                    node: m.node.clone(),
                    local: here.as_deref() == Some(m.node.as_str()),
                    destinations: m.ssh.clone(),
                    hub: m.hub.as_ref().map(|h| HubView {
                        checkout: h.checkout.clone(),
                        port: h.port,
                    }),
                    steps,
                }
            })
            .collect(),
        local_node: here,
    }
}

// ---------------------------------------------------------------- status

/// Ask every machine what it runs.
pub fn status(declaration: &Declaration, uri: &str, release: &Release) -> Status {
    let here = local_node();
    let machines = in_parallel(&declaration.machines, |m| {
        status_of(m, here.as_deref() == Some(m.node.as_str()), release)
    });
    Status {
        declaration: uri.to_string(),
        version: crate::VERSION.to_string(),
        machines,
    }
}

fn status_of(m: &MachineDeclaration, local: bool, release: &Release) -> MachineStatus {
    let mut s = MachineStatus {
        id: m.id.clone(),
        node: m.node.clone(),
        local,
        reached_by: None,
        platform: None,
        installed: None,
        hub: None,
        servers: Vec::new(),
        error: None,
    };
    let t = match reach(m, local) {
        Ok(t) => t,
        Err(why) => {
            s.error = Some(why);
            return s;
        }
    };
    s.reached_by = Some(t.label());
    let p = probe(&t, m, release);
    if p.code != Some(0) {
        s.error = Some(p.why());
        return s;
    }
    s.platform = platform(&p);
    s.installed = p.get("installed").map(str::to_string);
    s.servers = p.servers.clone();
    s.hub = m.hub.as_ref().map(|h| HubStatus {
        checkout: h.checkout.clone(),
        port: h.port,
        present: p.get("checkout") == Some("present"),
        branch: p.get("branch").map(str::to_string),
        head: p.get("head").map(str::to_string),
        dirty: p.get("dirty").map(|d| d == "yes"),
        answering: p.get("ready").and_then(ready_version),
    });
    s
}

fn probe(t: &Transport, m: &MachineDeclaration, release: &Release) -> Ran {
    let (checkout, port) = m
        .hub
        .as_ref()
        .map(|h| (h.checkout.clone(), h.port.to_string()))
        .unwrap_or_default();
    t.run(
        scripts::PROBE,
        &[&checkout, &port, &release.install_dir_in_home()],
        PROBE_BUDGET,
    )
}

fn platform(p: &Ran) -> Option<String> {
    match (p.get("os"), p.get("arch")) {
        (Some(os), Some(arch)) => Some(format!("{os} {arch}")),
        _ => None,
    }
}

/// The version a `/api/v1/ready` answer names.
fn ready_version(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()?
        .get("version")?
        .as_str()
        .map(str::to_string)
}

// ---------------------------------------------------------------- rollout

/// Bring every machine (or the ones named) to the release, and say where each ended.
pub fn rollout(
    declaration: &Declaration,
    uri: &str,
    release: &Release,
    only: &[String],
) -> Result<Rollout, String> {
    for id in only {
        if !declaration.machines.iter().any(|m| &m.id == id) {
            return Err(format!("the fleet declares no machine {id}"));
        }
    }
    let chosen: Vec<&MachineDeclaration> = declaration
        .machines
        .iter()
        .filter(|m| only.is_empty() || only.contains(&m.id))
        .collect();
    let here = local_node();
    let results = in_parallel(&chosen, |m| {
        roll(
            m,
            here.as_deref() == Some(m.node.as_str()),
            declaration,
            release,
        )
    });
    let machines: Vec<MachineOutcome> = results.iter().map(|(o, _)| o.clone()).collect();
    let mesh = sightings(&chosen, &results);
    let converged = machines
        .iter()
        .filter(|m| m.verdict == MachineVerdict::Converged)
        .count();
    let hubs_see_each_other = mesh.iter().all(|s| {
        // every converged hub other than this one sees it
        let others = results
            .iter()
            .filter(|(o, t)| t.is_some() && o.id != s.id)
            .count();
        s.seen_by.iter().filter(|w| **w != s.id).count() >= others
    });
    let verdict = if converged == machines.len() && hubs_see_each_other {
        FleetVerdict::Converged
    } else if converged > 0 {
        FleetVerdict::Partial
    } else {
        FleetVerdict::Failed
    };
    Ok(Rollout {
        declaration: uri.to_string(),
        version: release.version.clone(),
        machines,
        mesh,
        verdict,
    })
}

/// One machine's rollout, and the transport its converged hub can be asked through.
fn roll(
    m: &MachineDeclaration,
    local: bool,
    declaration: &Declaration,
    release: &Release,
) -> (MachineOutcome, Option<Transport>) {
    let mut o = MachineOutcome {
        id: m.id.clone(),
        node: m.node.clone(),
        local,
        reached_by: None,
        platform: None,
        before: None,
        after: None,
        steps: Vec::new(),
        verdict: MachineVerdict::Failed,
    };
    // reach
    let t = match reach(m, local) {
        Ok(t) => t,
        Err(why) => {
            o.record(Step::Reach, StepStatus::Failed, why);
            o.verdict = MachineVerdict::Unreachable;
            return (o, None);
        }
    };
    o.reached_by = Some(t.label());
    o.record(Step::Reach, StepStatus::Ok, t.label());

    // probe
    let p = probe(&t, m, release);
    if p.code != Some(0) {
        o.record(Step::Probe, StepStatus::Failed, p.why());
        return (o, None);
    }
    o.platform = platform(&p);
    o.before = p.get("installed").map(str::to_string);
    let Some(home) = p.get("home").map(str::to_string) else {
        o.record(
            Step::Probe,
            StepStatus::Failed,
            "the machine did not say its home directory".into(),
        );
        return (o, None);
    };
    let os = p.get("os").unwrap_or_default().to_string();
    o.record(
        Step::Probe,
        StepStatus::Ok,
        format!(
            "{}; {} installed",
            o.platform.clone().unwrap_or_default(),
            o.before.clone().unwrap_or_else(|| "nothing".into())
        ),
    );

    // install
    let mut changed = false;
    match o.before.clone().as_deref() {
        Some(v) if v == release.version => {
            o.record(
                Step::Install,
                StepStatus::Skipped,
                format!("{v} is installed"),
            );
        }
        Some(v) if !release.pinned && newer(v, &release.version) => {
            o.record(
                Step::Install,
                StepStatus::Skipped,
                format!(
                    "{v} is newer than {}, and left as it is; name a version to install that one",
                    release.version
                ),
            );
        }
        _ => {
            let r = t.run(
                scripts::INSTALL,
                &[&release.installer, &release.version],
                INSTALL_BUDGET,
            );
            if r.code != Some(0) {
                o.record(Step::Install, StepStatus::Failed, r.why());
                return (o, None);
            }
            changed = true;
            o.record(
                Step::Install,
                StepStatus::Changed,
                format!(
                    "{} → {}",
                    o.before.clone().unwrap_or_else(|| "nothing".into()),
                    release.version
                ),
            );
        }
    }

    let launcher = format!("{home}/{}/majordomus", release.install_dir_in_home());
    let Some(hub) = &m.hub else {
        // a machine without a hub: the launcher is what converges
        let after = t.run(
            "v=$(\"$1\" --version 2>/dev/null | awk '{print $2}'); echo \"installed=$v\"",
            &[&launcher],
            PROBE_BUDGET,
        );
        o.after = after.get("installed").map(str::to_string);
        let held = o.after.as_deref() == Some(release.version.as_str())
            || (!release.pinned
                && o.after
                    .as_deref()
                    .is_some_and(|v| newer(v, &release.version)));
        if held {
            o.record(
                Step::Verify,
                StepStatus::Ok,
                format!("{launcher} runs {}", o.after.clone().unwrap_or_default()),
            );
            if servers(&mut o, &t, release, &launcher, "") {
                o.verdict = MachineVerdict::Converged;
            }
        } else {
            o.record(
                Step::Verify,
                StepStatus::Failed,
                format!(
                    "{launcher} runs {}, not {}",
                    o.after.clone().unwrap_or_else(|| "nothing".into()),
                    release.version
                ),
            );
        }
        return (o, None);
    };

    // checkout
    let repository = declaration.repository.clone().unwrap_or_default();
    let c = t.run(
        scripts::CHECKOUT,
        &[&hub.checkout, &repository],
        CHECKOUT_BUDGET,
    );
    match c.code {
        Some(0) => {
            let before = c.get("before").unwrap_or_default();
            let head = c.get("head").unwrap_or_default();
            if c.get("cloned") == Some("yes") || before != head {
                changed = true;
                o.record(
                    Step::Checkout,
                    StepStatus::Changed,
                    format!("{} → {}", short(before), short(head)),
                );
            } else {
                o.record(
                    Step::Checkout,
                    StepStatus::Ok,
                    format!("at {}", short(head)),
                );
            }
        }
        Some(10) => {
            o.record(Step::Checkout, StepStatus::Refused, c.why());
            return (o, None);
        }
        _ => {
            o.record(Step::Checkout, StepStatus::Failed, c.why());
            return (o, None);
        }
    }

    // service
    let checkout = format!("{home}/{}", hub.checkout);
    let (manager, file, name, content) = match os.as_str() {
        "Linux" => (
            "systemd",
            format!("{home}/.config/systemd/user/{SYSTEMD_UNIT}"),
            SYSTEMD_UNIT,
            systemd_unit(&launcher, &checkout, hub.port),
        ),
        "Darwin" => (
            "launchd",
            format!("{home}/Library/LaunchAgents/{LAUNCHD_LABEL}.plist"),
            LAUNCHD_LABEL,
            launchd_agent(&launcher, &checkout, hub.port, &home),
        ),
        other => {
            o.record(
                Step::Service,
                StepStatus::Refused,
                format!("no service manager this executable writes for {other}"),
            );
            return (o, None);
        }
    };
    let answering = p.get("ready").and_then(ready_version);
    let mode = if changed || answering.as_deref() != Some(release.version.as_str()) {
        "restart"
    } else {
        "keep"
    };
    let s = t.run(
        scripts::SERVICE,
        &[manager, &file, &content, name, &checkout, &launcher, mode],
        SERVICE_BUDGET,
    );
    if s.code != Some(0) {
        let status = if s.code == Some(10) {
            StepStatus::Refused
        } else {
            StepStatus::Failed
        };
        o.record(Step::Service, status, s.why());
        return (o, None);
    }
    let mut detail = format!("{manager} {name}");
    if let Some(r) = s.get("replaced") {
        detail.push_str(&format!(", replacing {r}"));
    }
    if s.get("linger") == Some("unavailable") {
        detail.push_str(
            "; lingering could not be enabled, so the hub stops when the user's last session ends",
        );
    }
    let status = if s.get("action") == Some("unchanged") {
        StepStatus::Ok
    } else {
        StepStatus::Changed
    };
    o.record(Step::Service, status, detail);

    // verify
    let v = t.run(
        scripts::VERIFY,
        &[
            &hub.port.to_string(),
            &release.version,
            &VERIFY_SECONDS.to_string(),
        ],
        Duration::from_secs(VERIFY_SECONDS + 30),
    );
    o.after = v.get("ready").and_then(ready_version);
    if v.code != Some(0) {
        o.record(Step::Verify, StepStatus::Failed, v.why());
        return (o, None);
    }
    let active = v
        .get("mesh")
        .and_then(|m| serde_json::from_str::<Value>(m).ok())
        .and_then(|m| m.get("active").and_then(Value::as_bool))
        .unwrap_or(false);
    if !active {
        o.record(
            Step::Verify,
            StepStatus::Failed,
            format!(
                "the hub answers at {} on port {} and its mesh is not active",
                release.version, hub.port
            ),
        );
        return (o, None);
    }
    o.record(
        Step::Verify,
        StepStatus::Ok,
        format!(
            "the hub answers at {} on port {}, its mesh active",
            release.version, hub.port
        ),
    );
    if !servers(&mut o, &t, release, &launcher, &checkout) {
        return (o, Some(t));
    }
    o.verdict = MachineVerdict::Converged;
    (o, Some(t))
}

/// Restart the servers still running another installed version, unless they are kept;
/// false when one did not come back.
fn servers(
    o: &mut MachineOutcome,
    t: &Transport,
    release: &Release,
    launcher: &str,
    hub_checkout: &str,
) -> bool {
    if release.keep_servers {
        return true;
    }
    let r = t.run(
        scripts::SERVERS,
        &[
            &release.prefix_in_home(),
            &release.version,
            launcher,
            hub_checkout,
        ],
        SERVICE_BUDGET * 2,
    );
    if r.code != Some(0) {
        o.record(Step::Servers, StepStatus::Failed, r.why());
        return false;
    }
    let failed = r.get("failed") == Some("yes");
    let (status, detail) = match (r.log.is_empty(), failed) {
        (true, _) => (
            StepStatus::Skipped,
            "no server runs another installed version".to_string(),
        ),
        (false, false) => (StepStatus::Changed, r.log.join("; ")),
        (false, true) => (StepStatus::Failed, r.log.join("; ")),
    };
    o.record(Step::Servers, status, detail);
    !failed
}

/// Ask every converged hub which nodes it sees, until each converged hub is seen by every
/// other or the time is up.
fn sightings(
    chosen: &[&MachineDeclaration],
    results: &[(MachineOutcome, Option<Transport>)],
) -> Vec<Sighting> {
    let hubs: Vec<(&MachineDeclaration, &Transport)> = chosen
        .iter()
        .zip(results)
        .filter_map(|(m, (_, t))| Some((*m, t.as_ref()?)))
        .collect();
    let deadline = std::time::Instant::now() + Duration::from_secs(MESH_SECONDS);
    loop {
        let seen: Vec<(String, Vec<(String, String)>)> = hubs
            .iter()
            .map(|(m, t)| {
                let port = m.hub.as_ref().map_or(DEFAULT_HUB_PORT, |h| h.port);
                let r = t.run(
                    "curl -fsS -m 5 \"http://127.0.0.1:$1/api/v1/mesh/nodes\" | tr -d '\\n' | sed 's/^/nodes=/'",
                    &[&port.to_string()],
                    PROBE_BUDGET,
                );
                (m.id.clone(), present_nodes(r.get("nodes").unwrap_or_default()))
            })
            .collect();
        let view: Vec<Sighting> = hubs
            .iter()
            .map(|(m, _)| {
                let mut seen_by = Vec::new();
                let mut versions = BTreeSet::new();
                for (by, nodes) in &seen {
                    let mine: Vec<&(String, String)> =
                        nodes.iter().filter(|(n, _)| *n == m.node).collect();
                    if !mine.is_empty() {
                        seen_by.push(by.clone());
                        versions.extend(mine.iter().map(|(_, v)| v.clone()));
                    }
                }
                Sighting {
                    id: m.id.clone(),
                    node: m.node.clone(),
                    seen_by,
                    versions: versions.into_iter().filter(|v| !v.is_empty()).collect(),
                }
            })
            .collect();
        let complete = view
            .iter()
            .all(|s| s.seen_by.iter().filter(|w| **w != s.id).count() + 1 >= hubs.len());
        if complete || std::time::Instant::now() >= deadline {
            return view;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

/// The present nodes a `/api/v1/mesh/nodes` answer lists, with the version each advertises.
fn present_nodes(body: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    v.get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| n.get("presence").and_then(Value::as_str) == Some("present"))
                .filter_map(|n| {
                    Some((
                        n.get("node_id")?.as_str()?.to_string(),
                        n.get("version")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------- services

/// A hub's systemd user unit.
pub fn systemd_unit(launcher: &str, checkout: &str, port: u16) -> String {
    format!(
        "# Written by `majordomus fleet rollout` (ADR 0121). Run that again rather than editing this file.
[Unit]
Description=Majordomus mesh hub — the repository AI layer over HTTP, MCP and the Cockpit, and a rendezvous for the fleet
Documentation=https://majordomus.dev/docs/
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory={checkout}
# Bound beyond loopback on purpose: the mesh declaration names this address as a rendezvous
# endpoint. --idle 0 runs until stopped; stdin is null, not a pipe.
ExecStart={launcher} serve --repo {checkout} --host 0.0.0.0 --port {port} --idle 0
StandardInput=null
# A server that finds the checkout already served exits 0 at once; this cadence is how often
# the unit asks again.
Restart=always
RestartSec=60
KillSignal=SIGTERM
TimeoutStopSec=20
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=read-write
ProtectKernelTunables=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes

[Install]
WantedBy=default.target"
    )
}

/// A hub's launchd agent.
pub fn launchd_agent(launcher: &str, checkout: &str, port: u16, home: &str) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let (launcher, checkout, home) = (esc(launcher), esc(checkout), esc(home));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Written by `majordomus fleet rollout` (ADR 0121). Run that again rather than editing this file. -->
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{launcher}</string>
    <string>serve</string>
    <string>--repo</string>
    <string>{checkout}</string>
    <string>--host</string>
    <string>0.0.0.0</string>
    <string>--port</string>
    <string>{port}</string>
    <string>--idle</string>
    <string>0</string>
  </array>
  <key>WorkingDirectory</key>
  <string>{checkout}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>StandardInPath</key>
  <string>/dev/null</string>
  <key>StandardOutPath</key>
  <string>{home}/Library/Logs/majordomus-hub.log</string>
  <key>StandardErrorPath</key>
  <string>{home}/Library/Logs/majordomus-hub.log</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>60</integer>
</dict>
</plist>"#
    )
}

// ---------------------------------------------------------------- helpers

/// Is `a` a newer version than `b`? Dotted numbers, compared field by field; anything that
/// is not a number compares as zero.
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a) > parts(b)
}

fn short(sha: &str) -> &str {
    &sha[..10.min(sha.len())]
}

/// Run `f` over every item on its own thread, and answer in the items' order.
fn in_parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = items.iter().map(|i| scope.spawn(|| f(i))).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a fleet worker panicked"))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(yaml: &str) -> Result<Declaration, String> {
        let d: Declaration = crate::metadata::yaml::parse_into(yaml)?;
        d.validate()?;
        Ok(d)
    }

    const ONE: &str = "schema: fleet/v1\nkind: fleet-declaration\nid: x\nrepository: https://example.invalid/r.git\nmachines:\n  - id: a\n    node: 0123456789abcdef0123456789abcdef\n    ssh: [me@192.168.1.2]\n    hub:\n      checkout: dev/hub\n";

    #[test]
    fn a_declaration_reads_and_a_hub_port_defaults() {
        let d = declaration(ONE).unwrap();
        assert_eq!(d.machines[0].hub.as_ref().unwrap().port, DEFAULT_HUB_PORT);
    }

    #[test]
    fn a_public_address_is_refused_and_a_tailnet_one_is_not() {
        let public = ONE.replace("me@192.168.1.2", "me@8.8.8.8");
        assert!(declaration(&public).unwrap_err().contains("public address"));
        let tailnet = ONE.replace("me@192.168.1.2", "me@100.65.22.118");
        assert!(declaration(&tailnet).is_ok());
        let alias = ONE.replace("me@192.168.1.2", "lundra");
        assert!(declaration(&alias).is_ok(), "a host alias names no address");
    }

    #[test]
    fn a_hub_checkout_outside_the_home_directory_is_refused() {
        for bad in ["/etc/hub", "../hub", "dev//hub"] {
            let d = ONE.replace("dev/hub", bad);
            assert!(declaration(&d).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn a_destination_that_would_be_an_option_is_refused() {
        let d = ONE.replace("me@192.168.1.2", "-oProxyCommand=x");
        assert!(declaration(&d).is_err());
    }

    #[test]
    fn a_hub_needs_a_repository_to_clone_from() {
        let d = ONE.replace("repository: https://example.invalid/r.git\n", "");
        assert!(declaration(&d).unwrap_err().contains("no repository"));
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.17.0", "0.9.9"));
        assert!(newer("v1.0.0", "0.17.0"));
        assert!(!newer("0.17.0", "0.17.0"));
        assert!(!newer("0.14.0", "0.17.0"));
    }

    #[test]
    fn a_word_is_quoted_for_the_remote_shell() {
        assert_eq!(quote("a'b"), r"'a'\''b'");
        assert_eq!(quote("$HOME"), "'$HOME'");
    }

    #[test]
    fn present_nodes_are_read_with_their_versions() {
        let body = r#"{"count":2,"nodes":[{"node_id":"a","presence":"present","version":"0.17.0"},{"node_id":"b","presence":"expired"}]}"#;
        assert_eq!(present_nodes(body), vec![("a".into(), "0.17.0".into())]);
        assert!(present_nodes("not json").is_empty());
    }

    #[test]
    fn a_service_runs_the_launcher_beyond_loopback() {
        let unit = systemd_unit("/h/.local/bin/majordomus", "/h/dev/hub", 8791);
        assert!(unit.contains(
            "ExecStart=/h/.local/bin/majordomus serve --repo /h/dev/hub --host 0.0.0.0 --port 8791 --idle 0"
        ));
        let plist = launchd_agent("/h/.local/bin/majordomus", "/h/dev/hub", 8791, "/h");
        assert!(plist.contains("<string>dev.majordomus.hub</string>"));
        assert!(plist.contains("<string>8791</string>"));
    }
}
