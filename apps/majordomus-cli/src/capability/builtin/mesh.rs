//! The `mesh` module: this process's mesh runtime, projected. Discovery (`mesh.status`,
//! `mesh.nodes`, `mesh.identity`, `mesh.doctor`, `mesh.register`) reads the one
//! [`crate::mesh::MeshRegistry`]; cooperation (`mesh.cooperation`, `mesh.peers`,
//! `mesh.peer`, `mesh.state`, `mesh.events`, `mesh.verify` and the commands) reads and
//! writes through the one [`crate::mesh::cooperation::Cooperation`] runtime. No capability
//! holds state of its own, so the CLI, HTTP, OpenAPI, MCP and the Cockpit cannot disagree
//! about a peer, a claim or a link (ADR 0050, ADR 0067).
//!
//! Discovery grants nothing. Cooperation is admitted per link — same repository, trusted
//! key, compatible protocol — and what it carries is metadata about work: sessions,
//! claims, handovers, reviews. There is no remote execution here for a linked peer to gain.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::cache_provision::{provision, ProvisionInput, ProvisionReport};
use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::mesh::address::interfaces;
use crate::mesh::cooperation::{
    Cooperation, CooperationError, CooperationStatus, MeshVerifyReport, PeerView, RefusedView,
    Written,
};
use crate::mesh::firewall::{FirewallApplyReport, FirewallReport};
use crate::mesh::journal::{
    ClaimMode, Cursor, CursorError, MeshEvent, SessionInfo, StreamId, StreamLiveness,
};
use crate::mesh::link::{LinkReply, RefusalCode, Signed, HELLO_PATH, SYNC_PATH};
use crate::mesh::page::{self, ClaimBrief, Query, Summary, View};
use crate::mesh::rendezvous::REGISTER_PATH;
use crate::mesh::state::{ClaimView, CooperationState, HandoverView, SessionState};
use crate::mesh::{
    default_identity_path, MeshConfig, MeshDoctorReport, MeshStatus, NodeIdentity, NodeRecord,
    PublicIdentity, RegisterAnswer,
};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The MCP resource `mesh.status` is projected as.
pub const MESH_URI: &str = "majordomus://mesh";

// ---------------------------------------------------------------- mesh.status

fn mesh_status(ctx: &Context, _: Empty) -> Result<MeshStatus, CapabilityError> {
    Ok(ctx.mesh.status())
}

// ---------------------------------------------------------------- mesh.nodes

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `mesh.nodes`: every node this process has observed.
pub struct NodeList {
    /// How many nodes the registry holds.
    pub count: usize,
    /// The nodes, in node-id order — the one canonical order every surface shows.
    pub nodes: Vec<NodeRecord>,
}

fn mesh_nodes(ctx: &Context, _: Empty) -> Result<NodeList, CapabilityError> {
    let nodes = ctx.mesh.nodes();
    Ok(NodeList {
        count: nodes.len(),
        nodes,
    })
}

// ---------------------------------------------------------------- mesh.identity

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `mesh.identity`: this machine's node identity, public half only.
pub struct MeshIdentityReport {
    /// Where the identity file lives (or would).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether the file exists.
    pub present: bool,
    /// The public identity, when the file exists and loads. The signing key is not
    /// here, not in any projection, and not in any log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<PublicIdentity>,
    /// Why the identity did not load, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn mesh_identity(_: &Context, _: Empty) -> Result<MeshIdentityReport, CapabilityError> {
    let Some(path) = default_identity_path() else {
        return Ok(MeshIdentityReport {
            path: None,
            present: false,
            identity: None,
            error: Some("no HOME and no XDG_STATE_HOME: nowhere to keep a node identity".into()),
        });
    };
    if !path.is_file() {
        return Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: false,
            identity: None,
            error: None,
        });
    }
    match NodeIdentity::load_or_create(&path) {
        Ok(identity) => Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: true,
            identity: Some(identity.public),
            error: None,
        }),
        Err(e) => Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: true,
            identity: None,
            error: Some(e.to_string()),
        }),
    }
}

// ---------------------------------------------------------------- mesh.enrol

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `mesh.enrol`: which declaration, which key, and whether it changed.
pub struct MeshEnrolment {
    /// The declaration file, repository-relative.
    pub path: String,
    /// This machine's public key: what is listed, never a secret.
    pub key: String,
    /// Whether the key was added now; `false` when it was listed already.
    pub added: bool,
    /// What to do next: commit the file, or nothing.
    pub next: String,
}

/// The name a machine's allowlist entry carries: its node id, its host name and its
/// platform, the shape a person writes by hand ("641bdb94 — a MacBook (macos aarch64)").
fn machine_label(identity: &PublicIdentity) -> String {
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "this machine".into());
    format!(
        "{} — {host} ({} {})",
        identity.display_name,
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn mesh_enrol(ctx: &Context, _: Empty) -> Result<MeshEnrolment, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    let object = ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == crate::mesh::KIND)
        .ok_or_else(|| {
            CapabilityError::NotFound(
                "this repository declares no mesh: `majordomus init --extend` seeds .ai/repo/mesh/mesh.yaml".into(),
            )
        })?;
    let relative = object.provenance.path.clone();
    let path = root.join(&relative);
    let identity_path = default_identity_path().ok_or_else(|| {
        CapabilityError::Refused(
            "no HOME and no XDG_STATE_HOME: nowhere to keep this machine's node identity".into(),
        )
    })?;
    let identity = NodeIdentity::load_or_create(&identity_path)
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;
    let key = identity.public.public_key.clone();
    // read, edit and write as one step: the index has read and parsed this declaration
    // already, so in practice only the write can fail, and every failure names the file
    let added = std::fs::read_to_string(&path)
        .map_err(io_reason)
        .and_then(|text| crate::mesh::enrol::enrol(&text, &key, &machine_label(&identity.public)))
        .and_then(|edited| write_enrolled(&path, edited))
        .map_err(|e| CapabilityError::Internal(format!("{relative}: {e}")))?;
    Ok(if added {
        MeshEnrolment {
            next: format!(
                "commit {relative}, so every machine that pulls it trusts this one; restart this checkout's server (`majordomus serve stop`, then `majordomus serve ensure`) to open the mesh here"
            ),
            path: relative,
            key,
            added: true,
        }
    } else {
        MeshEnrolment {
            path: relative,
            key,
            added: false,
            next: "nothing: this machine's key is listed already".into(),
        }
    })
}

/// Write an enrolled declaration over `path`: whether a key was added, `false` when the
/// declaration listed it already and nothing was written.
fn write_enrolled(path: &std::path::Path, edited: Option<String>) -> Result<bool, String> {
    match edited {
        None => Ok(false),
        Some(text) => std::fs::write(path, text).map(|()| true).map_err(io_reason),
    }
}

/// An I/O failure as the reason a capability gives.
fn io_reason(e: std::io::Error) -> String {
    e.to_string()
}

/// How a cursor the reader cannot use is refused: one of another run is stale, and one that
/// is not a cursor is invalid input.
fn cursor_refusal(e: CursorError) -> CapabilityError {
    match e {
        CursorError::Stale { .. } => CapabilityError::Refused(e.to_string()),
        CursorError::Malformed(_) => CapabilityError::InvalidInput(e.to_string()),
    }
}

// ---------------------------------------------------------------- mesh.doctor

fn mesh_doctor(ctx: &Context, _: Empty) -> Result<MeshDoctorReport, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    // The runtime's verdict is reported only where a shared server made one: in the
    // command line's process nothing activates the mesh, and that absence is not news.
    let runtime = ctx.mesh.decided().then(|| ctx.mesh.status());
    let resolved = resolved_declaration(ctx);
    let source = crate::mesh::doctor::declaration_source(&resolved);
    let mut report =
        crate::mesh::doctor::doctor_at(resolved.declaration, Some(root), runtime.as_ref());
    if let Some(check) = source {
        report.checks.push(check);
    }
    // the journal is judged only where a runtime holds one
    if let Some(cooperation) = ctx.mesh.cooperation() {
        report.checks.push(crate::mesh::doctor::journal_check(
            &cooperation.journal().tallies(),
        ));
        report.checks.push(crate::mesh::doctor::session_beats_check(
            &cooperation.session_beat_tally(),
        ));
    }
    // one conjunction over every check, so neither added check's failure can be lost
    report.ok = report.checks.iter().all(|c| c.ok);
    Ok(report)
}

// ---------------------------------------------------------------- mesh.firewall

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.firewall` and `mesh.firewall.apply`.
pub struct FirewallInput {
    /// The port this checkout's server listens on beyond loopback, when it does: a peer
    /// dials it there on the link protocol, so it needs admitting too. Omitted, the plan
    /// holds the discovery ports and the hub ports the declaration gives this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

impl BenchmarkCases for FirewallInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("declared", FirewallInput { port: None })]
    }
}

/// The report over this checkout's declaration and this machine's interfaces.
fn firewall_report(ctx: &Context, input: &FirewallInput) -> FirewallReport {
    let config = declaration(ctx).and_then(|d| d.ok());
    let local: Vec<Ipv4Addr> = interfaces().into_iter().map(|(_, ip)| ip).collect();
    crate::mesh::firewall::report(config.as_ref(), &local, input.port)
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.build_cache.key`.
pub struct BuildCacheKeyInput {
    /// Only this kind; every kind the repository has when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::mesh::build_cache::key::Kind>,
}

impl BenchmarkCases for BuildCacheKeyInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("every-kind", BuildCacheKeyInput { kind: None }),
            NamedCase::new(
                "cargo",
                BuildCacheKeyInput {
                    kind: Some(crate::mesh::build_cache::key::Kind::CargoBuildDir),
                },
            ),
        ]
    }
}

fn mesh_build_cache_key(
    ctx: &Context,
    input: BuildCacheKeyInput,
) -> Result<crate::mesh::build_cache::key::KeyReport, CapabilityError> {
    Ok(crate::mesh::build_cache::key::report(
        std::path::Path::new(&ctx.index.repository.root),
        input.kind,
    ))
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.who`.
pub struct WhoInput {
    /// A pid, `host:pid`, a run tag, `host:tag`, an issue (`I<digits>`) or a path.
    pub subject: String,
}

impl BenchmarkCases for WhoInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "issue",
            WhoInput {
                subject: "I1".into(),
            },
        )]
    }
}

fn mesh_who(
    ctx: &Context,
    input: WhoInput,
) -> Result<crate::mesh::who::WhoReport, CapabilityError> {
    let subject =
        crate::mesh::who::Subject::parse(&input.subject).map_err(CapabilityError::InvalidInput)?;
    Ok(crate::mesh::who::who(ctx, input.subject.trim(), &subject))
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.build_cache.ensure`.
pub struct BuildCacheEnsureInput {
    /// What is cached: `npm-modules`, `mix-deps` or `cargo-build-dir`.
    pub kind: crate::mesh::build_cache::key::Kind,
    /// The directory holding the lockfile, relative to the repository; absent names the only
    /// one the repository tracks, or the one the command was started in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// The install or build command, run in that directory on a miss; required for npm and
    /// mix, optional for a cargo build directory.
    #[serde(default)]
    pub command: Vec<String>,
}

impl BenchmarkCases for BuildCacheEnsureInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: it runs a command and writes outside the repository; timing it would
        // measure the install and the disk, not this executable
        Vec::new()
    }
}

