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
use crate::intent_binding::BindingDrift;
use crate::mesh::cooperation::{
    Cooperation, CooperationError, CooperationStatus, MeshVerifyReport, PeerView, RefusedView,
    Written,
};
use crate::mesh::journal::{
    ClaimMode, ClaimPins, MeshEvent, SessionInfo, StreamId, StreamLiveness,
};
use crate::mesh::link::{LinkReply, RefusalCode, Signed, HELLO_PATH, SYNC_PATH};
use crate::mesh::rendezvous::REGISTER_PATH;
use crate::mesh::state::{ClaimState, ClaimView, CooperationState, HandoverView, SessionState};
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
    // The runtime's verdict is reported only where a shared server made one: in the
    // command line's process nothing activates the mesh, and that absence is not news.
    let runtime = ctx.mesh.decided().then(|| ctx.mesh.status());
    Ok(crate::mesh::doctor::doctor_at(
        declaration(ctx),
        Some(root),
        runtime.as_ref(),
    ))
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
    /// The digest of the cooperation state: equal digests, equal state — every field but a
    /// claim's pins, which an older executable does not read.
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
    crate::order::canonical(&mut machines);
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
    /// Every held claim that carries pins, with where those pins stand against what this
    /// checkout's own `intents.binding` answers now. Derived on every read, stored nowhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<MeshClaimPinStanding>,
}

/// Where a held claim's pins stand against this checkout's own reading of the work the
/// claiming task named: the question a worker asks about a peer it cannot see — is it still
/// working to the intent as it reads here?
///
/// `standing` is the word [`crate::intent_binding::BindingDrift`] gives a resumed worker —
/// `unchanged`, `evidence_moved` or `plan_changed` — or `unknown` when the binding could not
/// be asked or reached no intent here, with the reason. Unknown is never read as unchanged:
/// a checkout that does not hold the issue the peer named has learned nothing about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshClaimPinStanding {
    /// The claim's key, `<stream>/<claim>`.
    pub claim: String,
    /// The task whose record the pins came from.
    pub task: String,
    /// The issue that task named; empty when it named none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issue: String,
    /// The intent that task named; empty when it named none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub intent: String,
    /// `unchanged`, `evidence_moved`, `plan_changed` or `unknown`.
    pub standing: String,
    /// Why the standing is `unknown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
}

/// What this checkout's binding answers for the work a claim's pins name: its two pins, or
/// why there are none.
type PinsAnswer = Result<(String, String), String>;

/// A way to ask [`PinsAnswer`] for a claim's pins.
type AskBinding<'a> = dyn Fn(&ClaimPins) -> PinsAnswer + 'a;

/// The standing of every held, pinned claim, given a way to ask what the named work is
/// bound to here: `ask` answers the two pins of the current binding, or why it has none.
///
/// A pure function of the folded claims and the answers, so that the comparison is tested
/// without a server, and the one comparison is [`BindingDrift::between`] — the same a
/// handover is resolved with, so a claim and a handover cannot disagree about what "moved"
/// means.
fn pin_standings(claims: &[ClaimView], ask: &AskBinding) -> Vec<MeshClaimPinStanding> {
    claims
        .iter()
        .filter(|c| c.state == ClaimState::Held)
        .filter_map(|c| c.pins.as_ref().map(|p| (c, p)))
        .map(|(c, p)| {
            let (standing, because) = ask(p)
                .and_then(|(plan, evidence)| {
                    Some(plan)
                        .filter(|plan| !plan.is_empty())
                        .map(|plan| (plan, evidence))
                        .ok_or_else(|| "the work it names reaches no intent here".to_string())
                })
                .map_or_else(
                    |why| ("unknown".to_string(), Some(why)),
                    |(plan, evidence)| {
                        let drift = BindingDrift::between(
                            &p.plan_revision,
                            &p.evidence_standing,
                            &plan,
                            &evidence,
                        );
                        (drift.as_str().to_string(), None)
                    },
                );
            MeshClaimPinStanding {
                claim: c.key.clone(),
                task: p.task.clone(),
                issue: p.issue.clone(),
                intent: p.intent.clone(),
                standing,
                because,
            }
        })
        .collect()
}

