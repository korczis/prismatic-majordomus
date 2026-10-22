//! The `blackboard` module: who works on what in this repository, as one projection (ADR 0176).
//!
//! Three records already know it, each about its own half: the board of every checkout
//! (`peers.list`, gathered per ADR 0044) knows which clients are attached and what they
//! announced; the mesh journal (ADR 0067) knows the sessions, claims, reviews and handovers of
//! every linked machine, this repository's board peers among them as the `board-pN` sessions
//! their own server projected; the monitor (ADR 0133) knows what happened, folded, with a
//! cursor. This module joins them once, so that no surface joins them again. It writes nothing
//! and keeps nothing.
//!
//! The join is the whole of what is new here, and it is deliberately narrow: a board peer and
//! a mesh session are one worker only when an explicit identifier says so (see [`join`]). A
//! peer id such as `p1` is a position on one board, handed out again on every reconnect and by
//! every other board, so it is never an identity on its own.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::SystemTime;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::mesh::journal::{ClaimMode, StreamLiveness};
use crate::mesh::state::{ClaimState, CooperationState, ReviewState, SessionState, SessionView};
use crate::monitor::{MonitorPage, MonitorSignal, MonitorSourceHealth, MonitorSourceState};
use crate::peers::{claims_meet, rfc3339, OverlapPath, Peer};
use crate::repository;
use crate::{capability, module};

use super::monitor::MonitorInput;
use super::peers::{BoardView, PeerList, PeerListInput};
use super::server::Checkouts;
use super::{get, mcp};

/// How many signals the answer carries when the caller does not say.
pub(crate) const DEFAULT_SIGNALS: u64 = 30;

/// The most signals one answer carries.
pub(crate) const MAX_SIGNALS: u64 = 200;

/// The most conflicts one answer lists; `conflicts_total` counts them all.
pub(crate) const MAX_CONFLICTS: usize = 100;

/// The most open reviews and untaken handovers one answer lists, each.
pub(crate) const MAX_RECORDS: usize = 50;

/// The monitor sources asked when the caller does not say: the ones this machine answers.
/// GitHub is asked only when the caller names it — it is not who is here, and it is the network.
const DEFAULT_SOURCES: &str = "mesh,executor";

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `blackboard.snapshot`: how wide the board is, who reads it, and how much of
/// what happened it carries.
pub struct BlackboardInput {
    /// Which checkouts' boards it covers: `repository` (every one, the default) or `this`.
    #[serde(default)]
    pub checkouts: Checkouts,
    /// Comma-separated names of the reader — a worker (`1b`), a role, a login — so that the signals addressed to it are marked `for_you`. An MCP caller is also its own board session.
    #[serde(default, rename = "for", skip_serializing_if = "Option::is_none")]
    pub reader: Option<String>,
    /// How many of the newest signals of the last hour it carries (default 30, at most 200); 0 reads no signals at all, for a follower that reads them by its own cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<u64>,
    /// A comma-separated subset of the monitor's sources, `mesh`, `executor` and `github` (default: `mesh,executor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<String>,
}

impl BenchmarkCases for BlackboardInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        cases()
    }
}

/// The benchmark cases: every parameter has an example, no other checkout's server is asked
/// and GitHub — the network — never is, so a benchmark measures this machine.
fn cases() -> Vec<NamedCase<BlackboardInput>> {
    vec![
        NamedCase::new(
            "this-checkout",
            BlackboardInput {
                checkouts: Checkouts::This,
                reader: None,
                signals: None,
                sources: Some("mesh,executor".into()),
            },
        ),
        NamedCase::new(
            "addressed",
            BlackboardInput {
                checkouts: Checkouts::This,
                reader: Some("1b".into()),
                signals: Some(10),
                sources: Some("mesh".into()),
            },
        ),
    ]
}