fn mesh_build_cache_ensure(
    ctx: &Context,
    input: BuildCacheEnsureInput,
) -> Result<crate::mesh::build_cache::ensure::EnsureReport, CapabilityError> {
    use crate::mesh::build_cache::{ensure, key};
    if input.command.is_empty() && input.kind != key::Kind::CargoBuildDir {
        return Err(CapabilityError::InvalidInput(format!(
            "{} needs the install command after `--`",
            input.kind.as_str()
        )));
    }
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let report = key::report(&root, Some(input.kind));
    let cwd = std::env::current_dir().ok();
    let cwd_rel = cwd
        .as_deref()
        .and_then(|c| c.strip_prefix(&root).ok())
        .map(|r| r.to_string_lossy().into_owned());
    // an instance that cannot be named is not a reason to withhold the install: it runs
    // uncached, and says why (rule project.cache-is-invisible)
    let kk = ensure::resolve(
        &report,
        input.kind,
        input.dir.as_deref(),
        cwd_rel.as_deref(),
    )
    .unwrap_or_else(|why| {
        ensure::unresolved(input.kind, input.dir.as_deref().or(cwd_rel.as_deref()), why)
    });
    let declared = ctx
        .repository()
        .ok()
        .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
        .and_then(|p| p.policy.build_cache.local_max_gib);
    let machine = ensure::Machine::from_env(declared);
    Ok(ensure::ensure(&root, &kk, &input.command, &machine))
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.build_cache.env`.
pub struct BuildCacheEnvInput {
    /// The sccache executable to ask; `sccache` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sccache: Option<String>,
    /// The node file naming the store this node offers or knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_file: Option<String>,
    /// The AWS-format credential file, mode 0600.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<String>,
}

impl BenchmarkCases for BuildCacheEnvInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "no-sccache",
            BuildCacheEnvInput {
                sccache: Some("no-such-sccache".into()),
                ..BuildCacheEnvInput::default()
            },
        )]
    }
}

fn mesh_build_cache_env(
    _ctx: &Context,
    input: BuildCacheEnvInput,
) -> Result<crate::mesh::build_cache::env::EnvReport, CapabilityError> {
    Ok(crate::mesh::build_cache::env::report(
        &crate::mesh::build_cache::env::EnvOptions {
            sccache: input.sccache.unwrap_or_else(|| "sccache".to_string()),
            node_file: input.node_file.map(std::path::PathBuf::from),
            credentials: input.credentials.map(std::path::PathBuf::from),
        },
    ))
}

fn mesh_firewall(ctx: &Context, input: FirewallInput) -> Result<FirewallReport, CapabilityError> {
    Ok(firewall_report(ctx, &input))
}

fn mesh_firewall_apply(
    ctx: &Context,
    input: FirewallInput,
) -> Result<FirewallApplyReport, CapabilityError> {
    Ok(crate::mesh::firewall::apply(firewall_report(ctx, &input)))
}

/// Put the declaration's trust in force on a running cooperation runtime, ending the links of
/// every key it no longer trusts (I2134). The server calls this with each new generation of the
/// repository it reads, so a key withdrawn from `trust.allow` and committed loses its links
/// without a restart. Returns the runtime keys whose links ended.
pub(crate) fn retrust(ctx: &Context) -> Vec<String> {
    let (Some(cooperation), Some(Ok(config))) = (ctx.mesh.cooperation(), declaration(ctx)) else {
        return Vec::new();
    };
    cooperation.retrust(config.trust)
}

/// The repository's mesh declaration in force: `None` when no object of the kind exists,
/// the parse verdict when one does — the working tree's copy with every widening of the
/// trunk's copy removed ([`crate::mesh::root`]). Every reader of the declaration asks this,
/// so the runtime, the doctor and every status agree on what is obeyed.
pub fn declaration(ctx: &Context) -> Option<Result<MeshConfig, crate::mesh::MeshError>> {
    resolved_declaration(ctx).declaration
}

/// [`declaration`], with the trunk it was held against and what was narrowed.
pub(crate) fn resolved_declaration(ctx: &Context) -> crate::mesh::root::Resolved {
    let root = std::path::Path::new(&ctx.index.repository.root);
    let working = ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == crate::mesh::KIND);
    crate::mesh::root::resolve(root, working)
}

// ---------------------------------------------------------------- mesh.register

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.register`: one signed envelope, exactly as the wire carries it.
pub struct RegisterInput {
    /// The envelope. Verified here exactly as a datagram would be — bounds, staleness,
    /// signature — before anything is recorded.
    pub envelope: serde_json::Value,
}

impl BenchmarkCases for RegisterInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // A malformed envelope: the handler answers `accepted: false` without touching
        // the registry, which is exactly the cheap, side-effect-free path to measure.
        vec![NamedCase::new(
            "refused",
            RegisterInput {
                envelope: serde_json::json!({"v": 0}),
            },
        )]
    }
}

fn mesh_register(ctx: &Context, input: RegisterInput) -> Result<RegisterAnswer, CapabilityError> {
    Ok(ctx.mesh.register(&input.envelope, "http"))
}

// ---------------------------------------------------------------- cooperation: shared

fn cooperation(ctx: &Context) -> Result<std::sync::Arc<Cooperation>, CapabilityError> {
    ctx.mesh.cooperation().ok_or_else(|| {
        CapabilityError::Refused(format!(
            "not_active: {}",
            ctx.mesh
                .cooperation_status()
                .reason
                .unwrap_or_else(|| "cooperation is not running in this process".into())
        ))
    })
}

fn map_error(e: CooperationError) -> CapabilityError {
    match e {
        CooperationError::Conflict(_) => CapabilityError::Refused(format!("claim_conflict: {e}")),
        CooperationError::NotOwn(m) => CapabilityError::Refused(format!("not_own: {m}")),
        CooperationError::NotFound(m) => CapabilityError::NotFound(m),
        CooperationError::FeatureUnsupported(m) => {
            CapabilityError::Refused(format!("{}: {m}", RefusalCode::FeatureUnsupported.as_str()))
        }
        CooperationError::Invalid(m) => CapabilityError::InvalidInput(m),
    }
}

/// The session a call acts for: the one it names, else the calling MCP session's own board
/// session — the same `board-<peer>` session the board projection opened for it.
fn session_of(ctx: &Context, given: Option<String>) -> Result<String, CapabilityError> {
    if let Some(session) = given.filter(|s| !s.trim().is_empty()) {
        return Ok(session);
    }
    ctx.caller
        .as_ref()
        .map(|peer| format!("board-{}", peer.as_str()))
        .ok_or_else(|| {
            CapabilityError::InvalidInput(
                "name the session: over the command line or plain HTTP there is no calling session"
                    .into(),
            )
        })
}

// ---------------------------------------------------------------- mesh.cooperation

fn mesh_cooperation(ctx: &Context, _: Empty) -> Result<CooperationStatus, CapabilityError> {
    Ok(ctx.mesh.cooperation_status())
}

// ---------------------------------------------------------------- mesh.peers

/// One session in the peer tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionNode {
    /// `<stream>/<session>`.
    pub key: String,
    /// What the session last said about itself.
    pub info: SessionInfo,
    /// Its standing.
    pub state: SessionState,
    /// The claims it holds or held; empty in the summary view, which counts them instead.
    pub claims: Vec<ClaimView>,
    /// How many of its claims are live.
    pub claims_held: usize,
    /// The review requests it opened, by key.
    pub reviews: Vec<String>,
    /// Milliseconds since this runtime stored the session's newest session beat, on its own
    /// clock: how long ago its worker last showed progress (I3990). Absent for a session
    /// that never beat — it lives by its runtime's beat alone. Outside the digest, like
    /// every age.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_ms: Option<u64>,
    /// What its newest session beat said it was doing — `waiting on CI run 123` — when it
    /// said (I3990).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// One runtime in the peer tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeNode {
    /// `<node>-<runtime>`.
    pub runtime: String,
    /// Whether this is the runtime answering.
    pub this_runtime: bool,
    /// Whether its stream beats: own, live, or expired.
    pub liveness: StreamLiveness,
    /// Milliseconds since its beat last rose, on this runtime's clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat_ms: Option<u64>,
    /// This runtime's link to it, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<PeerView>,
    /// Its newest live claim, told briefly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_claim: Option<ClaimBrief>,
    /// Its sessions; in the summary view only the active ones.
    pub sessions: Vec<SessionNode>,
}

/// One machine in the peer tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MachineNode {
    /// The node id.
    pub node: String,
    /// Its display name, when a card or this node said.
    pub name: String,
    /// Whether it is this machine.
    pub local: bool,
    /// Its runtimes.
    pub runtimes: Vec<RuntimeNode>,
}

/// This machine first, then the others by the name a person reads; the node id ends the key,
/// because two machines can carry one name and the order must still be total.
impl crate::order::Ordered for MachineNode {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.name, &self.node).ranked(i64::from(!self.local))
    }
}

/// The answer of `mesh.peers`: machine → runtime → session → claim, every level from the
/// one journal and the one link table.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerTree {
    /// Whether cooperation runs.
    pub active: bool,
    /// What the answer carries: `summary` or `full`.
    pub view: View,
    /// Why not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// This runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The mesh repository id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// The machines, local first.
    pub machines: Vec<MachineNode>,
    /// Candidates refused, with the rule.
    pub refused: Vec<RefusedView>,
    /// The digest of the cooperation state: equal digests, equal state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

fn rank(l: StreamLiveness) -> u8 {
    match l {
        StreamLiveness::Own => 0,
        StreamLiveness::Live => 1,
        StreamLiveness::Expired => 2,
    }
}

