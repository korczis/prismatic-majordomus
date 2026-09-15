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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::mesh::cooperation::{
    Cooperation, CooperationError, CooperationStatus, MeshVerifyReport, PeerView, RefusedView,
    Written,
};
use crate::mesh::journal::{ClaimMode, MeshEvent, SessionInfo, StreamId, StreamLiveness};
use crate::mesh::link::{LinkReply, RefusalCode, Signed, HELLO_PATH, SYNC_PATH};
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

// ---------------------------------------------------------------- mesh.doctor

fn mesh_doctor(ctx: &Context, _: Empty) -> Result<MeshDoctorReport, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    Ok(crate::mesh::doctor::doctor_at(declaration(ctx), Some(root)))
}

/// The repository's mesh declaration, as the index discovered it: `None` when no object
/// of the kind exists, the parse verdict when one does.
pub fn declaration(ctx: &Context) -> Option<Result<MeshConfig, crate::mesh::MeshError>> {
    ctx.index
        .objects
        .iter()
        .find(|o| o.kind == crate::mesh::KIND)
        .map(MeshConfig::parse)
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
    /// The claims it holds or held.
    pub claims: Vec<ClaimView>,
    /// The review requests it opened, by key.
    pub reviews: Vec<String>,
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
    /// Its sessions.
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

/// The answer of `mesh.peers`: machine → runtime → session → claim, every level from the
/// one journal and the one link table.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerTree {
    /// Whether cooperation runs.
    pub active: bool,
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

/// Build the peer tree from a running cooperation.
pub fn peer_tree(c: &Cooperation) -> PeerTree {
    let status = c.status();
    let state = c.state();
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
    for session in &state.sessions {
        let claims = state
            .claims
            .iter()
            .filter(|cl| cl.session == session.key)
            .cloned()
            .collect();
        let reviews = state
            .reviews
            .iter()
            .filter(|r| r.session == session.key)
            .map(|r| r.key.clone())
            .collect();
        if let Some(runtime) = runtimes.get_mut(&session.runtime) {
            runtime.sessions.push(SessionNode {
                key: session.key.clone(),
                info: session.info.clone(),
                state: session.state,
                claims,
                reviews,
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
    machines.sort_by_key(|m| !m.local);
    PeerTree {
        active: status.active,
        reason: status.reason,
        runtime: status.runtime,
        repository: status.repository.map(|r| r.id),
        machines,
        refused: status.refused,
        digest: Some(state.digest),
    }
}

fn mesh_peers(ctx: &Context, _: Empty) -> Result<PeerTree, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => peer_tree(&c),
        None => PeerTree {
            active: false,
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
    let tree = peer_tree(&c);
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
    /// Why not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The folded state, when cooperation runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<CooperationState>,
    /// Every stream's liveness.
    pub streams: Vec<StreamView>,
}

fn mesh_state(ctx: &Context, _: Empty) -> Result<MeshStateAnswer, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => MeshStateAnswer {
            active: true,
            reason: None,
            state: Some(c.state()),
            streams: c
                .streams()
                .into_iter()
                .map(|(stream, liveness, last_beat_ms)| StreamView {
                    stream,
                    liveness,
                    last_beat_ms,
                })
                .collect(),
        },
        None => MeshStateAnswer {
            active: false,
            reason: ctx.mesh.cooperation_status().reason,
            state: None,
            streams: Vec::new(),
        },
    })
}

// ---------------------------------------------------------------- mesh.events

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.events`.
pub struct MeshEventsInput {
    /// Only events whose Lamport stamp is above this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// At most this many (default 100, at most 1000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

impl BenchmarkCases for MeshEventsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            MeshEventsInput {
                after: None,
                limit: Some(100),
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
    /// The highest Lamport stamp the journal has seen: the `after` of the next page.
    pub lamport: u64,
    /// The events, in Lamport order.
    pub events: Vec<MeshEvent>,
}

fn mesh_events(ctx: &Context, input: MeshEventsInput) -> Result<EventList, CapabilityError> {
    let Some(c) = ctx.mesh.cooperation() else {
        return Ok(EventList {
            active: false,
            count: 0,
            lamport: 0,
            events: Vec::new(),
        });
    };
    let limit = input.limit.unwrap_or(100).clamp(1, 1000) as usize;
    let events = c.journal().events_after(input.after.unwrap_or(0), limit);
    Ok(EventList {
        active: true,
        count: events.len(),
        lamport: c.journal().tallies().lamport,
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
            },
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
            },
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
                description: "Every prerequisite proved on this machine alone: the declaration parses, the identity loads, the repository has a mesh identity, the advertised endpoints are reachable from beyond this machine, a UDP socket binds, the multicast group joins, broadcast enables, and the discovery and link protocols sign and verify end to end in memory. Each failed check names its impact and its remedy. Deterministic, no second node required.",
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
            },
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
            },
            capability! {
                id: "mesh.peers",
                title: "Who cooperates, machine by machine",
                description: "Machine → runtime → session → claim: every runtime this one is linked to or has heard through the journal, grouped by machine, local first; each runtime with its liveness, the milliseconds since its beat rose and its link, each session with what it said and its claims. Remote and local runtimes share one shape, and the state digest closes the answer: equal digests, equal state.",
                input: Empty,
                output: PeerTree,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_peers"), http: get("/api/v1/mesh/peers"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_peers,
            },
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
            },
            capability! {
                id: "mesh.state",
                title: "The state every linked runtime converges on",
                description: "The journal folded: every session, every claim with its standing (held, released, expired, conflicted with its winner), the advisory overlaps, every handover with who consumed it, every review request with its answers, and the digest two runtimes compare. Plus every stream's liveness on this runtime's clock.",
                input: Empty,
                output: MeshStateAnswer,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_state"), http: get("/api/v1/mesh/state"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_state,
            },
            capability! {
                id: "mesh.events",
                title: "The cooperation journal",
                description: "The journal's events above a Lamport stamp, in Lamport order, at most a page: each with its stream, sequence, stamp, repository, signing key, kind and body. The answer's lamport is the next page's `after`.",
                input: MeshEventsInput,
                output: EventList,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_events"), http: get("/api/v1/mesh/events"), cli: None },
                tags: ["mesh", "coordination", "cooperation"],
                handler: mesh_events,
            },
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
                description: "Claim repository paths for a session. An exclusive claim that meets a live exclusive claim of another session — on this runtime or any runtime this one has heard — is refused as `claim_conflict` with the claims it meets; an advisory claim is recorded and its overlaps reported. A claim lives while its session is open and its runtime beats: a crashed holder's claim expires everywhere on its own. Writes this runtime's journal only.",
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
            },
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
            },
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
                Some(*path),
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
                assert_eq!(
                    c.exposure.http.as_ref().map(|h| h.method.as_str()),
                    Some("POST"),
                    "{} changes state and must be a POST",
                    c.id
                );
            }
            assert_ne!(
                c.execution.effect,
                crate::capability::model::Effect::RepositoryMutation,
                "{}: the mesh writes no tracked file",
                c.id
            );
        }
    }
}