// ---------------------------------------------------------------- the answer

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Where a worker of the blackboard was found.
pub enum BlackboardOrigin {
    /// On a checkout's board: an MCP client attached to that checkout's server.
    Board,
    /// In the mesh journal only: a session no board of this repository holds — another
    /// machine's, a task's, or a client's own.
    Mesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Whether a worker is here now.
pub enum BlackboardPresence {
    /// Attached to its board now.
    Attached,
    /// Gone from its board, kept because it still holds an announcement.
    Departed,
    /// An open mesh session whose runtime still beats.
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One claim a worker holds.
pub struct BlackboardClaim {
    /// The claim's name, when its worker holds several.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it is for, in the worker's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The repository paths it names.
    pub scope: Vec<String>,
    /// `advisory` (an announcement: overlaps are reported) or `exclusive` (a second exclusive
    /// claim meeting it is refused while it lives). A board announcement is advisory.
    pub mode: ClaimMode,
    /// The issue it is for, when the claim names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The mesh claim's key, when it is a journal record; a board announcement has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// Whether the mesh found it conflicting with another live claim.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub conflicted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One worker of the repository: a board peer, a mesh session, or one of each that an
/// explicit identifier joins.
pub struct BlackboardWorker {
    /// Stable for as long as the worker is here: `board:<checkout>/<peer>` or
    /// `mesh:<stream>/<session>`. Never a bare peer id.
    pub key: String,
    /// Where it was found.
    pub origin: BlackboardOrigin,
    /// Whether it is here now.
    pub presence: BlackboardPresence,
    /// The client's own name, from its MCP initialize or its mesh session.
    pub client: String,
    /// The client's display title or the worker it says it is, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The client's version, from its initialize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// What it said it is doing; absent means it announced nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The checkout's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    /// The checkout's path on its machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    /// The branch it works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Its position on its board (`p3`): meaningful on that board only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_peer: Option<String>,
    /// Its mesh session's key, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The machine's display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// The runtime (`<node>-<runtime>`) its mesh session belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The task its mesh session names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue its mesh session names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// When it attached to its board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<String>,
    /// Seconds since its board last heard from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_seconds_ago: Option<u64>,
    /// Milliseconds since its runtime last beat. A beat is liveness, not work done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat_ms: Option<u64>,
    /// Its claims, board announcements and live mesh claims alike.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<BlackboardClaim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// How sure a conflict is.
pub enum BlackboardCertainty {
    /// Two claims meet and at least one of them is advisory: two workers about to touch the
    /// same paths, told and not stopped.
    Potential,
    /// Two exclusive claims meet, or the mesh marked one conflicted: what an exclusive claim
    /// exists to prevent has happened.
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One side of a conflict: a worker and which of its claims.
pub struct BlackboardClaimRef {
    /// The worker's key.
    pub worker: String,
    /// The claim's name, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<String>,
    /// The mesh claim's key, when it is a journal record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Two claims of two different workers whose scopes meet.
pub struct BlackboardConflict {
    /// Potential or confirmed.
    pub certainty: BlackboardCertainty,
    /// The claim that came first in the answer's order.
    pub first: BlackboardClaimRef,
    /// The claim that meets it.
    pub second: BlackboardClaimRef,
    /// The paths that meet, `yours` the first's and `theirs` the second's.
    pub paths: Vec<OverlapPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A review request nobody has answered.
pub struct BlackboardReview {
    /// The review's key in the journal.
    pub key: String,
    /// The requesting worker's key, or its session key when no worker holds it.
    pub from: String,
    /// What it asks about.
    pub subject: String,
    /// Whom it asks, as the request names it; absent means anyone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// The issue it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A handover published to the mesh that nobody has taken. Its body is not repeated here:
/// `mesh.state` holds it. The journal names no recipient for a handover — whoever takes it
/// is its addressee — so none is inferred here.
pub struct BlackboardHandover {
    /// The handover's id.
    pub id: String,
    /// The runtime that published it.
    pub runtime: String,
    /// Its name, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The branch it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The issue it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The task it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// When it was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// What a relation between two workers is.
pub enum BlackboardRelationKind {
    /// A board peer and a mesh session that may be one worker.
    SameWorker,
    /// One worker asked another for a review.
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// What a relation rests on.
pub enum BlackboardEvidence {
    /// An explicit identifier or a journal record joins the two ends.
    Verified,
    /// The ends could be joined and nothing proves it: more than one record fits.
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A typed relation between two workers, or a worker and the addressee a record names.
pub struct BlackboardRelation {
    /// What it is.
    pub kind: BlackboardRelationKind,
    /// What it rests on.
    pub evidence: BlackboardEvidence,
    /// A worker's key.
    pub from: String,
    /// A worker's key, or the addressee as the record names it when no worker is it.
    pub to: String,
    /// The record it rests on: a session or a review key.
    pub record: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// How one source of the blackboard was read.
pub enum BlackboardSourceState {
    /// Read whole.
    Ok,
    /// Read, and some of what it covers could not be asked.
    Partial,
    /// Not read; the reason says why.
    Absent,
    /// Not asked for.
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One source of the blackboard and how it was read.
pub struct BlackboardSource {
    /// `board`, `mesh`, or a monitor source as `monitor.<name>`.
    pub source: String,
    /// How it was read.
    pub state: BlackboardSourceState,
    /// Why it is not `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The answer of `blackboard.snapshot`: who works on what in this repository, on every board
/// and every linked machine, with what meets and what waits, read once from the records that
/// know it.
pub struct Blackboard {
    /// When it was read (RFC 3339).
    pub at: String,
    /// The identity of the checkout whose server answered.
    pub checkout: String,
    /// That checkout's HEAD, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Whether every board was read and the mesh, when the repository declares one, could be
    /// asked. `false` names its reason in `sources`, so that "nobody is here" is never what
    /// "I could not ask" reads as. The monitor's sources are reported in `sources` with their
    /// state and do not decide it: an empty executor trail or an absent `gh` is no board that
    /// could not be read.
    pub complete: bool,
    /// Each source and how it was read.
    pub sources: Vec<BlackboardSource>,
    /// Each checkout's board, reached or not.
    pub boards: Vec<BoardView>,
    /// How many workers are here now: attached to a board or in an active mesh session.
    pub present: usize,
    /// Every worker, this checkout's board first, then the other boards, then the mesh.
    pub workers: Vec<BlackboardWorker>,
    /// The claims that meet, at most a hundred, confirmed first.
    pub conflicts: Vec<BlackboardConflict>,
    /// How many conflicts there are in all.
    pub conflicts_total: usize,
    /// The review requests nobody has answered, at most fifty.
    pub reviews: Vec<BlackboardReview>,
    /// The handovers nobody has taken, at most fifty.
    pub handovers: Vec<BlackboardHandover>,
    /// The typed relations between workers.
    pub relations: Vec<BlackboardRelation>,
    /// The newest signals of the last hour, oldest first.
    pub signals: Vec<MonitorSignal>,
    /// How many of them are for the reader.
    pub for_you: usize,
    /// The monitor's cursor after these signals: where a follower resumes.
    pub cursor: String,
}

// ---------------------------------------------------------------- the projection

/// Why the mesh's state is not in the answer.
pub(crate) enum MeshAbsence {
    /// The repository declares no mesh, or declares it off: there is nothing to ask, and the
    /// answer is complete about what the repository has.
    Undeclared(String),
    /// The mesh is declared and could not be asked: the answer is incomplete.
    Unreachable(String),
}

/// What the projection reads: the three answers, and what the mesh knows of machines and
/// runtime beats.
pub(crate) struct Reads<'a> {
    pub board: &'a PeerList,
    /// The mesh's state, or how it stands when it could not be read.
    pub mesh: Result<&'a CooperationState, MeshAbsence>,
    /// Node id → display name.
    pub machines: &'a HashMap<String, String>,
    /// Runtime key → (liveness, milliseconds since its beat).
    pub beats: &'a HashMap<String, (StreamLiveness, Option<u64>)>,
    pub monitor: &'a MonitorPage,
}

/// The mesh sessions the board peer's own server projected for it.
///
/// The server holding a checkout's board writes each attached peer into its own mesh stream
/// as the session `board-<peer>`, with the checkout's identity and the client's name
/// (`Cooperation::project_board`). A peer id is a position on that server's board, handed out
/// again by every server and after every restart, so the join is bound to the stream the
/// board's server names (`BoardView::stream`, which carries the process instance): a session of
/// that stream naming the same board session, checkout and client is that peer. A board whose
/// server names no stream joins nothing. More than one fitting session is a candidate each.
pub(crate) fn join<'s>(
    peer: &Peer,
    checkout: &str,
    stream: Option<&str>,
    sessions: &[&'s SessionView],
) -> Vec<&'s SessionView> {
    let Some(stream) = stream else {
        return Vec::new();
    };
    let name = format!("board-{}", peer.id);
    // cut exactly as the projection cut it: bytes on a character boundary, controls removed
    let client = crate::mesh::cooperation::clip(&peer.client.name, 64);
    sessions
        .iter()
        .copied()
        .filter(|s| {
            s.state == SessionState::Active
                && s.stream.as_str() == stream
                && s.info.session == name
                && s.info.checkout.as_deref() == Some(checkout)
                && s.info.client == client
        })
        .collect()
}

fn board_key(checkout: &str, peer: &Peer) -> String {
    format!("board:{checkout}/{}", peer.id)
}

fn mesh_key(session: &SessionView) -> String {
    format!("mesh:{}", session.key)
}

/// Join the reads into the blackboard.
pub(crate) fn project(
    reads: &Reads<'_>,
    at: String,
    checkout: String,
    head: Option<String>,
) -> Blackboard {
    let mut sources = Vec::new();
    let unread: Vec<&str> = reads
        .board
        .boards
        .iter()
        .filter_map(|b| b.reason.as_deref())
        .collect();
    sources.push(BlackboardSource {
        source: "board".into(),
        state: if reads.board.complete {
            BlackboardSourceState::Ok
        } else {
            BlackboardSourceState::Partial
        },
        reason: (!unread.is_empty()).then(|| unread.join("; ")),
    });
    let (mesh_state, mesh_source) = match &reads.mesh {
        Ok(state) => (Some(*state), (BlackboardSourceState::Ok, None)),
        Err(MeshAbsence::Undeclared(why)) => (None, (BlackboardSourceState::Skipped, Some(why.clone()))),
        Err(MeshAbsence::Unreachable(why)) => (
            None,
            (
                BlackboardSourceState::Absent,
                Some(format!("{why}: other machines and the journal's claims, reviews and handovers are not covered")),
            ),
        ),
    };
    sources.push(BlackboardSource {
        source: "mesh".into(),
        state: mesh_source.0,
        reason: mesh_source.1,
    });
    // who is here is the boards' and the mesh's to say: the monitor's sources are what
    // happened, and one that has recorded nothing yet is not a worker nobody could ask
    let complete = sources.iter().all(|s| {
        matches!(
            s.state,
            BlackboardSourceState::Ok | BlackboardSourceState::Skipped
        )
    });
    for s in &reads.monitor.sources {
        sources.push(monitor_source(s));
    }

    let active: Vec<&SessionView> = mesh_state
        .map(|m| {
            m.sessions
                .iter()
                .filter(|s| s.state == SessionState::Active)
                .collect()
        })
        .unwrap_or_default();
    let claims_of = |session: &SessionView| -> Vec<BlackboardClaim> {
        mesh_state
            .map(|m| {
                m.claims
                    .iter()
                    .filter(|c| c.session == session.key && c.state.is_live())
                    .map(|c| BlackboardClaim {
                        name: None,
                        intent: c.intent.clone(),
                        scope: c.scope.clone(),
                        mode: c.mode,
                        issue: c.issue.clone(),
                        record: Some(c.key.clone()),
                        conflicted: matches!(c.state, ClaimState::Conflicted(_)),
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut workers = Vec::new();
    let mut relations = Vec::new();
    // session key → the worker that holds it
    let mut held: BTreeMap<String, String> = BTreeMap::new();
    for peer in &reads.board.peers {
        let Some(checkout) = peer.checkout.as_ref() else {
            continue;
        };
        let key = board_key(&checkout.id, peer);
        let stream = reads
            .board
            .boards
            .iter()
            .find(|b| b.checkout.id == checkout.id)
            .and_then(|b| b.stream.as_deref());
        let fits = join(peer, &checkout.id, stream, &active);
        let joined = (fits.len() == 1).then(|| fits[0]);
        for s in &fits {
            relations.push(BlackboardRelation {
                kind: BlackboardRelationKind::SameWorker,
                evidence: if joined.is_some() {
                    BlackboardEvidence::Verified
                } else {
                    BlackboardEvidence::Candidate
                },
                from: key.clone(),
                to: mesh_key(s),
                record: s.key.clone(),
            });
        }
        if let Some(s) = joined {
            held.insert(s.key.clone(), key.clone());
        }
        let beat = joined
            .and_then(|s| reads.beats.get(&s.runtime))
            .and_then(|b| b.1);
        workers.push(BlackboardWorker {
            key,
            origin: BlackboardOrigin::Board,
            presence: if peer.attached {
                BlackboardPresence::Attached
            } else {
                BlackboardPresence::Departed
            },
            client: peer.client.name.clone(),
            title: peer.client.title.clone().filter(|t| !t.is_empty()),
            version: Some(peer.client.version.clone()).filter(|v| !v.is_empty()),
            intent: peer.announcement.as_ref().map(|a| a.intent.clone()),
            checkout: Some(checkout.id.clone()),
            worktree: Some(checkout.worktree.clone()),
            branch: checkout.branch.clone(),
            board_peer: Some(peer.id.to_string()),
            session: joined.map(|s| s.key.clone()),
            machine: joined.and_then(|s| reads.machines.get(&s.node).cloned()),
            runtime: joined.map(|s| s.runtime.clone()),
            task: joined.and_then(|s| s.info.task.clone()),
            issue: joined.and_then(|s| s.info.issue.clone()),
            connected_at: Some(peer.connected_at.clone()),
            last_seen_seconds_ago: Some(peer.last_seen_seconds_ago),
            last_beat_ms: beat,
            claims: claims_of_peer(peer, joined.map(&claims_of).unwrap_or_default()),
        });
    }
    // a candidate session stays a worker of its own: which board peer it is cannot be told
    let candidates: BTreeMap<String, String> = relations
        .iter()
        .filter(|r| r.evidence == BlackboardEvidence::Candidate)
        .map(|r| (r.to.clone(), r.from.clone()))
        .collect();
    for s in &active {
        if held.contains_key(&s.key) {
            continue;
        }
        let key = mesh_key(s);
        held.insert(s.key.clone(), key.clone());
        workers.push(BlackboardWorker {
            key,
            origin: BlackboardOrigin::Mesh,
            presence: BlackboardPresence::Active,
            client: s.info.client.clone(),
            title: s.info.worker.clone(),
            version: None,
            intent: s.info.intent.clone(),
            checkout: s.info.checkout.clone(),
            worktree: None,
            branch: s.info.branch.clone(),
            board_peer: None,
            session: Some(s.key.clone()),
            machine: reads.machines.get(&s.node).cloned(),
            runtime: Some(s.runtime.clone()),
            task: s.info.task.clone(),
            issue: s.info.issue.clone(),
            connected_at: None,
            last_seen_seconds_ago: None,
            last_beat_ms: reads.beats.get(&s.runtime).and_then(|b| b.1),
            claims: claims_of(s),
        });
    }

    let (conflicts_total, conflicts) = conflicts_among(&workers, &candidates);

    let mut reviews = Vec::new();
    let mut handovers = Vec::new();
    if let Some(m) = mesh_state {
        for r in m.reviews.iter().filter(|r| r.state == ReviewState::Open) {
            let from = held
                .get(&r.session)
                .cloned()
                .unwrap_or_else(|| r.session.clone());
            if let Some(reviewer) = &r.reviewer {
                // the record names its reviewer as a session key, a bare session id, a runtime
                // or a person's word; only a whole session key is a worker — a bare `board-p3`
                // is a position every board has — so anything else stays as the record says it
                relations.push(BlackboardRelation {
                    kind: BlackboardRelationKind::Review,
                    evidence: BlackboardEvidence::Verified,
                    from: from.clone(),
                    to: held
                        .get(reviewer)
                        .cloned()
                        .unwrap_or_else(|| reviewer.clone()),
                    record: r.key.clone(),
                });
            }
            if reviews.len() < MAX_RECORDS {
                reviews.push(BlackboardReview {
                    key: r.key.clone(),
                    from,
                    subject: r.subject.clone(),
                    reviewer: r.reviewer.clone(),
                    issue: r.issue.clone(),
                });
            }
        }
        for h in m
            .handovers
            .iter()
            .filter(|h| h.consumed_by.is_empty())
            .take(MAX_RECORDS)
        {
            handovers.push(BlackboardHandover {
                id: h.id.clone(),
                runtime: h.runtime.clone(),
                name: h.handover.name.clone(),
                branch: h.handover.branch.clone(),
                issue: h.handover.issue.clone(),
                task: h.handover.task.clone(),
                created_at: h.handover.created_at.clone(),
            });
        }
    }

    Blackboard {
        at,
        checkout,
        head,
        complete,
        sources,
        boards: reads.board.boards.clone(),
        present: workers
            .iter()
            .filter(|w| w.presence != BlackboardPresence::Departed)
            .count(),
        workers,
        conflicts,
        conflicts_total,
        reviews,
        handovers,
        relations,
        signals: reads.monitor.signals.clone(),
        for_you: reads.monitor.for_you,
        cursor: reads.monitor.cursor.clone(),
    }
}

/// A board peer's claims: its announcements, and the live claims its joined session holds
/// beside them — an exclusive `mesh.claim` taken through its MCP session, say. The advisory
/// claims its server projected from the announcements are the announcements themselves and
/// are not counted twice; a projected claim is one that carries exactly what the projection
/// writes (`Cooperation::project_board`): advisory, the announcement's scope and clipped
/// intent, no issue. Each announcement stands for one such claim. Any other claim of the
/// session, a same-scope advisory claim with its own intent or issue among them, is kept.
fn claims_of_peer(peer: &Peer, session: Vec<BlackboardClaim>) -> Vec<BlackboardClaim> {
    let normal = |scope: &[String]| -> Vec<String> {
        scope
            .iter()
            .map(|p| p.trim().trim_start_matches("./").to_string())
            .filter(|p| !p.is_empty())
            .collect()
    };
    let mut claims: Vec<BlackboardClaim> = peer
        .claims
        .iter()
        .map(|c| BlackboardClaim {
            name: c.name.clone(),
            intent: Some(c.intent.clone()).filter(|i| !i.is_empty()),
            scope: c.scope.clone(),
            mode: ClaimMode::Advisory,
            issue: None,
            record: None,
            conflicted: false,
        })
        .collect();
    // what the projection wrote for each announcement, each matched at most once
    let mut projected: Vec<Option<(Vec<String>, Option<String>)>> = peer
        .claims
        .iter()
        .map(|c| {
            let intent = crate::mesh::cooperation::clip(&c.intent, 512);
            Some((normal(&c.scope), Some(intent).filter(|i| !i.is_empty())))
        })
        .collect();
    claims.extend(session.into_iter().filter(|c| {
        if c.mode != ClaimMode::Advisory || c.conflicted || c.issue.is_some() {
            return true;
        }
        let this = (normal(&c.scope), c.intent.clone());
        match projected.iter_mut().find(|p| p.as_ref() == Some(&this)) {
            Some(slot) => {
                *slot = None;
                false
            }
            None => true,
        }
    }));
    claims
}

fn monitor_source(s: &MonitorSourceState) -> BlackboardSource {
    let name = serde_json::to_value(s.source)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    BlackboardSource {
        source: format!("monitor.{name}"),
        state: match s.state {
            MonitorSourceHealth::Ok => BlackboardSourceState::Ok,
            MonitorSourceHealth::Skipped => BlackboardSourceState::Skipped,
            MonitorSourceHealth::Absent | MonitorSourceHealth::Error => {
                BlackboardSourceState::Absent
            }
        },
        reason: s.reason.clone(),
    }
}

/// Every pair of claims of two different workers whose scopes meet, confirmed first, at most
/// [`MAX_CONFLICTS`], with how many there are in all. A worker and a candidate session of its
/// own are not two workers for this: the session's claims are the projection of the peer's.
fn conflicts_among(
    workers: &[BlackboardWorker],
    candidates: &BTreeMap<String, String>,
) -> (usize, Vec<BlackboardConflict>) {
    let same = |a: &str, b: &str| {
        candidates.get(a).is_some_and(|w| w == b) || candidates.get(b).is_some_and(|w| w == a)
    };
    let mut confirmed = Vec::new();
    let mut potential = Vec::new();
    for (i, first) in workers.iter().enumerate() {
        for second in workers.iter().skip(i + 1) {
            if same(&first.key, &second.key) {
                continue;
            }
            for a in &first.claims {
                for b in &second.claims {
                    let paths: Vec<OverlapPath> = a
                        .scope
                        .iter()
                        .flat_map(|x| {
                            b.scope
                                .iter()
                                .filter(|y| claims_meet(x, y))
                                .map(|y| OverlapPath {
                                    yours: x.clone(),
                                    theirs: y.clone(),
                                })
                        })
                        .collect();
                    if paths.is_empty() {
                        continue;
                    }
                    let both_exclusive =
                        a.mode == ClaimMode::Exclusive && b.mode == ClaimMode::Exclusive;
                    let certainty = if both_exclusive || a.conflicted || b.conflicted {
                        BlackboardCertainty::Confirmed
                    } else {
                        BlackboardCertainty::Potential
                    };
                    let conflict = BlackboardConflict {
                        certainty,
                        first: claim_ref(first, a),
                        second: claim_ref(second, b),
                        paths,
                    };
                    match certainty {
                        BlackboardCertainty::Confirmed => confirmed.push(conflict),
                        BlackboardCertainty::Potential => potential.push(conflict),
                    }
                }
            }
        }
    }
    let total = confirmed.len() + potential.len();
    confirmed.extend(potential);
    confirmed.truncate(MAX_CONFLICTS);
    (total, confirmed)
}

fn claim_ref(worker: &BlackboardWorker, claim: &BlackboardClaim) -> BlackboardClaimRef {
    BlackboardClaimRef {
        worker: worker.key.clone(),
        claim: claim.name.clone(),
        record: claim.record.clone(),
    }
}

// ---------------------------------------------------------------- the handler

/// Node id → display name: every node the mesh has observed, and this one.
fn machines(
    nodes: Vec<crate::mesh::registry::NodeRecord>,
    me: Option<crate::mesh::identity::PublicIdentity>,
) -> HashMap<String, String> {
    let mut machines = HashMap::new();
    for n in nodes {
        machines.insert(n.node_id.as_str().to_string(), n.display_name);
    }
    if let Some(me) = me {
        machines.insert(me.node_id.as_str().to_string(), me.display_name);
    }
    machines
}

/// How a mesh that is not read stands, from what the repository declares: one it does not
/// declare, or declares disabled, is not missing; one it declares and that could not be read
/// is, for `reason`.
fn absence(
    declaration: Option<Result<crate::mesh::MeshConfig, crate::mesh::MeshError>>,
    reason: String,
) -> MeshAbsence {
    match declaration {
        None => MeshAbsence::Undeclared("the repository declares no mesh".into()),
        Some(Ok(config)) if !config.enabled => {
            MeshAbsence::Undeclared("the repository's mesh declaration is disabled".into())
        }
        _ => MeshAbsence::Unreachable(reason),
    }
}

fn snapshot(ctx: &Context, input: BlackboardInput) -> Result<Blackboard, CapabilityError> {
    let asked = PeerListInput {
        checkouts: input.checkouts,
    };
    super::peers::peers_list(ctx, asked).and_then(|board| read(ctx, input, &board))
}

/// The blackboard over `board`: the signals and the mesh read, and every source joined.
fn read(
    ctx: &Context,
    input: BlackboardInput,
    board: &PeerList,
) -> Result<Blackboard, CapabilityError> {
    let wanted = input.signals.unwrap_or(DEFAULT_SIGNALS).min(MAX_SIGNALS);
    // no signals asked for is no monitor read: the journal is read once less, which is most
    // of what the answer costs
    let monitor = if wanted == 0 {
        MonitorPage::default()
    } else {
        super::monitor::monitor(
            ctx,
            MonitorInput {
                after: None,
                limit: Some(wanted),
                sources: Some(input.sources.unwrap_or_else(|| DEFAULT_SOURCES.into())),
                reader: input.reader,
                since: None,
            },
        )?
    };
    let cooperation = ctx.mesh.cooperation();
    let state = cooperation.as_ref().map(|c| c.state());
    let beats: HashMap<String, (StreamLiveness, Option<u64>)> = cooperation
        .as_ref()
        .map(|c| {
            c.streams()
                .into_iter()
                .map(|(stream, liveness, beat)| (stream.runtime_key(), (liveness, beat)))
                .collect()
        })
        .unwrap_or_default();
    let machines = machines(ctx.mesh.nodes(), ctx.mesh.status().identity);
    let reason = ctx
        .mesh
        .cooperation_status()
        .reason
        .unwrap_or_else(|| "the mesh is not running in this server".to_string());
    let absence = absence(super::mesh::resolved_declaration(ctx).declaration, reason);
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let head = crate::worktree::git::head_of(&root).ok().flatten();
    Ok(project(
        &Reads {
            board,
            mesh: state.as_ref().ok_or(absence),
            machines: &machines,
            beats: &beats,
            monitor: &monitor,
        },
        rfc3339(SystemTime::now()),
        repository::identity(&root),
        head,
    ))
}

/// The `blackboard` module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "blackboard",
        title: "The peer blackboard",
        description: "Who works on what in this repository, as one projection of the records that already know it: every checkout's board, the mesh journal's sessions, claims, reviews and handovers on every linked machine, and the monitor's newest signals, joined once so that no surface joins them again (ADR 0176). A board peer and a mesh session are one worker only when an explicit identifier joins them; a source that could not be read makes the answer incomplete and says why.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "blackboard.snapshot",
                title: "Who works on what, now",
                description: "Every worker of this repository — attached to a checkout's board or in an active mesh session on any linked machine — with the client it is, what it announced, its checkout, branch, machine and claims (advisory or exclusive); the claims that meet (potential, or confirmed when both are exclusive or the mesh marked one conflicted); the open reviews and untaken handovers; typed relations between workers, verified only when an explicit identifier joins them; the newest signals with the monitor's cursor a follower resumes from; each source with how it was read, and whether the answer is complete. Reads only.",
                input: BlackboardInput,
                output: Blackboard,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_blackboard"), http: get("/api/v1/blackboard"), cli: Some(CliExposure { path: vec!["blackboard".into()] }) },
                tags: ["peers", "mesh", "monitor", "coordination"],
                handler: snapshot,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::journal::{SessionInfo, StreamId};
    use crate::mesh::state::{ClaimView, ReviewView};
    use crate::peers::{Announcement, ClientInfo, PeerCheckout, PeerId, Transport};

    fn peer(id: &str, client: &str, checkout: &str, claims: &[(&str, &[&str])]) -> Peer {
        let claims: Vec<Announcement> = claims
            .iter()
            .map(|(intent, scope)| Announcement {
                name: None,
                intent: (*intent).into(),
                scope: scope.iter().map(|s| (*s).into()).collect(),
                at: "2026-10-18T00:00:00Z".into(),
            })
            .collect();
        Peer {
            id: serde_json::from_value::<PeerId>(serde_json::json!(id)).unwrap(),
            client: ClientInfo {
                name: client.into(),
                version: "1".into(),
                title: None,
            },
            transport: Transport::Http,
            connected_at: "2026-10-18T00:00:00Z".into(),
            last_seen_seconds_ago: 1,
            attached: true,
            announcement: claims.last().cloned(),
            claims,
            checkout: Some(PeerCheckout {
                id: checkout.into(),
                worktree: PathBuf::from("/w"),
                branch: Some("master".into()),
                this_checkout: true,
            }),
        }
    }

    /// The stream of machine `node`, runtime `runtime` and process `instance` (one hex digit
    /// each).
    fn stream(node: &str, runtime: &str, instance: &str) -> StreamId {
        StreamId::new(&node.repeat(32), &runtime.repeat(16), &instance.repeat(16)).unwrap()
    }

    fn session(at: &StreamId, id: &str, client: &str, checkout: Option<&str>) -> SessionView {
        SessionView {
            key: format!("{}/{id}", at.as_str()),
            stream: at.clone(),
            runtime: at.runtime_key(),
            node: at.as_str()[..32].to_string(),
            info: SessionInfo {
                session: id.into(),
                client: client.into(),
                checkout: checkout.map(str::to_string),
                ..SessionInfo::default()
            },
            state: SessionState::Active,
            updated_lamport: 1,
        }
    }

    fn claim(of: &SessionView, key: &str, scope: &[&str], mode: ClaimMode) -> ClaimView {
        ClaimView {
            key: key.into(),
            stream: of.stream.clone(),
            runtime: of.runtime.clone(),
            session: of.key.clone(),
            scope: scope.iter().map(|s| (*s).into()).collect(),
            intent: None,
            mode,
            issue: None,
            acquired_lamport: 1,
            acquired_seq: 1,
            state: ClaimState::Held,
        }
    }

    /// A board of checkout `checkout` whose server projects into `stream`.
    fn list(peers: Vec<Peer>, checkout: &str, stream: Option<&StreamId>) -> PeerList {
        let board = serde_json::json!({
            "id": checkout, "worktree": "/w", "this_checkout": true, "standing": "ready",
            "attached": peers.len(), "stream": stream.map(|s| s.as_str()),
        });
        PeerList {
            count: peers.len(),
            caller: None,
            peers,
            overlaps: Vec::new(),
            boards: vec![serde_json::from_value(board).unwrap()],
            complete: true,
            stream: stream.map(|s| s.as_str().to_string()),
        }
    }

    fn page() -> MonitorPage {
        serde_json::from_value(serde_json::json!({
            "signals": [], "for_you": 0, "suppressed": 0, "sources": [], "cursor": "x0", "more": false
        }))
        .unwrap()
    }

    fn answer(board: &PeerList, mesh: Result<&CooperationState, MeshAbsence>) -> Blackboard {
        let machines = HashMap::from([("1".repeat(32), "here".to_string())]);
        let beats = HashMap::new();
        let monitor = page();
        project(
            &Reads {
                board,
                mesh,
                machines: &machines,
                beats: &beats,
                monitor: &monitor,
            },
            "2026-10-18T00:00:00Z".into(),
            "c1".into(),
            None,
        )
    }

    /// A board peer is joined to the session its own server's stream projected for it; the
    /// same board session id in another process of the same runtime (a restarted or superseded
    /// server), on another machine or for another checkout is a different worker.
    #[test]
    fn a_peer_is_joined_only_through_its_servers_stream() {
        let here = stream("1", "1", "a");
        let board = list(
            vec![peer("p1", "claude-code", "c1", &[("x", &["apps"])])],
            "c1",
            Some(&here),
        );
        let state = CooperationState {
            sessions: vec![
                session(&here, "board-p1", "claude-code", Some("c1")),
                session(
                    &stream("1", "1", "b"),
                    "board-p1",
                    "claude-code",
                    Some("c1"),
                ),
                session(
                    &stream("2", "1", "a"),
                    "board-p1",
                    "claude-code",
                    Some("c1"),
                ),
                session(&here, "board-p2", "claude-code", Some("c1")),
            ],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        assert!(b.complete);
        assert_eq!(b.workers.len(), 4, "{:#?}", b.workers);
        let joined = &b.workers[0];
        assert_eq!(joined.key, "board:c1/p1");
        assert_eq!(
            joined.session.as_deref(),
            Some(state.sessions[0].key.as_str())
        );
        assert_eq!(joined.machine.as_deref(), Some("here"));
        assert_eq!(b.relations.len(), 1);
        assert_eq!(b.relations[0].evidence, BlackboardEvidence::Verified);
        assert!(b.workers[1..]
            .iter()
            .all(|w| w.origin == BlackboardOrigin::Mesh));

        // a board whose server names no stream joins nothing
        let unnamed = list(board.peers.clone(), "c1", None);
        let b = answer(&unnamed, Ok(&state));
        assert!(b.relations.is_empty());
        assert_eq!(b.workers.len(), 5);
    }

    /// A joined peer keeps the exclusive claim its session took beside its announcements, and
    /// the advisory claim its server projected from an announcement is not counted twice.
    #[test]
    fn a_joined_peer_keeps_its_exclusive_mesh_claims() {
        let here = stream("1", "1", "a");
        let board = list(
            vec![peer("p1", "codex", "c1", &[("x", &["docs"])])],
            "c1",
            Some(&here),
        );
        let s = session(&here, "board-p1", "codex", Some("c1"));
        // what the projection writes for the announcement: advisory, its scope, its intent
        let mut projected = claim(&s, "k1", &["docs"], ClaimMode::Advisory);
        projected.intent = Some("x".into());
        let state = CooperationState {
            claims: vec![projected, claim(&s, "k2", &["apps"], ClaimMode::Exclusive)],
            sessions: vec![s],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        let claims = &b.workers[0].claims;
        assert_eq!(claims.len(), 2, "{claims:#?}");
        assert_eq!(claims[1].mode, ClaimMode::Exclusive);
        assert_eq!(claims[1].record.as_deref(), Some("k2"));
    }

    /// An advisory claim the session took itself over the same paths as an announcement, with
    /// its own intent or issue, is not the announcement's projection: it is kept with its
    /// record. Only a claim carrying exactly what the projection writes is folded, once.
    #[test]
    fn an_independent_same_scope_claim_is_kept() {
        let here = stream("1", "1", "a");
        let board = list(
            vec![peer("p1", "codex", "c1", &[("document UI", &["docs"])])],
            "c1",
            Some(&here),
        );
        let s = session(&here, "board-p1", "codex", Some("c1"));
        let mut projected = claim(&s, "k1", &["docs"], ClaimMode::Advisory);
        projected.intent = Some("document UI".into());
        let mut review = claim(&s, "k2", &["docs"], ClaimMode::Advisory);
        review.intent = Some("review delivery".into());
        review.issue = Some("I2980".into());
        let mut twin = claim(&s, "k3", &["./docs"], ClaimMode::Advisory);
        twin.intent = Some("document UI".into());
        let state = CooperationState {
            claims: vec![projected, review, twin],
            sessions: vec![s],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        let claims = &b.workers[0].claims;
        let records: Vec<Option<&str>> = claims.iter().map(|c| c.record.as_deref()).collect();
        assert_eq!(records, vec![None, Some("k2"), Some("k3")], "{claims:#?}");
        assert_eq!(claims[1].issue.as_deref(), Some("I2980"));
        assert_eq!(claims[1].intent.as_deref(), Some("review delivery"));
    }

    /// A client name is cut for the join exactly as the projection cut it — at 64 bytes on a
    /// character boundary, controls removed — so a long multi-byte or control-bearing name is
    /// one worker, not two.
    #[test]
    fn a_multibyte_client_name_joins_as_the_projection_cut_it() {
        let here = stream("1", "1", "a");
        let name = format!("{}\u{7}", "\u{17e}".repeat(40));
        let board = list(vec![peer("p1", &name, "c1", &[])], "c1", Some(&here));
        let projected_client = crate::mesh::cooperation::clip(&name, 64);
        assert_eq!(projected_client, "\u{17e}".repeat(32));
        let state = CooperationState {
            sessions: vec![session(&here, "board-p1", &projected_client, Some("c1"))],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        assert_eq!(b.workers.len(), 1, "{:#?}", b.workers);
        assert_eq!(b.relations[0].evidence, BlackboardEvidence::Verified);
    }

    /// Two sessions of one stream that fit the same peer are candidates, neither is joined,
    /// and the peer is not in conflict with its own projection.
    #[test]
    fn two_fitting_sessions_are_candidates_and_no_join() {
        let here = stream("1", "1", "a");
        let board = list(
            vec![peer("p1", "codex", "c1", &[("x", &["docs"])])],
            "c1",
            Some(&here),
        );
        let one = session(&here, "board-p1", "codex", Some("c1"));
        let mut two = one.clone();
        two.key.push('x');
        let state = CooperationState {
            claims: vec![claim(&one, "k1", &["docs"], ClaimMode::Advisory)],
            sessions: vec![one, two],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        assert_eq!(b.workers[0].session, None);
        assert_eq!(b.relations.len(), 2);
        assert!(b
            .relations
            .iter()
            .all(|r| r.evidence == BlackboardEvidence::Candidate));
        assert_eq!(b.workers.len(), 3);
        assert_eq!(b.conflicts_total, 0, "{:#?}", b.conflicts);
    }

    /// Claims of two workers that meet are a conflict: potential when one is advisory,
    /// confirmed when both are exclusive. Claims that do not meet are not.
    #[test]
    fn meeting_claims_are_conflicts_and_exclusive_ones_are_confirmed() {
        let board = list(
            vec![
                peer("p1", "a", "c1", &[("one", &["apps/majordomus-cli"])]),
                peer("p2", "b", "c1", &[("two", &["apps"]), ("three", &["site"])]),
            ],
            "c1",
            None,
        );
        let b = answer(&board, Err(MeshAbsence::Undeclared("none".into())));
        assert_eq!(b.conflicts_total, 1);
        assert_eq!(b.conflicts[0].certainty, BlackboardCertainty::Potential);
        assert_eq!(b.conflicts[0].first.worker, "board:c1/p1");
        assert_eq!(b.conflicts[0].second.worker, "board:c1/p2");

        let mut x = b.workers.clone();
        for w in &mut x {
            for c in &mut w.claims {
                c.mode = ClaimMode::Exclusive;
            }
        }
        let (total, found) = conflicts_among(&x, &BTreeMap::new());
        assert_eq!(total, 1);
        assert_eq!(found[0].certainty, BlackboardCertainty::Confirmed);
    }

    /// A review names its reviewer as the requester wrote it: a whole session key a worker
    /// holds is that worker, a bare `board-p1` is a position every board has and stays a word.
    #[test]
    fn a_reviewer_is_a_worker_only_by_a_whole_session_key() {
        let here = stream("1", "1", "a");
        let board = list(vec![peer("p1", "codex", "c1", &[])], "c1", Some(&here));
        let s = session(&here, "board-p1", "codex", Some("c1"));
        let review = |key: &str, reviewer: &str| ReviewView {
            key: key.into(),
            session: "elsewhere/s9".into(),
            subject: "read this".into(),
            scope: Vec::new(),
            issue: None,
            reviewer: Some(reviewer.into()),
            body: None,
            state: ReviewState::Open,
            answers: Vec::new(),
            result: None,
        };
        let state = CooperationState {
            reviews: vec![review("r1", &s.key), review("r2", "board-p1")],
            sessions: vec![s],
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        let to: Vec<&str> = b
            .relations
            .iter()
            .filter(|r| r.kind == BlackboardRelationKind::Review)
            .map(|r| r.to.as_str())
            .collect();
        assert_eq!(to, ["board:c1/p1", "board-p1"]);
        assert_eq!(b.reviews.len(), 2);
        assert_eq!(b.reviews[0].from, "elsewhere/s9");
    }

    /// A mesh that is declared and not running and a board that could not be read each make
    /// the answer incomplete with their reason; a mesh the repository does not declare does
    /// not; the workers that could be read are there either way.
    #[test]
    fn only_what_could_not_be_asked_makes_the_answer_incomplete() {
        let mut board = list(vec![peer("p1", "a", "c1", &[])], "c1", None);
        let b = answer(&board, Err(MeshAbsence::Unreachable("not active".into())));
        assert!(!b.complete);
        let mesh = b.sources.iter().find(|s| s.source == "mesh").unwrap();
        assert_eq!(mesh.state, BlackboardSourceState::Absent);
        assert!(mesh.reason.as_deref().unwrap().starts_with("not active"));
        assert_eq!(b.present, 1);
        assert_eq!(b.workers[0].intent, None, "announced nothing");

        let b = answer(&board, Err(MeshAbsence::Undeclared("no mesh".into())));
        assert!(b.complete, "{:?}", b.sources);

        board.complete = false;
        let b = answer(&board, Ok(&CooperationState::default()));
        assert!(!b.complete);
        assert_eq!(b.sources[0].state, BlackboardSourceState::Partial);
    }

    /// What a reading could not read is said with its reason, a peer that left is departed
    /// with the title its client gave, a peer of no checkout is no worker, a handover nobody
    /// took waits, and a monitor source that was read is `ok`.
    #[test]
    fn what_was_not_read_who_left_and_what_waits_are_said() {
        let mut gone = peer("p2", "codex", "c1", &[]);
        gone.attached = false;
        gone.client.title = Some("Codex".into());
        let mut nowhere = peer("p3", "x", "c1", &[]);
        nowhere.checkout = None;
        let mut board = list(vec![peer("p1", "a", "c1", &[]), gone, nowhere], "c1", None);
        board.complete = false;
        board.boards[0].reason = Some("its server is starting".into());
        let here = stream("1", "1", "a");
        let body = crate::mesh::journal::HandoverBody {
            id: "h".into(),
            task: None,
            issue: Some("I1".into()),
            milestone: None,
            branch: Some("side".into()),
            head: None,
            created_at: None,
            name: Some("the rest".into()),
            body: "# Objective\n".into(),
        };
        let handover = |id: &str, consumed: &[&str]| crate::mesh::state::HandoverView {
            id: id.into(),
            stream: here.clone(),
            runtime: here.runtime_key(),
            published_lamport: 1,
            handover: body.clone(),
            consumed_by: consumed.iter().map(|c| (*c).into()).collect(),
        };
        let state = CooperationState {
            handovers: vec![handover("h-open", &[]), handover("h-taken", &["s/x"])],
            ..CooperationState::default()
        };
        let monitor: MonitorPage = serde_json::from_value(serde_json::json!({
            "signals": [], "for_you": 0, "suppressed": 0, "cursor": "x0", "more": false,
            "sources": [{"source": "mesh", "state": "ok", "signals": 0, "suppressed": 0}]
        }))
        .unwrap();
        let (machines, beats) = (HashMap::new(), HashMap::new());
        let b = project(
            &Reads {
                board: &board,
                mesh: Ok(&state),
                machines: &machines,
                beats: &beats,
                monitor: &monitor,
            },
            "2026-10-18T00:00:00Z".into(),
            "c1".into(),
            None,
        );
        assert!(!b.complete);
        assert_eq!(
            b.sources[0].reason.as_deref(),
            Some("its server is starting")
        );
        let read = b
            .sources
            .iter()
            .find(|s| s.source == "monitor.mesh")
            .unwrap();
        assert_eq!(read.state, BlackboardSourceState::Ok);
        assert_eq!(b.workers.len(), 2, "a peer of no checkout is no worker");
        let left = b.workers.iter().find(|w| w.client == "codex").unwrap();
        assert_eq!(left.presence, BlackboardPresence::Departed);
        assert_eq!(left.title.as_deref(), Some("Codex"));
        assert_eq!(b.handovers.len(), 1, "{:?}", b.handovers);
        assert_eq!(b.handovers[0].id, "h-open");
        assert_eq!(b.handovers[0].name.as_deref(), Some("the rest"));
    }

    /// Every observed node is named, and this one by its own name.
    #[test]
    fn every_observed_node_and_this_one_are_named() {
        let me = crate::mesh::identity::NodeIdentity::ephemeral()
            .unwrap()
            .public;
        let other = crate::mesh::identity::NodeIdentity::ephemeral()
            .unwrap()
            .public;
        let node: crate::mesh::registry::NodeRecord = serde_json::from_value(serde_json::json!({
            "node_id": other.node_id, "public_key": other.public_key, "display_name": "lundra",
            "instance_id": other.instance_id, "trust": { "state": "observed" },
            "presence": "present", "endpoints": [], "capabilities": [], "repositories": [],
            "protocol_version": 1, "version": "t", "sources": [], "first_seen": "",
            "last_seen": "", "restarts": 0,
        }))
        .unwrap();
        let named = machines(vec![node], Some(me.clone()));
        assert_eq!(named[other.node_id.as_str()], "lundra");
        assert_eq!(named[me.node_id.as_str()], me.display_name);
        assert!(machines(Vec::new(), None).is_empty());
    }

    /// A review for anyone relates nobody, and the reviews listed stop at the bound while
    /// every one is still read.
    #[test]
    fn a_review_for_anyone_relates_nobody_and_the_list_is_bounded() {
        let board = list(vec![peer("p1", "codex", "c1", &[])], "c1", None);
        let review = |n: usize, reviewer: Option<&str>| ReviewView {
            key: format!("s/r-{n}"),
            session: "elsewhere/s9".into(),
            subject: "read this".into(),
            scope: Vec::new(),
            issue: None,
            reviewer: reviewer.map(str::to_string),
            body: None,
            state: ReviewState::Open,
            answers: Vec::new(),
            result: None,
        };
        let mut reviews = vec![review(0, None)];
        reviews.extend((1..=MAX_RECORDS).map(|n| review(n, Some("1b"))));
        let state = CooperationState {
            reviews,
            ..CooperationState::default()
        };
        let b = answer(&board, Ok(&state));
        assert_eq!(b.reviews.len(), MAX_RECORDS);
        assert_eq!(b.reviews[0].reviewer, None);
        assert_eq!(
            b.relations
                .iter()
                .filter(|r| r.kind == BlackboardRelationKind::Review)
                .count(),
            MAX_RECORDS,
            "the review for anyone relates nobody"
        );
    }

    /// A mesh the repository does not declare, or declares disabled, is not missing; one it
    /// declares that could not be read is, with the reason.
    #[test]
    fn a_mesh_is_missing_only_when_it_is_declared() {
        let declared = |enabled: bool| {
            crate::mesh::MeshConfig::from_metadata(
                &serde_json::json!({
                    "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": enabled,
                }),
                "x",
            )
        };
        let undeclared = |a: MeshAbsence| matches!(a, MeshAbsence::Undeclared(_));
        assert!(undeclared(absence(None, "off".into())));
        assert!(undeclared(absence(Some(declared(false)), "off".into())));
        assert!(matches!(
            absence(Some(declared(true)), "not active".into()),
            MeshAbsence::Unreachable(r) if r == "not active"
        ));
    }

    /// Asked through the capability, a running mesh is read, and signals asked of a source the
    /// monitor does not know are refused.
    #[test]
    fn the_handler_reads_a_running_mesh_and_refuses_an_unknown_source() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mesh_of = |ctx: &Context| {
            let v = ctx
                .execute(
                    "blackboard.snapshot",
                    serde_json::json!({ "checkouts": "this", "signals": 0 }),
                )
                .unwrap();
            let mesh = v["sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["source"] == "mesh")
                .cloned()
                .unwrap();
            (mesh["state"].as_str().unwrap().to_string(), v)
        };
        let ctx = repo.context().unwrap();
        crate::mesh::testing::attach(&ctx, crate::monitor::testing::cooperation());
        let (state, v) = mesh_of(&ctx);
        assert_eq!(state, "ok", "{v}");
        assert!(ctx
            .execute(
                "blackboard.snapshot",
                serde_json::json!({ "checkouts": "this", "signals": 5, "sources": "nowhere" }),
            )
            .is_err());
    }

    #[test]
    fn the_benchmark_cases_ask_nothing_beyond_this_machine() {
        for case in cases() {
            assert_eq!(case.input.checkouts, Checkouts::This, "{}", case.name);
            let sources = super::super::monitor::wanted(case.input.sources.as_deref()).unwrap();
            assert!(
                !sources.contains(&crate::monitor::MonitorSource::Github),
                "{}",
                case.name
            );
        }
        let default = super::super::monitor::wanted(Some(DEFAULT_SOURCES)).unwrap();
        assert!(!default.contains(&crate::monitor::MonitorSource::Github));
    }
}