/// Build the peer tree from a running cooperation, narrowed and viewed as `query` asks. The
/// summary view keeps every runtime and only the active sessions, each with its claims
/// counted rather than listed and its intent cut; the full view is every session with every
/// claim. Either way the digest is the whole fold's.
pub fn peer_tree(c: &Cooperation, query: &Query) -> PeerTree {
    let status = c.status();
    let whole = c.state();
    let state = page::narrow(&whole, query);
    let summary = query.view() == View::Summary;
    let links: BTreeMap<String, PeerView> = status
        .peers
        .iter()
        .map(|p| (p.runtime.clone(), p.clone()))
        .collect();
    let own = status.runtime.clone().unwrap_or_default();
    let own_node = own.get(..32).unwrap_or_default().to_string();

    // Runtimes: this one, every linked one, and every one the journal has heard of.
    let mut runtimes: BTreeMap<String, RuntimeNode> = BTreeMap::new();
    let mut touch = |key: &str, liveness: StreamLiveness, age: Option<u64>| {
        let node = runtimes
            .entry(key.to_string())
            .or_insert_with(|| RuntimeNode {
                runtime: key.to_string(),
                this_runtime: key == own,
                liveness,
                last_beat_ms: age,
                link: None,
                newest_claim: None,
                sessions: Vec::new(),
            });
        if rank(liveness) < rank(node.liveness) {
            node.liveness = liveness;
        }
        if let Some(a) = age {
            node.last_beat_ms = Some(node.last_beat_ms.map_or(a, |b| b.min(a)));
        }
    };
    for (stream, liveness, age) in c.streams() {
        touch(&stream.runtime_key(), liveness, age);
    }
    for key in links.keys() {
        touch(key, StreamLiveness::Expired, None);
    }
    for (key, node) in runtimes.iter_mut() {
        node.link = links.get(key).cloned();
    }
    for claim in state.claims.iter().filter(|cl| cl.state.is_live()) {
        // every claim's runtime is a node: each stream of the journal was touched above
        runtimes.entry(claim.runtime.clone()).and_modify(|runtime| {
            let newer = runtime
                .newest_claim
                .as_ref()
                .is_none_or(|b| claim.acquired_lamport > b.acquired_lamport);
            if newer {
                runtime.newest_claim = Some(ClaimBrief {
                    key: claim.key.clone(),
                    session: claim.session.clone(),
                    issue: claim.issue.clone(),
                    intent: claim.intent.as_deref().map(page::cut),
                    paths: claim.scope.len(),
                    mode: claim.mode,
                    acquired_lamport: claim.acquired_lamport,
                });
            }
        });
    }
    for session in &state.sessions {
        if summary && session.state != SessionState::Active {
            continue;
        }
        let claims: Vec<ClaimView> = state
            .claims
            .iter()
            .filter(|cl| cl.session == session.key)
            .cloned()
            .collect();
        let claims_held = claims.iter().filter(|cl| cl.state.is_live()).count();
        let mut info = session.info.clone();
        if summary {
            info.intent = info.intent.as_deref().map(page::cut);
        }
        let reviews = state
            .reviews
            .iter()
            .filter(|r| r.session == session.key)
            .map(|r| r.key.clone())
            .collect();
        let progress = c.journal().session_beat_age(&session.key);
        if let Some(runtime) = runtimes.get_mut(&session.runtime) {
            runtime.sessions.push(SessionNode {
                key: session.key.clone(),
                info,
                state: session.state,
                claims: if summary { Vec::new() } else { claims },
                claims_held,
                reviews,
                progress_ms: progress.as_ref().map(|(age, _)| age.as_millis() as u64),
                // this runtime's own sessions may have said something newer than their last
                // beat carried: a phase inside the interval is held, not written (I3990)
                phase: c
                    .session_phase(&session.key)
                    .or_else(|| progress.and_then(|(_, phase)| phase)),
            });
        }
    }

    let own_name = status
        .peers
        .iter()
        .find(|p| p.local)
        .map(|p| p.name.clone());
    let mut machines: BTreeMap<String, MachineNode> = BTreeMap::new();
    for (key, runtime) in runtimes {
        let node = key.get(..32).unwrap_or_default().to_string();
        let name = runtime
            .link
            .as_ref()
            .map(|l| l.name.clone())
            .or_else(|| (node == own_node).then(|| own_name.clone()).flatten())
            .unwrap_or_else(|| node.chars().take(8).collect());
        machines
            .entry(node.clone())
            .or_insert_with(|| MachineNode {
                node: node.clone(),
                name,
                local: node == own_node,
                runtimes: Vec::new(),
            })
            .runtimes
            .push(runtime);
    }
    let mut machines: Vec<MachineNode> = machines.into_values().collect();
    crate::order::canonical(&mut machines);
    PeerTree {
        active: status.active,
        view: query.view(),
        reason: status.reason,
        runtime: status.runtime,
        repository: status.repository.map(|r| r.id),
        machines,
        refused: status.refused,
        digest: Some(whole.digest),
    }
}

fn mesh_peers(ctx: &Context, query: Query) -> Result<PeerTree, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => peer_tree(&c, &query),
        None => PeerTree {
            active: false,
            view: query.view(),
            reason: ctx.mesh.cooperation_status().reason,
            runtime: None,
            repository: None,
            machines: Vec::new(),
            refused: Vec::new(),
            digest: None,
        },
    })
}

// ---------------------------------------------------------------- mesh.peer

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.peer`.
pub struct PeerInput {
    /// The runtime, `<node>-<runtime>` (a node id alone matches its first runtime).
    pub runtime: String,
}

impl BenchmarkCases for PeerInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            PeerInput {
                runtime: "0".repeat(49),
            },
        )]
    }
}

/// The answer of `mesh.peer`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerDetail {
    /// Whether the runtime is known here.
    pub found: bool,
    /// Why not, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The machine it runs on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// The runtime, its link and its sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeNode>,
    /// Refusals recorded against it.
    pub refused: Vec<RefusedView>,
}

fn mesh_peer(ctx: &Context, input: PeerInput) -> Result<PeerDetail, CapabilityError> {
    let Some(c) = ctx.mesh.cooperation() else {
        return Ok(PeerDetail {
            found: false,
            reason: ctx.mesh.cooperation_status().reason,
            machine: None,
            runtime: None,
            refused: Vec::new(),
        });
    };
    let full = Query {
        view: Some(View::Full),
        ..Query::default()
    };
    let tree = peer_tree(&c, &full);
    let wanted = input.runtime.trim();
    let hit = tree.machines.iter().find_map(|m| {
        m.runtimes
            .iter()
            .find(|r| r.runtime == wanted || (wanted.len() == 32 && r.runtime.starts_with(wanted)))
            .map(|r| (m.name.clone(), r.clone()))
    });
    let refused = tree
        .refused
        .into_iter()
        .filter(|r| r.runtime.as_deref() == Some(wanted))
        .collect();
    Ok(match hit {
        Some((machine, runtime)) => PeerDetail {
            found: true,
            reason: None,
            machine: Some(machine),
            runtime: Some(runtime),
            refused,
        },
        None => PeerDetail {
            found: false,
            reason: Some(format!("no runtime {wanted} is linked or heard here")),
            machine: None,
            runtime: None,
            refused,
        },
    })
}

// ---------------------------------------------------------------- mesh.state

/// One stream's liveness.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamView {
    /// The stream.
    pub stream: StreamId,
    /// Own, live or expired.
    pub liveness: StreamLiveness,
    /// Milliseconds since its beat rose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat_ms: Option<u64>,
}

/// The answer of `mesh.state`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshStateAnswer {
    /// Whether cooperation runs.
    pub active: bool,
    /// What the answer carries: `summary` or `full`.
    pub view: View,
    /// Why not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The summary, in the summary view: counts, each runtime's newest live claim, the
    /// open reviews and the untaken handovers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    /// The folded state narrowed by the filters, in the full view; its digest is the whole
    /// fold's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<CooperationState>,
    /// Every stream's liveness.
    pub streams: Vec<StreamView>,
    /// The build-cache offers this runtime reads from the journal, and every record it does
    /// not read as one with why (I2671); absent when the journal holds none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_cache: Option<crate::mesh::build_cache::offer::OfferReading>,
}

fn mesh_state(ctx: &Context, query: Query) -> Result<MeshStateAnswer, CapabilityError> {
    let view = query.view();
    let Some(c) = ctx.mesh.cooperation() else {
        return Ok(MeshStateAnswer {
            active: false,
            view,
            reason: ctx.mesh.cooperation_status().reason,
            summary: None,
            state: None,
            streams: Vec::new(),
            build_cache: None,
        });
    };
    let state = page::narrow(&c.state(), &query);
    let build_cache =
        Some(c.build_cache_offers()).filter(|r| !(r.offers.is_empty() && r.ignored.is_empty()));
    let streams: Vec<StreamView> = c
        .streams()
        .into_iter()
        .filter(|(stream, _, _)| query.admits_runtime(&stream.runtime_key()))
        .map(|(stream, liveness, last_beat_ms)| StreamView {
            stream,
            liveness,
            last_beat_ms,
        })
        .collect();
    Ok(match view {
        View::Summary => {
            let beats: Vec<_> = streams
                .iter()
                .map(|s| (s.stream.runtime_key(), s.liveness, s.last_beat_ms))
                .collect();
            MeshStateAnswer {
                active: true,
                view,
                reason: None,
                summary: Some(page::summarize(&state, &beats)),
                state: None,
                // the streams are the summary's runtimes, told once
                streams: Vec::new(),
                build_cache,
            }
        }
        View::Full => MeshStateAnswer {
            active: true,
            view,
            reason: None,
            summary: None,
            state: Some(state),
            streams,
            build_cache,
        },
    })
}

// ---------------------------------------------------------------- mesh.events

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.events`.
pub struct MeshEventsInput {
    /// Only events whose Lamport stamp is above this: a walk by stamp, which never returns
    /// an event a partitioned runtime stamped at or below it and delivered later. Prefer
    /// `cursor`, which returns those too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// The `cursor` the previous page answered: the next page is what this run of the
    /// runtime applied after it, every event exactly once across the whole walk, a late one
    /// from a healed partition included (I2733). A cursor another run issued is refused as
    /// `stale_cursor`. Without `cursor` and `after` the walk starts at the beginning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// At most this many (default 100, at most 1000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// The newest `limit` events above `after` instead of the first ones: what a reader who
    /// wants the journal as it stands now asks for. Still in Lamport order, and `lamport` is
    /// then the newest stamp the journal holds, so resuming after it reads only what comes
    /// next. Without it the journal is read forward from `after`, a page at a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest: Option<bool>,
    /// Include the `session_beat` events, which are left out by default: a working session's
    /// runtime writes one every few minutes to keep its claims alive (I3990), and a reader
    /// of the journal wants what sessions did rather than that they are still doing it.
    /// A walk by `after` cuts its page before the beats leave it, so its `lamport` resumes
    /// after them; a walk by `cursor` passes them over without counting them toward `limit`.
    /// Paging misses nothing either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beats: Option<bool>,
}

impl BenchmarkCases for MeshEventsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            MeshEventsInput {
                after: None,
                cursor: None,
                limit: Some(100),
                newest: None,
                beats: None,
            },
        )]
    }
}

/// The answer of `mesh.events`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventList {
    /// Whether cooperation runs.
    pub active: bool,
    /// How many events this answer carries.
    pub count: usize,
    /// The `after` of the next page of a walk by stamp: the stamp of the last event this
    /// page read — carried, or a session beat it left out — or the `after` that was asked
    /// when it read none. A page never ends inside a run of equal stamps (I2140).
    pub lamport: u64,
    /// The `cursor` of the next page: pass it back as `cursor` and the walk resumes exactly
    /// after what this page returned (I2733). Absent from an answer to a walk by `after`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// The events, in Lamport order.
    pub events: Vec<MeshEvent>,
}