/// What the work a claim's pins name is bound to in this checkout, as `intents.binding`
/// answers it: the two pins, or the reason there are none. Asked through the registry, the
/// one way a handler calls another, so the mesh holds no second derivation of a binding.
fn binding_pins(ctx: &Context, pins: &ClaimPins) -> PinsAnswer {
    let named = |v: &str| Some(v).filter(|v| !v.is_empty()).map(str::to_string);
    let field = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    ctx.execute(
        "intents.binding",
        serde_json::json!({ "issue": named(&pins.issue), "intent": named(&pins.intent) }),
    )
    .map_err(unasked)
    .and_then(|v| {
        let plan = field(&v, "plan_revision");
        let refusal = v["refusals"][0]["message"].as_str().map(str::to_string);
        match (plan.is_empty(), refusal) {
            (true, Some(why)) => Err(format!("the binding here refuses it: {why}")),
            _ => Ok((plan, field(&v, "evidence_standing"))),
        }
    })
}

/// The reason a binding could not be asked at all, as opposed to one it refused. Every input
/// the mesh sends today is answered or refused, so this is the registry failing; it is said
/// rather than read as no pins.
fn unasked(e: CapabilityError) -> String {
    format!("the binding could not be asked: {e}")
}

/// Ask once per distinct work named in one read: ten claims for the same issue are one
/// derivation, not ten (rule project.derived-once).
fn memoised<'a>(ask: &'a AskBinding) -> impl Fn(&ClaimPins) -> PinsAnswer + 'a {
    let seen: std::cell::RefCell<BTreeMap<(String, String), PinsAnswer>> =
        std::cell::RefCell::new(BTreeMap::new());
    move |p: &ClaimPins| {
        let key = (p.issue.clone(), p.intent.clone());
        if let Some(hit) = seen.borrow().get(&key) {
            return hit.clone();
        }
        let answer = ask(p);
        seen.borrow_mut().insert(key, answer.clone());
        answer
    }
}