fn mesh_events(ctx: &Context, input: MeshEventsInput) -> Result<EventList, CapabilityError> {
    let Some(c) = ctx.mesh.cooperation() else {
        return Ok(EventList {
            active: false,
            count: 0,
            lamport: 0,
            cursor: None,
            events: Vec::new(),
        });
    };
    let limit = input.limit.unwrap_or(100).clamp(1, 1000) as usize;
    if input.after.is_some() && input.cursor.is_some() {
        return Err(CapabilityError::InvalidInput(
            "after and cursor are two different walks: pass one of them".into(),
        ));
    }
    let after = input.after.unwrap_or(0);
    let shown = |e: &MeshEvent| input.beats == Some(true) || e.kind() != "session_beat";
    let (events, lamport, cursor) = if input.newest == Some(true) {
        // the whole tail above `after`, in order, and then its end: the journal is in memory
        // and bounded by its own retention, so this is a walk and not a second index
        let head = c.journal().head_cursor();
        let mut all = c.journal().events_after(after, usize::MAX);
        let lamport = all.last().map_or(after, |e| e.lamport);
        all.retain(|e| shown(e));
        let from = all.len().saturating_sub(limit);
        all.drain(..from);
        (all, lamport, Some(head))
    } else if input.after.is_some() {
        // the page is cut before the beats leave it, so its `lamport` resumes after them
        let mut page = c.journal().events_after(after, limit);
        let lamport = page.last().map_or(after, |e| e.lamport);
        page.retain(|e| shown(e));
        (page, lamport, None)
    } else {
        let from = match &input.cursor {
            Some(text) => Some(text.parse::<Cursor>().map_err(cursor_refusal)?),
            None => None,
        };
        // a beat left out moves the cursor past it without taking a place on the page, so a
        // page is short only at the end of the journal and the walk still misses nothing
        let (page, next) = c
            .journal()
            .events_since_shown(from.as_ref(), limit, shown)
            .map_err(cursor_refusal)?;
        let lamport = page.last().map_or(after, |e| e.lamport);
        (page, lamport, Some(next))
    };
    Ok(EventList {
        active: true,
        count: events.len(),
        lamport,
        cursor: cursor.map(|c| c.to_string()),
        events,
    })
}

// ---------------------------------------------------------------- mesh.verify

fn mesh_verify(ctx: &Context, _: Empty) -> Result<MeshVerifyReport, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => c.verify(),
        None => MeshVerifyReport::inactive(
            &ctx.mesh
                .cooperation_status()
                .reason
                .unwrap_or_else(|| "cooperation is not running".into()),
        ),
    })
}

// ---------------------------------------------------------------- mesh.session.*

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.session.open`: what a session says about itself.
pub struct SessionInput {
    /// The session id within this runtime; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The client (`claude-code`, `codex`, `cli`); `cli` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// The worker's name for itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// What it is doing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The task id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Whether the working tree is dirty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// The context revision it works from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// The client process the session belongs to, on this machine: the runtime closes the
    /// session once that process has ended (I3810).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_pid: Option<u32>,
    /// When that process started, as this machine's process table reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_started: Option<String>,
}

impl BenchmarkCases for SessionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "named",
            SessionInput {
                session: Some("bench".into()),
                client: None,
                worker: None,
                intent: Some("benchmark".into()),
                task: None,
                issue: None,
                milestone: None,
                branch: None,
                head: None,
                dirty: None,
                context: None,
                client_pid: None,
                client_started: None,
            },
        )]
    }
}

fn mesh_session_open(ctx: &Context, input: SessionInput) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    c.open_session(SessionInfo {
        session,
        client: input.client.unwrap_or_else(|| "cli".into()),
        worker: input.worker,
        intent: input.intent,
        task: input.task,
        issue: input.issue,
        milestone: input.milestone,
        checkout: None,
        branch: input.branch,
        head: input.head,
        dirty: input.dirty,
        context: input.context,
        client_pid: input.client_pid,
        client_started: input.client_started,
    })
    .map_err(map_error)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.session.close`.
pub struct SessionCloseInput {
    /// The session; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl BenchmarkCases for SessionCloseInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "named",
            SessionCloseInput {
                session: Some("bench".into()),
            },
        )]
    }
}

fn mesh_session_close(ctx: &Context, input: SessionCloseInput) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    c.close_session(&session).map_err(map_error)
}

// ---------------------------------------------------------------- mesh.claim / release

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.claim`.
pub struct ClaimInput {
    /// The claiming session; the calling MCP session's own when omitted. Opened when new.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The client, for a session this opens; `cli` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// Repository-relative paths.
    pub scope: Vec<String>,
    /// What the claim is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// `exclusive` (the default) or `advisory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ClaimMode>,
    /// The issue the claim is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The task, for a session this opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

impl BenchmarkCases for ClaimInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "exclusive",
            ClaimInput {
                session: Some("bench".into()),
                client: None,
                scope: vec!["docs".into()],
                intent: None,
                mode: None,
                issue: None,
                task: None,
            },
        )]
    }
}

fn mesh_claim(ctx: &Context, input: ClaimInput) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    let info = SessionInfo {
        session,
        client: input.client.unwrap_or_else(|| "cli".into()),
        task: input.task,
        issue: input.issue.clone(),
        intent: input.intent.clone(),
        ..SessionInfo::default()
    };
    c.claim(
        &info,
        input.scope,
        input.intent,
        input.mode.unwrap_or_default(),
        input.issue,
    )
    .map_err(map_error)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.release`.
pub struct ReleaseInput {
    /// The claim's key, `<stream>/<claim>`, as `mesh.claim` answered it.
    pub claim: String,
}

impl BenchmarkCases for ReleaseInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            ReleaseInput {
                claim: "none/c-0".into(),
            },
        )]
    }
}

fn mesh_release(ctx: &Context, input: ReleaseInput) -> Result<Written, CapabilityError> {
    cooperation(ctx)?.release(&input.claim).map_err(map_error)
}

// ---------------------------------------------------------------- mesh.beat

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.beat`.
pub struct BeatInput {
    /// The session making progress; the calling MCP session's own when omitted. It must be
    /// a session this runtime opened (a claim opens one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// What the session is doing, one line: `waiting on CI run 123`, `holding the build
    /// lock`. Redacted before it is signed, and shown beside the session everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// The process the beat speaks for; refused unless it is running on this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl BenchmarkCases for BeatInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            BeatInput {
                session: Some("none".into()),
                phase: Some("waiting on CI".into()),
                pid: None,
            },
        )]
    }
}

/// A session says it is making progress, and what it is doing (I3990): the typed way for a
/// long-running process to keep its exclusive claims excluding while it sends no MCP request.
fn mesh_beat(ctx: &Context, input: BeatInput) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    // Over MCP a beat speaks for the calling session alone: one client must not keep another
    // client's claims excluding. The command line and plain HTTP name the session they beat
    // for — a long-running script is not an MCP session — and a caller beyond loopback is
    // refused every write before it gets here (ADR 0126).
    if let (Some(peer), Some(named)) = (ctx.caller.as_ref(), input.session.as_deref()) {
        let own = format!("board-{}", peer.as_str());
        if !named.trim().is_empty() && named != own {
            return Err(CapabilityError::Refused(format!(
                "not_own: over MCP a session beats only for itself ({own}), not for '{named}'"
            )));
        }
    }
    let session = session_of(ctx, input.session)?;
    c.beat(&session, input.phase, input.pid).map_err(map_error)
}

// ---------------------------------------------------------------- mesh.handover.*

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.handover.publish`.
pub struct HandoverPublishInput {
    /// The record, relative to the repository root and inside
    /// `.ai/local/state/handovers/`; the newest record when omitted. Nothing outside that
    /// directory is ever read by this command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The issue the handover belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
}

impl BenchmarkCases for HandoverPublishInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "latest",
            HandoverPublishInput {
                path: None,
                issue: None,
                milestone: None,
            },
        )]
    }
}

fn mesh_handover_publish(
    ctx: &Context,
    input: HandoverPublishInput,
) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let root = std::path::Path::new(&ctx.index.repository.root);
    let dir = crate::mesh::handover::directory(root);
    let path = match input.path {
        None => crate::mesh::handover::latest(root).map_err(CapabilityError::NotFound)?,
        Some(p) => {
            let candidate = root.join(p.trim_start_matches('/'));
            let (Ok(real), Ok(real_dir)) = (candidate.canonicalize(), dir.canonicalize()) else {
                return Err(CapabilityError::NotFound(format!(
                    "no handover record at {p}"
                )));
            };
            if !real.starts_with(&real_dir) {
                return Err(CapabilityError::Refused(format!(
                    "{p} is not a handover record: only files under .ai/local/state/handovers/ are published"
                )));
            }
            real
        }
    };
    let body = crate::mesh::handover::to_body(&path, input.issue, input.milestone)
        .map_err(CapabilityError::InvalidInput)?;
    c.publish_handover(body).map_err(map_error)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.handover.consume`.
pub struct HandoverConsumeInput {
    /// The handover's id (its content digest).
    pub handover: String,
    /// The consuming session; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Whether to write it as a local handover record `majordomus handover --resolve`
    /// finds (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialize: Option<bool>,
}

impl BenchmarkCases for HandoverConsumeInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            HandoverConsumeInput {
                handover: "0".repeat(32),
                session: Some("bench".into()),
                materialize: Some(false),
            },
        )]
    }
}

/// The answer of `mesh.handover.consume`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsumeAnswer {
    /// The handover, as the journal holds it.
    pub handover: HandoverView,
    /// The consumption event.
    pub written: Written,
    /// The local record written, relative to the repository root, when materialized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

fn mesh_handover_consume(
    ctx: &Context,
    input: HandoverConsumeInput,
) -> Result<ConsumeAnswer, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    let (view, written) = c
        .consume_handover(&input.handover, &session)
        .map_err(map_error)?;
    let root = std::path::Path::new(&ctx.index.repository.root);
    let path = if input.materialize.unwrap_or(true) {
        let written =
            crate::mesh::handover::materialize(root, &view).map_err(CapabilityError::Internal)?;
        Some(
            written
                .strip_prefix(root)
                .unwrap_or(&written)
                .display()
                .to_string(),
        )
    } else {
        None
    };
    Ok(ConsumeAnswer {
        handover: view,
        written,
        path,
    })
}

// ---------------------------------------------------------------- mesh.review.*

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.review.request`.
pub struct ReviewRequestInput {
    /// The requesting session; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// What to review: a branch, a commit, a pull request.
    pub subject: String,
    /// The paths it covers.
    #[serde(default)]
    pub scope: Vec<String>,
    /// The issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The runtime asked (`<node>-<runtime>`); anyone when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// What the reviewer needs beyond the subject: evidence, commands, the question. Up to
    /// 8 KiB, several lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl BenchmarkCases for ReviewRequestInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "open",
            ReviewRequestInput {
                session: Some("bench".into()),
                subject: "feature/bench".into(),
                scope: vec![],
                issue: None,
                reviewer: None,
                body: None,
            },
        )]
    }
}

fn mesh_review_request(
    ctx: &Context,
    input: ReviewRequestInput,
) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    c.request_review(
        &session,
        input.subject,
        input.scope,
        input.issue,
        input.reviewer,
        input.body,
    )
    .map_err(map_error)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.review.answer`.
pub struct ReviewAnswerInput {
    /// The request's key, `<stream>/<review>`.
    pub request: String,
    /// The answering session; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// `approved`, `changes_requested` or `commented`.
    pub verdict: String,
    /// The note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl BenchmarkCases for ReviewAnswerInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            ReviewAnswerInput {
                request: "none/r-0".into(),
                session: Some("bench".into()),
                verdict: "commented".into(),
                note: None,
            },
        )]
    }
}