fn mesh_state(ctx: &Context, _: Empty) -> Result<MeshStateAnswer, CapabilityError> {
    Ok(match ctx.mesh.cooperation() {
        Some(c) => {
            let state = c.state();
            let ask = |p: &ClaimPins| binding_pins(ctx, p);
            let once = memoised(&ask);
            MeshStateAnswer {
                active: true,
                reason: None,
                pins: pin_standings(&state.claims, &once),
                state: Some(state),
                streams: c
                    .streams()
                    .into_iter()
                    .map(|(stream, liveness, last_beat_ms)| StreamView {
                        stream,
                        liveness,
                        last_beat_ms,
                    })
                    .collect(),
            }
        }
        None => MeshStateAnswer {
            active: false,
            reason: ctx.mesh.cooperation_status().reason,
            state: None,
            streams: Vec::new(),
            pins: Vec::new(),
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
    let pins = task_pins(
        std::path::Path::new(&ctx.index.repository.root),
        info.task.as_deref(),
    );
    c.claim_pinned(
        &info,
        input.scope,
        input.intent,
        input.mode.unwrap_or_default(),
        input.issue,
        pins,
    )
    .map_err(map_error)
}

/// What the claiming task was bound to when it started, read from this checkout's task
/// record: the pins `start` wrote there (ADR 0111), carried and never recomputed.
///
/// None — and the claim travels without pins — when the checkout has no task record, when
/// that task is no longer `active` (`finish` rewrites the record in place and only the next
/// `start` archives it, so a finished task's record is still there), when its binding was not
/// `bound` or it named neither an issue nor an intent (an advisory start by paths records pins
/// that name nothing, and an exempt task has none), or when the claim names a task other than
/// the active one: this checkout holds one task record, and pins read from it would then
/// describe somebody else's work. A claim that names no task is taken for the checkout's task,
/// whichever client makes it: the pins say what the checkout is bound to, not who claimed.
fn task_pins(root: &std::path::Path, named: Option<&str>) -> Option<ClaimPins> {
    super::continuity::read_task(&crate::gates::task_path(root))
        .filter(|t| named.is_none_or(|n| n == t.id))
        .filter(|t| t.outcome == "active" && t.binding == "bound")
        .filter(|t| !(t.issue.is_empty() && t.intent.is_empty()))
        .filter(|t| !t.plan_revision.is_empty())
        .map(|t| ClaimPins {
            task: t.id,
            issue: t.issue,
            intent: t.intent,
            plan_revision: t.plan_revision,
            evidence_standing: t.evidence_standing,
        })
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
                description: "Machine → runtime → session → claim: every runtime this one is linked to or has heard through the journal, grouped by machine, local first; each runtime with its liveness, the milliseconds since its beat rose and its link, each session with what it said and its claims. Remote and local runtimes share one shape, and the state digest closes the answer: equal digests, equal state (a claim's pins aside, which runtimes of different ages need not both read).",
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
                description: "The journal folded: every session, every claim with its standing (held, released, expired, conflicted with its winner), the advisory overlaps, every handover with who consumed it, every review request with its answers, and the digest two runtimes compare (a claim's pins left out). Plus every stream's liveness on this runtime's clock, and `pins`: every held claim carrying the pins of the task it was taken under, with where they stand against this checkout's own `intents.binding` for the work they name — `unchanged`, `evidence_moved`, `plan_changed`, or `unknown` with the reason when the binding cannot be asked, refuses it, or reaches no intent here. Derived on every read, one binding per distinct work named, stored nowhere.",
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
                description: "Claim repository paths for a session. An exclusive claim that meets a live exclusive claim of another session — on this runtime or any runtime this one has heard — is refused as `claim_conflict` with the claims it meets; an advisory claim is recorded and its overlaps reported. A claim lives while its session is open and its runtime beats: a crashed holder's claim expires everywhere on its own. When this checkout's task is active and started bound to a named issue or intent, the claim carries that task's id, names and two pins as its record holds them (ADR 0111); they refuse nothing. Writes this runtime's journal only.",
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

    /// The binding a claim's pins are compared with is the one `intents.binding` answers in
    /// this checkout: its pins for work it holds, and the refusal, as the reason, for work it
    /// does not hold.
    #[test]
    fn the_pins_are_compared_with_what_this_checkouts_binding_answers() {
        let repo = crate::intent_binding::tests::planned();
        let ctx = repo.context().unwrap();
        let pins = |issue: &str| ClaimPins {
            task: "t".into(),
            issue: issue.into(),
            plan_revision: "p".into(),
            ..ClaimPins::default()
        };
        let (plan, _) = binding_pins(&ctx, &pins("I0001")).expect("a binding with its pins");
        assert_eq!(plan.len(), 64, "a SHA-256 pin: {plan}");
        let why = binding_pins(&ctx, &pins("I9999")).unwrap_err();
        assert!(why.starts_with("the binding here refuses it: "), "{why}");
    }

    /// A binding this checkout cannot answer — the issue is not in its plan — is said as the
    /// reason there are no pins here, never read as pins that match.
    #[test]
    fn a_binding_that_cannot_be_answered_here_is_said_and_not_guessed() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let pins = ClaimPins {
            task: "t-1".into(),
            issue: "I9999".into(),
            intent: String::new(),
            plan_revision: "p".into(),
            evidence_standing: "e".into(),
        };
        let answer = binding_pins(&ctx, &pins);
        assert!(answer.is_err(), "{answer:?}");
        assert!(answer.unwrap_err().contains("I9999"));
        // naming nothing is refused too, with the binding's own reason
        let nothing = ClaimPins {
            issue: String::new(),
            ..pins
        };
        assert!(binding_pins(&ctx, &nothing)
            .unwrap_err()
            .contains("nothing was named"));
        // and a registry that cannot be asked is said as such
        assert_eq!(
            unasked(CapabilityError::NotFound("intents.binding".into())),
            format!(
                "the binding could not be asked: {}",
                CapabilityError::NotFound("intents.binding".into())
            )
        );
    }

    /// One read asks the binding once per distinct work named: ten claims for one issue are
    /// one derivation, and the answer — pins or the reason there are none — is the same each time.
    #[test]
    fn one_read_asks_the_binding_once_per_distinct_work() {
        let asked = std::cell::Cell::new(0);
        let ask = |p: &ClaimPins| {
            asked.set(asked.get() + 1);
            match p.issue.as_str() {
                "I1" => Ok(("plan".to_string(), "ev".to_string())),
                _ => Err("not here".to_string()),
            }
        };
        let once = memoised(&ask);
        let pins = |issue: &str| ClaimPins {
            task: "t".into(),
            issue: issue.into(),
            plan_revision: "p".into(),
            ..ClaimPins::default()
        };
        for _ in 0..3 {
            assert_eq!(once(&pins("I1")), Ok(("plan".into(), "ev".into())));
            assert_eq!(once(&pins("I2")), Err("not here".into()));
        }
        assert_eq!(asked.get(), 2, "two works named, two derivations");
    }

    /// A pinned claim is compared with what this checkout answers for the same work, by the
    /// comparison a handover is resolved with; a claim that is not held or carries no pins is
    /// not listed, and a binding that cannot be asked or reaches no intent is unknown — never
    /// unchanged.
    #[test]
    fn a_pinned_claim_stands_where_the_local_binding_puts_it_and_unknown_is_not_unchanged() {
        let view = |key: &str, issue: &str, state: ClaimState, pinned: bool| -> ClaimView {
            serde_json::from_value(serde_json::json!({
                "key": key, "stream": format!("{}-{}-{}", "0".repeat(32), "0".repeat(16), "0".repeat(16)),
                "runtime": "r", "session": "s", "scope": ["apps"], "mode": "advisory",
                "acquired_lamport": 1, "acquired_seq": 1,
                "state": serde_json::to_value(&state).unwrap(),
                "pins": pinned.then(|| serde_json::json!({
                    "task": "t-1", "issue": issue,
                    "plan_revision": "plan-1", "evidence_standing": "ev-1",
                })),
            }))
            .expect("a claim view")
        };
        let claims = [
            view("a/same", "I1", ClaimState::Held, true),
            view("a/evidence", "I2", ClaimState::Held, true),
            view("a/plan", "I3", ClaimState::Held, true),
            view("a/nowhere", "I4", ClaimState::Held, true),
            view("a/unasked", "I5", ClaimState::Held, true),
            view("a/released", "I1", ClaimState::Released, true),
            view("a/unpinned", "I1", ClaimState::Held, false),
        ];
        let ask = |p: &ClaimPins| match p.issue.as_str() {
            "I1" => Ok(("plan-1".to_string(), "ev-1".to_string())),
            "I2" => Ok(("plan-1".to_string(), "ev-2".to_string())),
            "I3" => Ok(("plan-2".to_string(), "ev-2".to_string())),
            "I4" => Ok((String::new(), String::new())),
            _ => Err("the binding could not be asked: no index".to_string()),
        };
        let got = pin_standings(&claims, &ask);
        let words: Vec<(&str, &str)> = got
            .iter()
            .map(|s| (s.claim.as_str(), s.standing.as_str()))
            .collect();
        assert_eq!(
            words,
            [
                ("a/same", "unchanged"),
                ("a/evidence", "evidence_moved"),
                ("a/plan", "plan_changed"),
                ("a/nowhere", "unknown"),
                ("a/unasked", "unknown"),
            ]
        );
        assert_eq!(got[0].because, None);
        assert_eq!(
            got[3].because.as_deref(),
            Some("the work it names reaches no intent here")
        );
        assert!(got[4]
            .because
            .as_deref()
            .unwrap()
            .contains("could not be asked"));
        assert_eq!((got[0].task.as_str(), got[0].issue.as_str()), ("t-1", "I1"));
    }

    /// The pins a claim carries are the task record's own, read from the one record this
    /// checkout holds — and from nowhere when that record is absent, pins nothing, or is
    /// another task's than the one the claim names.
    #[test]
    fn a_claim_carries_the_pins_of_the_checkouts_task_and_of_no_other() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(task_pins(root, None), None, "no task, no pins");

        let record = crate::gates::task_path(root);
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(&record, "id: t-1\ntask: unbound work\n").unwrap();
        assert_eq!(
            task_pins(root, None),
            None,
            "a task that named nothing pins nothing"
        );

        let write = |extra: &str| {
            std::fs::write(
                &record,
                format!(
                    "id: t-1\ntask: the work\nplan_revision: 9f2c\nevidence_standing: 1ab0\n{extra}"
                ),
            )
            .unwrap()
        };
        // pins that name nothing, or a binding that did not stand bound, are not carried: an
        // advisory start by paths, an exemption, a refused binding
        for extra in [
            "outcome: active\nbinding: bound\n",
            "outcome: active\nbinding: exempt\nexemption: emergency\n",
            "outcome: active\nbinding: refused\nissue: I2071\n",
            "outcome: completed\nbinding: bound\nissue: I2071\n",
            "outcome: handed_over\nbinding: bound\nissue: I2071\n",
        ] {
            write(extra);
            assert_eq!(task_pins(root, None), None, "{extra}");
        }
        write("outcome: active\nbinding: bound\nissue: I2071\n");
        let pins = task_pins(root, None).expect("the active task's pins");
        assert_eq!(
            (
                pins.task.as_str(),
                pins.issue.as_str(),
                pins.intent.as_str()
            ),
            ("t-1", "I2071", "")
        );
        assert_eq!(
            (pins.plan_revision.as_str(), pins.evidence_standing.as_str()),
            ("9f2c", "1ab0")
        );
        assert!(
            pins.validate().is_ok(),
            "what is read is what may be signed"
        );
        assert_eq!(
            task_pins(root, Some("t-1")),
            Some(pins),
            "named, and it is this one"
        );
        assert_eq!(
            task_pins(root, Some("t-2")),
            None,
            "a claim for another task does not borrow this task's pins"
        );
    }

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