fn mesh_review_answer(ctx: &Context, input: ReviewAnswerInput) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    c.answer_review(&input.request, &session, input.verdict, input.note)
        .map_err(map_error)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.review.withdraw`.
pub struct ReviewWithdrawInput {
    /// The request's key, `<stream>/<review>`.
    pub request: String,
    /// The withdrawing session of the asking runtime; the calling MCP session's own when
    /// omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The requester's result or reason, up to 8 KiB: what came of it, or why it is void.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl BenchmarkCases for ReviewWithdrawInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "unknown",
            ReviewWithdrawInput {
                request: "none/r-0".into(),
                session: Some("bench".into()),
                note: Some("superseded".into()),
            },
        )]
    }
}

fn mesh_review_withdraw(
    ctx: &Context,
    input: ReviewWithdrawInput,
) -> Result<Written, CapabilityError> {
    let c = cooperation(ctx)?;
    let session = session_of(ctx, input.session)?;
    c.withdraw_review(&input.request, &session, input.note)
        .map_err(map_error)
}

// ---------------------------------------------------------------- mesh.link.*

impl BenchmarkCases for Signed {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // Malformed: refused before any signature is checked or any state is touched.
        vec![NamedCase::new(
            "malformed",
            Signed {
                body: serde_json::json!({}),
                sig: String::new(),
            },
        )]
    }
}

fn not_active(ctx: &Context) -> LinkReply {
    LinkReply::refused(
        RefusalCode::NotActive,
        ctx.mesh
            .cooperation_status()
            .reason
            .unwrap_or_else(|| "cooperation is not running".into()),
    )
}

fn mesh_link_hello(ctx: &Context, input: Signed) -> Result<LinkReply, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => c.accept_hello(&input),
        None => not_active(ctx),
    })
}

fn mesh_link_sync(ctx: &Context, input: Signed) -> Result<LinkReply, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => c.accept_sync(&input),
        None => not_active(ctx),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "mesh",
        title: "Mesh",
        description: "The mesh of this process: discovery — authenticated observations of other running Majordomus instances in one registry — and cooperation — authenticated links to trusted runtimes of the same repository, replicating sessions, claims, handovers and reviews through one journal every runtime folds into the same state. Discovery grants nothing; a link is admitted per peer, and nothing a peer sends executes anything here.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "mesh.status",
                title: "The mesh, at a glance",
                description: "Whether the mesh runs in this process and why not when it does not; this node's public identity; every discovery provider with its state and counters; the registry's tallies; and the refused datagrams by reason. The one status every surface shows.",
                input: Empty,
                output: MeshStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_mesh".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: MESH_URI.into(),
                            name: "mesh".into(),
                        }),
                    }),
                    http: get("/api/v1/mesh"),
                    cli: None,
                },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_status,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.nodes",
                title: "The discovered nodes",
                description: "Every runtime this process has observed, one record per node and runtime slot, deduplicated across every discovery source, in node-id order: identity, trust, presence, endpoints, capabilities, repositories, provenance and versions. In memory, gone with the process.",
                input: Empty,
                output: NodeList,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_nodes"), http: get("/api/v1/mesh/nodes"), cli: None },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_nodes,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.enrol",
                kind: CapabilityKind::Command,
                title: "Enrol this machine in the mesh",
                description: "Add this machine's public key to trust.allow of the repository's mesh declaration, under a comment naming the machine, creating the node identity first when there is none. Idempotent: a listed key changes nothing. The comments and every other line of the declaration stay as they were, and an edit that would not parse back as a declaration is refused before anything is written. Under deny_unknown a machine opens its mesh only once its key is listed, so this is the one step that joins a machine; commit the file and restart the server.",
                input: Empty,
                output: MeshEnrolment,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_enrol"),
                    http: post("/api/v1/mesh/enrol"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "enrol".into()] }),
                },
                tags: ["mesh", "identity", "trust"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: mesh_enrol,
            }
            .writes_repository(),
            capability! {
                id: "mesh.identity",
                title: "This machine's node identity",
                description: "The node identity kept under the user's state directory, public half only: node id, public key, display name. The signing key appears in no projection. Absent is an answer, not an error — the identity is created when a mesh first activates.",
                input: Empty,
                output: MeshIdentityReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_identity"),
                    http: get("/api/v1/mesh/identity"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "identity".into()] }),
                },
                tags: ["mesh", "identity"],
                handler: mesh_identity,
            },
            capability! {
                id: "mesh.doctor",
                title: "The mesh self-check",
                description: "Every prerequisite proved on this machine alone — the declaration parses, the identity loads and, under an allowlist, is on it, the repository has a mesh identity, a UDP socket binds, the multicast group joins, broadcast enables, and the discovery and link protocols sign and verify end to end in memory — and then the server's own verdict: in a shared server, whether an enabled declaration actually activated the mesh and, when it did not, the server's reason, which fails the report. Each failed check names its impact and its remedy. Deterministic, no second node required.",
                input: Empty,
                output: MeshDoctorReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_doctor"),
                    http: get("/api/v1/mesh/doctor"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "doctor".into()] }),
                },
                tags: ["mesh", "diagnostics"],
                handler: mesh_doctor,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.build_cache.env",
                title: "The sccache client configuration for the store this node selected",
                description: "What the process that starts the sccache server should have in its environment on this machine, derived and never typed: the local disk as the first level and the selected store as the second (SCCACHE_MULTILEVEL_CHAIN=disk,s3, the bucket, the endpoint, the region, whether TLS is used, a key prefix of rustc's host triple, the credential FILE path and its profile) only when sccache has the multi-level chain (measured on a throwaway server, not assumed), a credential file of mode 0600 holding the profile exists, and the store answered its readiness route now; otherwise no variable at all, with the one condition that did not hold. sccache fixes its chain at server start and fails the compile when a level is down then, so a store is added only to a server that is starting. No key is ever in the answer, only the credential file's path (ADR 0157, I2673).",
                input: BuildCacheEnvInput,
                output: crate::mesh::build_cache::env::EnvReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_build_cache_env"),
                    http: get("/api/v1/mesh/build-cache/env"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "build-cache".into(), "env".into()] }),
                },
                tags: ["mesh", "build", "cache"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: mesh_build_cache_env,
            }
            // this machine's own sccache, files and store, not the repository's: a loopback caller only
            .reads_only_locally(),
            capability! {
                id: "mesh.build_cache.key",
                title: "What every dependency install and build directory is cached under",
                description: "Every lockfile the repository tracks — Cargo.lock, mix.lock, package-lock.json — as the thing it makes cacheable (a cargo build directory, mix dependencies, npm node_modules), each with its key: a schema version, the platform (the toolchain's own host triple, or `any` for platform-independent source), the toolchain as its executables state it, and the digest of every input and environment variable that changes what the install leaves. Where the entry is restored to is a separate field, so two checkouts with one lockfile share one entry. A kind whose toolchain is absent says why and has no key (ADR 0157, I3950).",
                input: BuildCacheKeyInput,
                output: crate::mesh::build_cache::key::KeyReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_build_cache_key"),
                    http: get("/api/v1/mesh/build-cache/key"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "build-cache".into(), "key".into()] }),
                },
                tags: ["mesh", "build", "cache"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: mesh_build_cache_key,
            },
            capability! {
                id: "mesh.who",
                title: "Who runs this",
                description: "The session behind a process, a path, an issue or an offloaded run (I3956): a pid (or `host:pid` on a declared offload host, read through its ssh path only) is read where it runs — its parent chain up to the provider's process and its messaging socket, its group, start, command and working directory, and the owner variables of its environment — and joined with that host's run records, the repository's board (whose rows carry each worker's address) and the mesh claims; a run tag is the record its host keeps; an issue or a path is every live mesh claim, announcement and run record naming or covering it. Each owner comes with its board peer, issue, address and the facts that named it. A process whose parent is init and whose directory is gone is marked a leftover and named with its owner when its environment or a run record knows it. Nothing is guessed from a name: an answer that cannot name an owner says unknown and lists the evidence it lacked. Reads processes and records; signals nothing.",
                input: WhoInput,
                output: crate::mesh::who::WhoReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_who"),
                    http: get("/api/v1/mesh/who"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "who".into()] }),
                },
                tags: ["mesh", "coordination", "diagnostics"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: mesh_who,
            }
            // the machine's processes and the board, not the repository's: a loopback caller only
            .reads_only_locally(),
            capability! {
                id: "mesh.build_cache.ensure",
                kind: CapabilityKind::Command,
                title: "Restore an install before its command, and save it after",
                description: "For one lockfile the repository tracks (npm node_modules, mix deps, or the cargo build directory), skip an install whose key is already in place; otherwise restore it from this machine's local archive level (~/.cache/majordomus/build-cache) and then from the store this node declares when its own probe finds it healthy and a credential file is present, each only after the archive's SHA-256 matches the digest recorded when it was saved and every entry is judged safe (no absolute name, no `..`, no link leaving the target). On a miss the command runs, and when it succeeds what it left is archived to the local level and the store. A failure of any level is a miss: the command runs exactly as without the cache. npm's command is not run on a hit; mix's is; a cargo build directory is restored only when cold, under cargo's own lock, without incremental state, and never saved above one PUT. A restore and a save respect the build's free-space floor (MAJORDOMUS_MIN_FREE_MB); the local level is held under the policy's build_cache.local_max_gib, oldest first. MAJORDOMUS_BUILD_CACHE=off bypasses every level. Offered on the command line only: it runs a command (ADR 0157, I3952).",
                input: BuildCacheEnsureInput,
                output: crate::mesh::build_cache::ensure::EnsureReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["mesh".into(), "build-cache".into(), "ensure".into()] }),
                },
                tags: ["mesh", "build", "cache"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: mesh_build_cache_ensure,
            },
            capability! {
                id: "mesh.firewall",
                title: "The host firewall, as the mesh needs it",
                description: "What inbound traffic the declaration implies this machine must admit — the multicast group's port, the broadcast port when it is a permitted fallback, every declared hub port whose address is this machine's, and the server's own port when it listens beyond loopback — from the private networks the declaration names; which firewall front this host runs (ufw, nftables, the macOS application firewall); the commands that admit the plan there; what the firewall itself says about the plan now (present, missing, inactive, or unobservable without root); and what the kernel logged it dropping toward those ports in the last five minutes. Fails when a rule is observed missing or a drop was logged. Reads the host; changes nothing.",
                input: FirewallInput,
                output: FirewallReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_firewall"),
                    http: get("/api/v1/mesh/firewall"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "firewall".into()] }),
                },
                tags: ["mesh", "diagnostics", "firewall"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: mesh_firewall,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.firewall.apply",
                kind: CapabilityKind::Command,
                title: "Admit the mesh through the host firewall",
                description: "Run the commands mesh.firewall renders, on this host, as root: one allow per rule and source network on ufw or nftables, the executable admitted on the macOS application firewall; nothing else is touched, and every rule written carries the comment `majordomus mesh` so it can be told from an operator's own. Refuses, running nothing, without root or without a backend; records every command with its exit and output; and asks the firewall again afterwards, so the verdict is the firewall's. Offered on the command line only: it runs a privileged host tool, which nothing reachable over HTTP or MCP may do.",
                input: FirewallInput,
                output: FirewallApplyReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["mesh".into(), "firewall".into(), "apply".into()] }),
                },
                tags: ["mesh", "firewall"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: mesh_firewall_apply,
            },
            capability! {
                id: "mesh.build_cache.provision",
                kind: CapabilityKind::Command,
                title: "Provision this node as a mesh build cache provider",
                description: "The host layer of a cache provider as one declared policy: a disk guard on a timer, services that restart always and are last for the OOM killer, a bounded journal and container log, a private bucket with expiry and a hard quota beside the CI cache's own bound, one least-privilege identity per node whose credential is generated on the host into a mode 0600 file outside any repository, and the rustc wrapper that makes the node's own builds use the cache. Without `apply` it plans only: every file, edit and command still owed, from what the host holds, with no secret named. With `apply` it writes what is missing or drifted and runs the commands, as root, refusing without it before touching anything; a second apply changes nothing. A `root` other than `/` is a staging directory: files only, no privilege, no command. Offered on the command line only: it installs services and writes credentials, which nothing reachable over HTTP or MCP may do.",
                input: ProvisionInput,
                output: ProvisionReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["mesh".into(), "build-cache".into(), "provision".into()] }),
                },
                tags: ["mesh", "build-cache", "provisioning"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: provision,
            },
            capability! {
                id: "mesh.register",
                kind: CapabilityKind::Command,
                title: "Register with this node's mesh",
                description: "Present one signed envelope; it is verified exactly as a datagram — bounds, staleness, signature, trust policy — and recorded as a rendezvous observation when it holds. The answer carries this node's own envelope and the candidates its registry holds, each verifiable end to end on its own signature. Registration grants nothing: the caller becomes a record, never an authorization. Changes this process's memory only.",
                input: RegisterInput,
                output: RegisterAnswer,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_register"), http: post(REGISTER_PATH), cli: None },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_register,
            }
            // the caller is another machine by design, admitted by the signature on its envelope
            // and the trust policy, never by its address (ADR 0126)
            .authenticates_its_input(),
            capability! {
                id: "mesh.cooperation",
                title: "Cooperation, at a glance",
                description: "Whether this runtime cooperates and why not when it does not: its runtime and stream, the repository identity links are matched on, the link protocol and features, the heartbeat and expiry, every linked peer with its link state, last exchange, round trip and failures, every refused candidate with the rule that refused it, and the counters of handshakes, syncs, reconnects, expiries, replicated and refused events.",
                input: Empty,
                output: CooperationStatus,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_cooperation"), http: get("/api/v1/mesh/cooperation"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_cooperation,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.peers",
                title: "Who cooperates, machine by machine",
                description: "Machine → runtime → session → claim: every runtime this one is linked to or has heard through the journal, grouped by machine, local first; each runtime with its liveness, the milliseconds since its beat rose, its link and its newest live claim. The default `summary` view lists only the active sessions, each with its live claims counted and its intent cut; `view=full` lists every session with every claim. `session` and `runtime` narrow either. Remote and local runtimes share one shape, and the whole fold's digest closes the answer: equal digests, equal state.",
                input: Query,
                output: PeerTree,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_peers"), http: get("/api/v1/mesh/peers"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_peers,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.peer",
                title: "One runtime",
                description: "One runtime by its key (`<node>-<runtime>`, or a node id for its first runtime): the machine it runs on, its liveness, its link from here, its sessions and claims, and any refusal recorded against it. Not found is an answer with the reason.",
                input: PeerInput,
                output: PeerDetail,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_peer"), http: get("/api/v1/mesh/peer"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_peer,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.state",
                title: "The state every linked runtime converges on",
                description: "The journal folded. The default `summary` view is bounded whatever the history: sessions and claims counted by standing, each runtime with its liveness, its counts and its newest live claim, the open review requests and the handovers nobody has taken, and the digest two runtimes compare. `view=full` is every session, every claim with its standing (held, released, expired, conflicted with its winner), the advisory overlaps, every handover with who consumed it, every review request with its answers, and every stream's liveness on this runtime's clock. `claims=live`, `after` (a Lamport stamp), `session` and `runtime` narrow either view; the digest is always the whole fold's.",
                input: Query,
                output: MeshStateAnswer,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_state"), http: get("/api/v1/mesh/state"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_state,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.events",
                title: "The cooperation journal",
                description: "The journal's events a page at a time, in Lamport order: each with its stream, sequence, stamp, repository, signing key, kind and body. The answer's cursor is the next page's `cursor`, and a walk resumed from each one returns every event exactly once, a partitioned runtime's late event included; `after` still walks by Lamport stamp.",
                input: MeshEventsInput,
                output: EventList,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_events"), http: get("/api/v1/mesh/events"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_events,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
            capability! {
                id: "mesh.verify",
                kind: CapabilityKind::Command,
                title: "Prove cooperation now",
                description: "Run a live verification: cooperation is active, the repository identity resolves, the endpoints are reachable beyond loopback, the heartbeat beats, the journal holds no stuck gap; then one sync round with every peer this runtime dials, timed, and whether both sides now hold the same high-water marks. Each failed check names its impact and remedy. Changes nothing but the journal's replication.",
                input: Empty,
                output: MeshVerifyReport,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_verify"), http: post("/api/v1/mesh/verify"), cli: None },
                tags: ["mesh", "diagnostics", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: mesh_verify,
            },
            capability! {
                id: "mesh.session.open",
                kind: CapabilityKind::Command,
                title: "Open or update a session on the mesh",
                description: "Say what a session of this runtime is: its client, worker, intent, task, issue, milestone, branch, head, dirtiness and context revision. Replicated to every linked runtime; a later word replaces an earlier one. Writes this runtime's journal only.",
                input: SessionInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_session_open"), http: post("/api/v1/mesh/sessions"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_session_open,
            },
            capability! {
                id: "mesh.session.close",
                kind: CapabilityKind::Command,
                title: "Close a session on the mesh",
                description: "End a session of this runtime; every claim it holds ends with it, on every linked runtime. Writes this runtime's journal only.",
                input: SessionCloseInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_session_close"), http: post("/api/v1/mesh/sessions/close"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_session_close,
            },
            capability! {
                id: "mesh.claim",
                kind: CapabilityKind::Command,
                title: "Claim a scope across the mesh",
                description: "Claim repository paths for a session. An exclusive claim that meets a live exclusive claim of another session — on this runtime or any runtime this one has heard — is refused as `claim_conflict` with the claims it meets; an advisory claim is recorded and its overlaps reported. A claim lives while its session is open and its runtime beats: a crashed holder's claim expires everywhere on its own; and an exclusive claim of a session that beats is suspended — refusing nobody — while its holder makes no progress within its ttl. Writes this runtime's journal only.",
                input: ClaimInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_claim"), http: post("/api/v1/mesh/claims"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_claim,
            },
            capability! {
                id: "mesh.release",
                kind: CapabilityKind::Command,
                title: "Release a claim",
                description: "Release a claim this runtime's current run holds, by its key. A claim written elsewhere is refused as `not_own`: only its holder releases it, and a dead holder's claim expires instead. Writes this runtime's journal only.",
                input: ReleaseInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_release"), http: post("/api/v1/mesh/claims/release"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_release,
            },
            capability! {
                id: "mesh.beat",
                kind: CapabilityKind::Command,
                title: "Say a session is making progress",
                description: "Record that a session of this runtime is making progress, with the phase it is in (`waiting on CI run 123`) and optionally the running process it speaks for. A session that has beaten lives by its beats: its exclusive claims are suspended — they refuse nobody — once no beat arrives within `cooperation.session_ttl_seconds`, and hold again at the next. A worker's MCP requests other than `ping` beat for it; this is the way for a long-running process that sends none. A beat within a quarter of the ttl with an unchanged phase writes nothing and answers the last one. Writes this runtime's journal only.",
                input: BeatInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_beat"), http: post("/api/v1/mesh/beats"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_beat,
            },
            capability! {
                id: "mesh.handover.publish",
                kind: CapabilityKind::Command,
                title: "Publish a handover to the mesh",
                description: "Publish a handover record of this checkout — the newest, or the one named under .ai/local/state/handovers/ — to every linked runtime: its task, branch, head and time from its front matter, the issue and milestone given, and its Markdown body, bounded and identified by the body's digest. No path of this machine travels, and no file outside the handovers directory is ever read.",
                input: HandoverPublishInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_handover_publish"), http: post("/api/v1/mesh/handovers"), cli: None },
                tags: ["mesh", "continuity", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_handover_publish,
            },
            capability! {
                id: "mesh.handover.consume",
                kind: CapabilityKind::Command,
                title: "Consume a handover from the mesh",
                description: "Take a handover another runtime published: record the consumption on the mesh, and write it into this checkout's handovers directory as a record `majordomus handover --resolve` finds on the same branch — once, however often it is consumed.",
                input: HandoverConsumeInput,
                output: ConsumeAnswer,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_handover_consume"), http: post("/api/v1/mesh/handovers/consume"), cli: None },
                tags: ["mesh", "continuity", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_handover_consume,
            },
            capability! {
                id: "mesh.review.request",
                kind: CapabilityKind::Command,
                title: "Ask the mesh for a review",
                description: "Ask for a review of a branch, commit or pull request, optionally of one named runtime — which must be linked and carry the `reviews` feature, or the request is refused as `feature_unsupported`. Replicated to every linked runtime. Writes this runtime's journal only.",
                input: ReviewRequestInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_review_request"), http: post("/api/v1/mesh/reviews"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_review_request,
            },
            capability! {
                id: "mesh.review.answer",
                kind: CapabilityKind::Command,
                title: "Answer a review request",
                description: "Answer a review request from any runtime with approved, changes_requested or commented, and a note. Replicated to every linked runtime. Writes this runtime's journal only.",
                input: ReviewAnswerInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_review_answer"), http: post("/api/v1/mesh/reviews/answer"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_review_answer,
            },
            capability! {
                id: "mesh.review.withdraw",
                kind: CapabilityKind::Command,
                title: "Withdraw a review request",
                description: "End a review request this runtime asked for — done, void or superseded — with an optional result note the request then carries. Any session of the asking runtime may (a reconnected asker is another session of it); no other runtime may, and it is never counted as an answer. An unanswered request also reads expired while its runtime has stopped beating. Replicated to every linked runtime. Writes this runtime's journal only.",
                input: ReviewWithdrawInput,
                output: Written,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_review_withdraw"), http: post("/api/v1/mesh/reviews/withdraw"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: mesh_review_withdraw,
            },
            capability! {
                id: "mesh.link.hello",
                kind: CapabilityKind::Command,
                title: "Open a link (the handshake)",
                description: "The link handshake another runtime posts: a signed hello carrying its runtime card, a fresh nonce and its protocol range. Refused, typed, when malformed, oversized, mis-signed, stale, replayed, of an unsupported protocol, of another repository, untrusted, or this runtime itself; otherwise answered with a signed welcome: this runtime's card, a link id, its marks and the echoed nonce. Changes this process's link table only.",
                input: Signed,
                output: LinkReply,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: None, http: post(HELLO_PATH), cli: None },
                tags: ["mesh", "cooperation", "link"],
                handler: mesh_link_hello,
            }
            // a hello is signed by the dialling node and verified before a link is admitted
            .authenticates_its_input(),
            capability! {
                id: "mesh.link.sync",
                kind: CapabilityKind::Command,
                title: "One sync round of a link",
                description: "The replication round a linked runtime posts every heartbeat, signed under its link id with a rising counter: its marks and the events this runtime lacks. Ingested — verified end to end, deduplicated, applied in stream order — and answered, signed, with this runtime's marks and the events the peer lacks. An unknown link or a restarted peer is told to say hello again.",
                input: Signed,
                output: LinkReply,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: None, http: post(SYNC_PATH), cli: None },
                tags: ["mesh", "cooperation", "link"],
                handler: mesh_link_sync,
            }
            // a sync round is signed under the link's key and verified before anything is ingested
            .authenticates_its_input(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; this is the assertion a
    /// refactor that dropped a projection would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "mesh");
        let expected: &[(&str, Option<&str>, &str)] = &[
            ("mesh.status", Some("majordomus_mesh"), "/api/v1/mesh"),
            (
                "mesh.nodes",
                Some("majordomus_mesh_nodes"),
                "/api/v1/mesh/nodes",
            ),
            (
                "mesh.enrol",
                Some("majordomus_mesh_enrol"),
                "/api/v1/mesh/enrol",
            ),
            (
                "mesh.identity",
                Some("majordomus_mesh_identity"),
                "/api/v1/mesh/identity",
            ),
            (
                "mesh.doctor",
                Some("majordomus_mesh_doctor"),
                "/api/v1/mesh/doctor",
            ),
            (
                "mesh.build_cache.env",
                Some("majordomus_mesh_build_cache_env"),
                "/api/v1/mesh/build-cache/env",
            ),
            (
                "mesh.build_cache.key",
                Some("majordomus_mesh_build_cache_key"),
                "/api/v1/mesh/build-cache/key",
            ),
            ("mesh.who", Some("majordomus_mesh_who"), "/api/v1/mesh/who"),
            // a command served nowhere over HTTP or MCP: it runs the caller's install command
            ("mesh.build_cache.ensure", None, ""),
            (
                "mesh.firewall",
                Some("majordomus_mesh_firewall"),
                "/api/v1/mesh/firewall",
            ),
            // the one command served nowhere over HTTP: it runs a privileged host tool
            ("mesh.firewall.apply", None, ""),
            ("mesh.build_cache.provision", None, ""),
            (
                "mesh.register",
                Some("majordomus_mesh_register"),
                REGISTER_PATH,
            ),
            (
                "mesh.cooperation",
                Some("majordomus_mesh_cooperation"),
                "/api/v1/mesh/cooperation",
            ),
            (
                "mesh.peers",
                Some("majordomus_mesh_peers"),
                "/api/v1/mesh/peers",
            ),
            (
                "mesh.peer",
                Some("majordomus_mesh_peer"),
                "/api/v1/mesh/peer",
            ),
            (
                "mesh.state",
                Some("majordomus_mesh_state"),
                "/api/v1/mesh/state",
            ),
            (
                "mesh.events",
                Some("majordomus_mesh_events"),
                "/api/v1/mesh/events",
            ),
            (
                "mesh.verify",
                Some("majordomus_mesh_verify"),
                "/api/v1/mesh/verify",
            ),
            (
                "mesh.session.open",
                Some("majordomus_mesh_session_open"),
                "/api/v1/mesh/sessions",
            ),
            (
                "mesh.session.close",
                Some("majordomus_mesh_session_close"),
                "/api/v1/mesh/sessions/close",
            ),
            (
                "mesh.claim",
                Some("majordomus_mesh_claim"),
                "/api/v1/mesh/claims",
            ),
            (
                "mesh.release",
                Some("majordomus_mesh_release"),
                "/api/v1/mesh/claims/release",
            ),
            (
                "mesh.beat",
                Some("majordomus_mesh_beat"),
                "/api/v1/mesh/beats",
            ),
            (
                "mesh.handover.publish",
                Some("majordomus_mesh_handover_publish"),
                "/api/v1/mesh/handovers",
            ),
            (
                "mesh.handover.consume",
                Some("majordomus_mesh_handover_consume"),
                "/api/v1/mesh/handovers/consume",
            ),
            (
                "mesh.review.request",
                Some("majordomus_mesh_review_request"),
                "/api/v1/mesh/reviews",
            ),
            (
                "mesh.review.answer",
                Some("majordomus_mesh_review_answer"),
                "/api/v1/mesh/reviews/answer",
            ),
            (
                "mesh.review.withdraw",
                Some("majordomus_mesh_review_withdraw"),
                "/api/v1/mesh/reviews/withdraw",
            ),
            ("mesh.link.hello", None, HELLO_PATH),
            ("mesh.link.sync", None, SYNC_PATH),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                *tool,
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                (!path.is_empty()).then_some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
    }

    /// Reads are queries; everything that changes the journal, the link table or the
    /// registry is a command, reachable only by POST — and none of them writes a tracked
    /// file, so none is a repository writer.
    #[test]
    fn what_changes_state_is_a_command_and_nothing_writes_the_repository() {
        let queries = [
            "mesh.status",
            "mesh.nodes",
            "mesh.identity",
            "mesh.doctor",
            "mesh.build_cache.env",
            "mesh.build_cache.key",
            "mesh.who",
            "mesh.firewall",
            "mesh.cooperation",
            "mesh.peers",
            "mesh.peer",
            "mesh.state",
            "mesh.events",
        ];
        for e in module().capabilities {
            let c = &e.capability;
            let is_query = queries.contains(&c.id.as_str());
            assert_eq!(
                c.kind == CapabilityKind::Command,
                !is_query,
                "{}: reads are queries, changes are commands",
                c.id
            );
            if !is_query {
                match c.exposure.http.as_ref() {
                    Some(h) => assert_eq!(
                        h.method.as_str(),
                        "POST",
                        "{} changes state and must be a POST",
                        c.id
                    ),
                    // a command served nowhere over HTTP runs a privileged host tool or a
                    // caller's own command, which nothing reachable over the network may do
                    None => assert!(
                        [
                            "mesh.firewall.apply",
                            "mesh.build_cache.ensure",
                            "mesh.build_cache.provision",
                        ]
                        .contains(&c.id.as_str()),
                        "{}: a command without an HTTP route must run a host or caller command",
                        c.id
                    ),
                }
            }
            // one tracked file is written, by one capability: the declaration's trust list,
            // by `mesh.enrol` (ADR 0137); nothing the mesh hears ever writes the repository
            assert_eq!(
                c.execution.effect == crate::capability::model::Effect::RepositoryMutation,
                c.id.as_str() == "mesh.enrol",
                "{}: only enrolment writes a tracked file",
                c.id
            );
        }
    }

    /// A cooperation runtime that dials nobody until it is started, which these never are.
    fn cooperation() -> std::sync::Arc<Cooperation> {
        crate::mesh::testing::cooperation()
    }

    /// A context over a synthetic repository, with a mesh declaration in its working tree
    /// when `declared`.
    fn context(declared: bool) -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mut index = repo.index().unwrap();
        if declared {
            index.objects.push(crate::model::Object {
                kind: crate::mesh::KIND.into(),
                identity: "majordomus".into(),
                uri: "majordomus://mesh-declaration/majordomus".into(),
                title: None,
                description: None,
                metadata: serde_json::json!({
                    "schema": "mesh/v1", "kind": "mesh-declaration", "id": "majordomus",
                    "enabled": true, "trust": { "policy": "deny_unknown", "allow": [] },
                }),
                body: String::new(),
                content: String::new(),
                media_type: "application/yaml",
                provenance: crate::model::Provenance {
                    path: ".ai/repo/mesh/majordomus.yaml".into(),
                    directory: ".ai/repo/mesh".into(),
                    source_class: "mesh-declaration".into(),
                    section: None,
                    bytes: 0,
                    member: None,
                },
            });
        }
        let registry = capability::CapabilityRegistry::builder()
            .with_modules(vec![module()])
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    #[test]
    fn an_ensure_of_npm_or_mix_without_its_install_command_is_refused_by_name() {
        let (_repo, ctx) = context(false);
        for kind in [
            crate::mesh::build_cache::key::Kind::NpmModules,
            crate::mesh::build_cache::key::Kind::MixDeps,
        ] {
            let refused = mesh_build_cache_ensure(
                &ctx,
                BuildCacheEnsureInput {
                    kind,
                    dir: None,
                    command: Vec::new(),
                },
            );
            let Err(CapabilityError::InvalidInput(why)) = refused else {
                panic!("{} without a command was not refused", kind.as_str());
            };
            assert_eq!(
                why,
                format!("{} needs the install command after `--`", kind.as_str())
            );
        }
    }

    #[test]
    fn the_env_asks_sccache_by_name_when_no_executable_is_given() {
        let (repo, ctx) = context(false);
        // the node file is absent, so no store is asked whatever sccache is installed here
        let answer = mesh_build_cache_env(
            &ctx,
            BuildCacheEnvInput {
                sccache: None,
                node_file: Some(repo.root().join("absent.json").display().to_string()),
                credentials: None,
            },
        )
        .unwrap();
        assert_eq!(
            answer.mode,
            crate::mesh::build_cache::env::Mode::LocalOnly,
            "{}",
            answer.reason
        );
        assert!(answer.variables.is_empty() && answer.store.is_none());
        if !answer.sccache.found {
            assert!(
                answer.reason.starts_with("sccache does not answer"),
                "the default executable is `sccache`: {}",
                answer.reason
            );
        }
    }

    #[test]
    fn retrust_asks_the_running_cooperation_with_the_declaration_in_force() {
        let (_repo, ctx) = context(true);
        assert!(
            matches!(declaration(&ctx), Some(Ok(_))),
            "the declaration parses"
        );
        assert!(retrust(&ctx).is_empty(), "no cooperation, nothing to end");
        ctx.mesh.attach_cooperation(cooperation());
        assert!(
            retrust(&ctx).is_empty(),
            "a runtime with no links ends none"
        );
        let (_repo, undeclared) = context(false);
        undeclared.mesh.attach_cooperation(cooperation());
        assert!(
            retrust(&undeclared).is_empty(),
            "no declaration, no trust to put in force"
        );
    }

    #[test]
    fn a_peer_asked_of_a_server_without_cooperation_is_not_found_with_the_reason() {
        let (_repo, ctx) = context(false);
        ctx.mesh.decline_cooperation("no endpoint to be dialed at");
        let detail = mesh_peer(
            &ctx,
            PeerInput {
                runtime: "anything".into(),
            },
        )
        .unwrap();
        assert!(!detail.found);
        assert_eq!(
            detail.reason.as_deref(),
            Some("no endpoint to be dialed at")
        );
        assert!(detail.machine.is_none() && detail.runtime.is_none() && detail.refused.is_empty());
    }

    #[test]
    fn a_peer_is_found_by_its_runtime_key_in_the_full_tree_and_another_is_not() {
        let (_repo, ctx) = context(false);
        let c = cooperation();
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&c));
        let own = c.runtime_key().to_string();
        let found = mesh_peer(
            &ctx,
            PeerInput {
                runtime: format!(" {own} "),
            },
        )
        .unwrap();
        assert!(found.found, "{found:?}");
        assert_eq!(found.runtime.map(|r| r.runtime), Some(own));
        let missing = mesh_peer(
            &ctx,
            PeerInput {
                runtime: "nobody".into(),
            },
        )
        .unwrap();
        assert!(!missing.found);
        assert_eq!(
            missing.reason.as_deref(),
            Some("no runtime nobody is linked or heard here")
        );
    }

    #[test]
    fn the_summary_tree_lists_active_sessions_counted_and_the_full_one_every_claim() {
        let c = cooperation();
        let s1 = SessionInfo::named("s1", "test");
        c.open_session(s1.clone()).unwrap();
        // the older claim's id sorts last, so the fold lists the newer one first and the
        // older one meets a newest it does not replace
        for (claim, path, issue) in [("z1", "apps", "#1"), ("a2", "docs", "#2")] {
            c.journal()
                .append_own(crate::mesh::journal::EventBody::ClaimAcquired {
                    claim: claim.into(),
                    session: s1.session.clone(),
                    scope: vec![path.into()],
                    intent: Some("a long intent ".repeat(20)),
                    mode: ClaimMode::Exclusive,
                    issue: Some(issue.into()),
                })
                .unwrap();
        }
        c.open_session(SessionInfo::named("s2", "test")).unwrap();
        c.close_session("s2").unwrap();
        c.request_review("s1", "feature/x".into(), vec![], None, None, None)
            .unwrap();
        let own = |tree: &PeerTree| -> RuntimeNode {
            tree.machines
                .iter()
                .flat_map(|m| m.runtimes.iter())
                .find(|r| r.this_runtime)
                .cloned()
                .expect("this runtime is in its own tree")
        };

        let summary = own(&peer_tree(&c, &Query::default()));
        assert_eq!(
            summary
                .newest_claim
                .as_ref()
                .and_then(|b| b.issue.as_deref()),
            Some("#2"),
            "the newest live claim"
        );
        assert_eq!(summary.sessions.len(), 1, "only the active session");
        let s = &summary.sessions[0];
        assert_eq!(s.state, SessionState::Active);
        assert!(s.claims.is_empty(), "counted, not listed");
        assert_eq!(s.claims_held, 2);
        assert_eq!(s.reviews.len(), 1, "the review the session asked for");

        let full = own(&peer_tree(
            &c,
            &Query {
                view: Some(View::Full),
                ..Query::default()
            },
        ));
        assert_eq!(full.sessions.len(), 2, "every session");
        let s1 = full
            .sessions
            .iter()
            .find(|s| s.state == SessionState::Active)
            .unwrap();
        assert_eq!(s1.claims.len(), 2, "every claim listed");
        assert_eq!(s1.claims_held, 2);
    }

    /// A session that beat says in the peer tree how long ago it made progress and what it
    /// said it was doing; one that never beat says neither (I3990).
    #[test]
    fn a_session_that_beat_shows_its_progress_and_phase_in_the_peer_tree() {
        let c = cooperation();
        for session in ["lander", "quiet"] {
            c.claim(
                &SessionInfo::named(session, "cli"),
                vec![format!("{session}/x")],
                None,
                ClaimMode::Exclusive,
                None,
            )
            .unwrap();
        }
        c.beat("lander", Some("waiting on CI run 42".into()), None)
            .unwrap();
        let tree = peer_tree(&c, &Query::default());
        let session = |name: &str| -> SessionNode {
            tree.machines
                .iter()
                .flat_map(|m| m.runtimes.iter())
                .flat_map(|r| r.sessions.iter())
                .find(|s| s.key.ends_with(&format!("/{name}")))
                .cloned()
                .expect("the session is in the tree")
        };
        let lander = session("lander");
        assert!(
            lander.progress_ms.is_some_and(|ms| ms < 60_000),
            "{lander:?}"
        );
        assert_eq!(lander.phase.as_deref(), Some("waiting on CI run 42"));
        let quiet = session("quiet");
        assert_eq!((quiet.progress_ms, quiet.phase), (None, None));
    }

    /// Over MCP a beat speaks for the calling session alone, so one client cannot keep
    /// another's claims excluding; the command line names the session it beats for, and a
    /// beat is only for a session this runtime opened (I3990).
    #[test]
    fn a_beat_over_mcp_speaks_for_its_own_session_alone() {
        let (_repo, ctx) = context(false);
        let beat = |ctx: &Context, session: Option<&str>| {
            mesh_beat(
                ctx,
                BeatInput {
                    session: session.map(Into::into),
                    phase: Some("waiting on CI run 42".into()),
                    pid: None,
                },
            )
        };
        assert!(
            matches!(beat(&ctx, Some("lander")), Err(CapabilityError::Refused(m))
                if m.starts_with("not_active:")),
            "no cooperation, nothing to beat"
        );
        let c = cooperation();
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&c));
        c.claim(
            &SessionInfo::named("lander", "cli"),
            vec!["integration".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .unwrap();
        assert!(
            matches!(beat(&ctx, None), Err(CapabilityError::InvalidInput(_))),
            "the command line names the session it beats for"
        );
        let written = beat(&ctx, Some("lander")).expect("the command line names its session");
        assert!(written.key.ends_with("/lander"), "{}", written.key);

        let board = crate::peers::PeerBoard::new();
        let peer = board.attach(crate::peers::Transport::Http);
        let mcp = ctx.for_caller(peer.clone());
        let refused = beat(&mcp, Some("lander")).unwrap_err();
        assert!(
            matches!(&refused, CapabilityError::Refused(m)
                if m.starts_with("not_own:") && m.contains("not for 'lander'")),
            "{refused}"
        );
        // its own session, named or not, passes the guard: this runtime never opened it
        let own = format!("board-{peer}");
        for session in [None, Some(own.as_str()), Some(" ")] {
            assert!(
                matches!(beat(&mcp, session), Err(CapabilityError::NotFound(_))),
                "{session:?}"
            );
        }
    }

    /// Every walk of `mesh.events` leaves the session beats out unless asked for them, and
    /// misses nothing for it: a walk by cursor passes a beat over without giving it a place
    /// on the page, a walk by `after` cuts its page first, and the newest page counts it in
    /// its `lamport` (I3990, I2733).
    #[test]
    fn every_walk_of_the_events_leaves_the_beats_out_and_misses_nothing() {
        let (_repo, ctx) = context(false);
        let c = cooperation();
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&c));
        let claim = |scope: &str| {
            c.claim(
                &SessionInfo::named("lander", "cli"),
                vec![scope.into()],
                None,
                ClaimMode::Exclusive,
                None,
            )
            .unwrap();
        };
        claim("integration");
        c.beat("lander", Some("waiting on CI run 42".into()), None)
            .unwrap();
        claim("release");
        let ask = |after: Option<u64>, cursor: Option<String>, newest, beats| {
            mesh_events(
                &ctx,
                MeshEventsInput {
                    after,
                    cursor,
                    limit: Some(1),
                    newest,
                    beats,
                },
            )
            .unwrap()
        };
        let kinds = |beats: Option<bool>| {
            let (mut kinds, mut cursor) = (Vec::new(), None);
            loop {
                let page = ask(None, cursor, None, beats);
                cursor = page.cursor.clone();
                if page.events.is_empty() {
                    return kinds;
                }
                assert_eq!(page.count, 1, "a page is full until the journal ends");
                kinds.extend(page.events.iter().map(|e| e.kind().to_string()));
            }
        };
        let hidden = kinds(None);
        let all = kinds(Some(true));
        assert!(all.iter().any(|k| k == "session_beat"), "{all:?}");
        assert!(!hidden.iter().any(|k| k == "session_beat"), "{hidden:?}");
        let without: Vec<_> = all.iter().filter(|k| *k != "session_beat").collect();
        assert_eq!(
            hidden.iter().collect::<Vec<_>>(),
            without,
            "nothing else is missed"
        );

        // by stamp: a page holding only a beat is empty and still moves `lamport` past it
        let beat_at = c
            .journal()
            .events_after(0, usize::MAX)
            .iter()
            .find(|e| e.kind() == "session_beat")
            .map(|e| e.lamport)
            .expect("the beat is in the journal");
        let page = ask(Some(beat_at - 1), None, None, None);
        assert_eq!((page.count, page.lamport, page.cursor), (0, beat_at, None));
        let page = ask(Some(beat_at - 1), None, None, Some(true));
        assert_eq!(page.events[0].kind(), "session_beat");

        // the newest page: the beat is not on it, and its stamp is the journal's newest
        let newest = ask(None, None, Some(true), None);
        assert_eq!(newest.count, 1);
        assert!(
            newest.cursor.is_some(),
            "the newest page names where to resume"
        );
        assert_eq!(newest.events[0].kind(), "claim_acquired");
    }
}
