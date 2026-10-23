//! The `brief` module: what surrounds one working session, in one line and in a few, so that
//! a session can report on itself without being asked (ADR 0136). Nothing here writes.
//!
//! # Why one capability composes what five already answer
//!
//! Every fact in the brief has an owner already: `lifecycle.episodes` and `continuity.state`
//! the session and its task, `peers.list` the board of every checkout, `mesh.cooperation`
//! and `mesh.state` the machines, `majordomus prs brief` the integration queue, and the
//! machine's derive lock (`lib/machine_lock.sh`) the derive queue. A session reads none of
//! them unless it remembers to, and on 2026-10-10 the sessions of this repository relayed
//! board state to each other by hand. The brief asks each owner once and says, per section,
//! whether it could be read; it judges nothing of its own and stores nothing.
//!
//! # Who answers
//!
//! The board and the mesh live in the memory of the server that holds this checkout's lease.
//! A command-line process has an empty board of its own and no mesh, so a brief composed
//! there would report "nobody is here" when the truth is "I did not ask". The command line
//! therefore asks the leaseholder for the brief (`local: true`, so the server never asks
//! back), and composes it itself only when no server answers — saying so.
//!
//! A server that lost the lease (superseded by a newer one) still holds its own MCP
//! sessions, and the repository's gather cannot see them: `peers.list` reads a checkout's
//! board from the server its lease names. Such a server composes the brief from its own
//! board, adds the leaseholder's, and says that its own is unreachable by everyone else.

use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::lease;
use crate::peers::Peer;
use crate::{capability, module};

use super::lifecycle::Episodes;
use super::peers::PeerList;
use super::server::{self, Checkouts, ServerStanding};
use super::{get, mcp, Continuity};

/// The interval a policy that declares none gets, in minutes.
pub(crate) const DEFAULT_INTERVAL_MINUTES: u64 = 20;
/// How many peers the multi-line brief names before it says how many it left out.
const PEERS_NAMED: usize = 8;
/// How many characters of an announced intent a peer's line keeps.
const INTENT_CHARS: usize = 72;
/// How long the command line waits for the leaseholder's brief: its gather probes every
/// checkout's server, measured at about 3 s over 118 checkouts.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `brief.session`.
pub struct BriefInput {
    /// The provider's own session identity (Claude Code's session id): the brief names the open episode that session owns. Absent, the episode the pointer resolves to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Which checkouts the peers section covers: `repository` (every board, the default) or `this`.
    #[serde(default)]
    pub checkouts: Checkouts,
    /// Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again.
    #[serde(default)]
    pub local: bool,
}

impl BenchmarkCases for BriefInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // every field filled, so that the GET's query parameters each carry an example, and
        // narrow: a benchmark asks no other checkout's server and forwards nothing
        vec![NamedCase::new(
            "default",
            BriefInput {
                session: Some("00000000-0000-0000-0000-000000000000".into()),
                checkouts: Checkouts::This,
                local: true,
            },
        )]
    }
}

// ---------------------------------------------------------------- output

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Whether a section of the brief could be read.
pub enum BriefSectionState {
    /// Its owner answered in full.
    Read,
    /// Its owner answered, and part of what it covers could not be asked; the summary says which.
    Partial,
    /// Its owner did not answer; the summary says why.
    Unread,
    /// The repository declares nothing for it (no derive lock, a mesh switched off).
    Undeclared,
}

impl BriefSectionState {
    fn as_str(self) -> &'static str {
        match self {
            BriefSectionState::Read => "read",
            BriefSectionState::Partial => "partial",
            BriefSectionState::Unread => "unread",
            BriefSectionState::Undeclared => "undeclared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Which process composed the brief, and why that one.
pub struct BriefOrigin {
    /// `leaseholder` (the server this checkout's lease names), `superseded` (a server that lost the lease), or `this_process` (no server answered, or the call asked for `local`).
    pub process: String,
    /// The address of the server that composed it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Why this process and not the leaseholder, when it is not the leaseholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The session the brief is about.
pub struct BriefSessionSection {
    /// Whether it could be read.
    pub state: BriefSectionState,
    /// The open episode, when one is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    /// The provider session that owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session: Option<String>,
    /// The checkout's branch.
    pub branch: String,
    /// The checkout's head, abbreviated.
    pub head: String,
    /// The active task's id, when one is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// The active task's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The calling MCP session's place on this board, when the call came through one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The workers of this repository, from every board the brief could read.
pub struct BriefPeersSection {
    /// Whether every board could be read.
    pub state: BriefSectionState,
    /// Attached peers across the boards read, the caller included.
    pub attached: usize,
    /// How many boards the answer covers.
    pub boards: usize,
    /// Every board that could not be read, as `<worktree>: <reason>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unread: Vec<String>,
    /// Pairs of peers whose claimed scopes meet.
    pub overlaps: usize,
    /// Set when the composing server is superseded: its own board is unreachable by the repository's gather.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded: Option<String>,
    /// The attached peers, one short line each, at most a bounded number.
    pub named: Vec<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The mesh: the links of this runtime and what the journal addresses to it.
pub struct BriefMeshSection {
    /// Whether cooperation could be read.
    pub state: BriefSectionState,
    /// Whether cooperation runs in the composing process.
    pub active: bool,
    /// Why it does not, when it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Links to other runtimes.
    pub links: usize,
    /// Of them, connected.
    pub connected: usize,
    /// Of them, degraded, unreachable or expired.
    pub failing: usize,
    /// Claims held across the mesh.
    pub claims_held: usize,
    /// Of them, held by this runtime.
    pub own_claims: usize,
    /// Open review requests that name this runtime as the reviewer.
    pub reviews_for_you: usize,
    /// Handovers another runtime published that nobody has consumed.
    pub handovers_waiting: usize,
    /// Overlaps between a claim of this runtime and another's.
    pub overlaps_for_you: usize,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The integration queue, as `majordomus prs brief` says it.
pub struct BriefIntegrationSection {
    /// Whether this checkout ever observed the forge.
    pub state: BriefSectionState,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Whether the trunk's landed work is released, as `release.currency` says it (I3070).
pub struct BriefReleaseSection {
    /// `read` when it was measured, `unread` when it could not be, with the summary saying why.
    pub state: BriefSectionState,
    /// Current, owed, overdue or unknown, when the capability answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<crate::release::currency::Stance>,
    /// Landed commits past the last release.
    #[serde(default)]
    pub landed: usize,
    /// The section in words.
    pub summary: String,
}

impl Default for BriefReleaseSection {
    /// What a brief composed by a server older than this section carries: nothing measured.
    fn default() -> Self {
        BriefReleaseSection {
            state: BriefSectionState::Unread,
            stance: None,
            landed: 0,
            summary: "the server that composed this brief does not measure the release".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The machine's derive queue: its lock and its priority marker (`lib/machine_lock.sh`).
pub struct BriefDeriveSection {
    /// `undeclared` when the repository ships no derive lock.
    pub state: BriefSectionState,
    /// The lock's owner line, when the lock is held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    /// The holder's process id, when its line names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Whether that process is running on this machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    /// Seconds since the lock was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_seconds: Option<u64>,
    /// The branches the priority marker names, when it is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// The section in words.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The answer of `brief.session`: what surrounds one session, in one line and in a few.
pub struct SessionBrief {
    /// When it was composed, RFC 3339.
    pub at: String,
    /// The brief in one line: the status line's text.
    pub line: String,
    /// The brief in a few lines: one per section, then the peers it names.
    pub lines: Vec<String>,
    /// How often a session reports on itself, in minutes (`session.brief_interval_minutes`).
    pub interval_minutes: u64,
    /// What a session is told at its start about keeping itself awake.
    pub keep_awake: String,
    /// Which process composed it.
    pub answered_by: BriefOrigin,
    /// This session: its episode and task.
    pub session: BriefSessionSection,
    /// The workers of every board.
    pub peers: BriefPeersSection,
    /// The mesh.
    pub mesh: BriefMeshSection,
    /// The integration queue.
    pub integration: BriefIntegrationSection,
    /// Whether the trunk's landed work is released. Absent from the answer of a server older
    /// than this section, which leaves it unread rather than failing the whole brief.
    #[serde(default)]
    pub release: BriefReleaseSection,
    /// The derive queue.
    pub derive: BriefDeriveSection,
}

// ---------------------------------------------------------------- the handler

fn brief(ctx: &Context, input: BriefInput) -> Result<SessionBrief, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let (standing, _, read) = server::standing_at(&root, &server::local_half(&root));
    let doc = read.document();
    let own = doc.is_some_and(lease::is_own);
    let url = doc.and_then(|d| d.url.clone());
    let answering = matches!(standing, ServerStanding::Ready | ServerStanding::Outdated);

    // The command line (and a plain request to a server that is not the leaseholder) asks
    // the leaseholder, which holds the board and the mesh. An MCP session of a superseded
    // server is that server's own, and is answered there.
    let mut note = None;
    if !own && !input.local && ctx.caller.is_none() {
        if let (true, Some(url)) = (answering, url.as_deref()) {
            match forward(ctx, url, &input) {
                Ok(b) => return Ok(b),
                Err(why) => note = Some(why),
            }
        } else {
            note = Some(format!(
                "no server answers for this checkout (its server is {}); composed by this process, whose board is empty",
                standing.as_str()
            ));
        }
    }
    let superseded = !own && ctx.caller.is_some() && doc.is_some();
    let origin = if own {
        BriefOrigin {
            process: "leaseholder".into(),
            url: url.clone(),
            note: None,
        }
    } else if superseded {
        BriefOrigin {
            process: "superseded".into(),
            url: None,
            note: Some(format!(
                "this server does not hold the lease; the leaseholder is {}",
                url.clone().unwrap_or_else(|| "still binding".into())
            )),
        }
    } else {
        BriefOrigin {
            process: "this_process".into(),
            url: None,
            note,
        }
    };

    let session = session_section(ctx, input.session.as_deref());
    let peers = peers_section(ctx, input.checkouts, superseded.then_some(url).flatten());
    let mesh = mesh_section(ctx);
    let integration = integration_section(&root);
    let release = release_section(ctx);
    let derive = derive_section(&root);
    let interval_minutes = interval_minutes(&root);
    Ok(compose(
        crate::ledger::now(),
        interval_minutes,
        origin,
        session,
        peers,
        mesh,
        integration,
        release,
        derive,
    ))
}

/// Ask the leaseholder over the route the registry declares for this capability.
fn forward(ctx: &Context, url: &str, input: &BriefInput) -> Result<SessionBrief, String> {
    let path = ctx
        .registry
        .get("brief.session")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or_else(|| "brief.session declares no HTTP route to ask over".to_string())?;
    let mut query = format!(
        "{path}?local=true&checkouts={}",
        checkouts_word(input.checkouts)
    );
    if let Some(s) = &input.session {
        query.push_str("&session=");
        query.push_str(&percent_encode(s));
    }
    let reply = crate::mcp::bridge::request(url, "GET", &query, &[], None, FORWARD_TIMEOUT)
        .map_err(|e| {
            format!("the leaseholder at {url} did not answer ({e}); composed by this process")
        })?;
    if reply.status != 200 {
        return Err(format!(
            "the leaseholder at {url} answered {} (a server older than this executable has no brief); composed by this process",
            reply.status
        ));
    }
    serde_json::from_str(&reply.body).map_err(|e| {
        format!("the leaseholder at {url} answered a brief this executable cannot read ({e}); composed by this process")
    })
}

/// The word a [`Checkouts`] is spelled with on every transport.
fn checkouts_word(c: Checkouts) -> &'static str {
    match c {
        Checkouts::Repository => "repository",
        Checkouts::This => "this",
    }
}

/// Percent-encode a query value: everything but the unreserved characters.
fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn session_section(ctx: &Context, provider_session: Option<&str>) -> BriefSessionSection {
    let continuity: Result<Continuity, String> = ctx
        .execute("continuity.state", json!({}))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    let episodes: Result<Episodes, String> = ctx
        .execute("lifecycle.episodes", json!({}))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    let caller = ctx.caller.as_ref().map(|p| p.as_str().to_string());
    let (episode, owner) = match &episodes {
        Ok(e) => {
            let mine = provider_session
                .and_then(|s| e.episodes.iter().find(|ep| ep.provider_session == s))
                .or_else(|| e.episodes.iter().find(|ep| ep.session_id == e.current));
            (
                mine.map(|ep| ep.session_id.clone()),
                mine.map(|ep| ep.provider_session.clone())
                    .filter(|s| !s.is_empty()),
            )
        }
        Err(_) => (None, None),
    };
    let (state, branch, head, task_id, task) = match &continuity {
        Ok(c) => (
            if episodes.is_ok() {
                BriefSectionState::Read
            } else {
                BriefSectionState::Partial
            },
            c.branch.clone(),
            c.head.chars().take(10).collect(),
            c.task.as_ref().map(|t| t.id.clone()),
            c.task.as_ref().map(|t| t.task.clone()),
        ),
        Err(_) => (
            BriefSectionState::Unread,
            String::new(),
            String::new(),
            None,
            None,
        ),
    };
    let summary = match &continuity {
        Err(e) => format!("unread: continuity.state did not answer ({e})"),
        Ok(_) => {
            let mut s = match &episode {
                Some(ep) => format!("episode {ep}"),
                None => match provider_session {
                    Some(p) => format!("no open episode for provider session {p}"),
                    None => "no open episode".into(),
                },
            };
            s.push_str(&format!(" on {branch} at {head}"));
            match (&task_id, &task) {
                (Some(id), Some(t)) => s.push_str(&format!("; task {id}: {}", clip(t, 80))),
                _ => s.push_str("; no task active"),
            }
            if let Some(p) = &caller {
                s.push_str(&format!(
                    "; this connection is {p} on this checkout's board"
                ));
            }
            s
        }
    };
    BriefSessionSection {
        state,
        episode,
        provider_session: owner,
        branch,
        head,
        task_id,
        task,
        caller,
        summary,
    }
}

fn peers_section(
    ctx: &Context,
    checkouts: Checkouts,
    leaseholder: Option<String>,
) -> BriefPeersSection {
    let list: PeerList = match ctx
        .execute(
            "peers.list",
            json!({ "checkouts": checkouts_word(checkouts) }),
        )
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    {
        Ok(l) => l,
        Err(e) => {
            return BriefPeersSection {
                state: BriefSectionState::Unread,
                attached: 0,
                boards: 0,
                unread: Vec::new(),
                overlaps: 0,
                superseded: None,
                named: Vec::new(),
                summary: format!("unread: peers.list did not answer ({e})"),
            }
        }
    };
    let mut peers = list.peers;
    let mut unread: Vec<String> = list
        .boards
        .iter()
        .filter_map(|b| {
            b.reason
                .as_ref()
                .map(|r| format!("{}: {r}", b.checkout.worktree.display()))
        })
        .collect();
    let mut superseded = None;
    if let Some(url) = leaseholder {
        let mine = peers.iter().filter(|p| p.attached).count();
        superseded = Some(format!(
            "this server is superseded by {url}: the {mine} peer(s) attached here are on a board the repository's gather cannot reach"
        ));
        match super::peers::board_of(ctx, &url) {
            Ok(theirs) => {
                let stamp = peers.iter().find_map(|p| p.checkout.clone());
                peers.extend(theirs.into_iter().map(|mut p| {
                    p.checkout = stamp.clone();
                    p
                }))
            }
            Err(e) => unread.push(format!("the leaseholder's board: {e}")),
        }
    }
    let attached: Vec<&Peer> = peers.iter().filter(|p| p.attached).collect();
    let named: Vec<String> = attached
        .iter()
        .take(PEERS_NAMED)
        .map(|p| peer_line(p))
        .collect();
    let state = if unread.is_empty() && superseded.is_none() {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let mut summary = format!("{} attached on {} board(s)", attached.len(), list.covered);
    if !list.overlaps.is_empty() {
        summary.push_str(&format!(
            "; {} overlapping claim(s): majordomus_peers",
            list.overlaps.len()
        ));
    }
    if !unread.is_empty() {
        summary.push_str(&format!("; {} board(s) unread", unread.len()));
    }
    if let Some(s) = &superseded {
        summary.push_str(&format!("; {s}"));
    }
    if attached.len() > PEERS_NAMED {
        summary.push_str(&format!(
            "; {} more not named",
            attached.len() - PEERS_NAMED
        ));
    }
    BriefPeersSection {
        state,
        attached: attached.len(),
        // every checkout covered, listed or counted: the summary lists only the ones that
        // hold a board (peers::PeerView), and a checkout with none is still covered
        boards: list.covered,
        unread,
        overlaps: list.overlaps.len(),
        superseded,
        named,
        summary,
    }
}

/// `p3 claude-code @feature/x: what it announced`.
fn peer_line(p: &Peer) -> String {
    let branch = p
        .checkout
        .as_ref()
        .and_then(|c| c.branch.clone())
        .unwrap_or_else(|| "?".into());
    let intent = p
        .announcement
        .as_ref()
        .map(|a| clip(&a.intent, INTENT_CHARS))
        .unwrap_or_else(|| "announced nothing".into());
    format!("{} {} @{branch}: {intent}", p.id.as_str(), p.client.name)
}

fn mesh_section(ctx: &Context) -> BriefMeshSection {
    use crate::mesh::cooperation::LinkState;
    use crate::mesh::state::{ClaimState, ReviewState};
    let status = ctx.mesh.cooperation_status();
    let mut section = BriefMeshSection {
        state: BriefSectionState::Undeclared,
        active: status.active,
        reason: status.reason.clone(),
        links: status.peers.len(),
        connected: 0,
        failing: 0,
        claims_held: 0,
        own_claims: 0,
        reviews_for_you: 0,
        handovers_waiting: 0,
        overlaps_for_you: 0,
        summary: String::new(),
    };
    let Some(cooperation) = ctx.mesh.cooperation() else {
        section.summary = format!(
            "not active here: {}",
            status
                .reason
                .as_deref()
                .unwrap_or("cooperation is not running in this process")
        );
        return section;
    };
    section.state = BriefSectionState::Read;
    section.connected = status
        .peers
        .iter()
        .filter(|p| matches!(p.state, LinkState::Connected))
        .count();
    section.failing = status
        .peers
        .iter()
        .filter(|p| {
            matches!(
                p.state,
                LinkState::Degraded | LinkState::Unreachable | LinkState::Expired
            )
        })
        .count();
    let runtime = status.runtime.clone().unwrap_or_default();
    let state = cooperation.state();
    let held: Vec<_> = state
        .claims
        .iter()
        .filter(|c| matches!(c.state, ClaimState::Held))
        .collect();
    section.claims_held = held.len();
    section.own_claims = held.iter().filter(|c| c.runtime == runtime).count();
    section.reviews_for_you = state
        .reviews
        .iter()
        .filter(|r| {
            matches!(r.state, ReviewState::Open) && r.reviewer.as_deref() == Some(runtime.as_str())
        })
        .count();
    section.handovers_waiting = state
        .handovers
        .iter()
        .filter(|h| h.runtime != runtime && h.consumed_by.is_empty())
        .count();
    let own_keys: Vec<&str> = held
        .iter()
        .filter(|c| c.runtime == runtime)
        .map(|c| c.key.as_str())
        .collect();
    section.overlaps_for_you = state
        .overlaps
        .iter()
        .filter(|o| own_keys.contains(&o.first.as_str()) || own_keys.contains(&o.second.as_str()))
        .count();
    let mut s = format!(
        "{} of {} link(s) connected",
        section.connected, section.links
    );
    if section.failing > 0 {
        s.push_str(&format!(", {} failing", section.failing));
    }
    s.push_str(&format!(
        "; {} claim(s) held, {} of them this runtime's",
        section.claims_held, section.own_claims
    ));
    let addressed = section.reviews_for_you + section.handovers_waiting + section.overlaps_for_you;
    if addressed == 0 {
        s.push_str("; nothing addressed to this runtime");
    } else {
        s.push_str(&format!(
            "; for this runtime: {} review(s), {} handover(s) waiting, {} overlap(s)",
            section.reviews_for_you, section.handovers_waiting, section.overlaps_for_you
        ));
    }
    section.summary = s;
    section
}

fn integration_section(root: &Path) -> BriefIntegrationSection {
    match crate::commands::prs::brief(root) {
        Some(line) => BriefIntegrationSection {
            state: BriefSectionState::Read,
            summary: line,
        },
        None => BriefIntegrationSection {
            state: BriefSectionState::Undeclared,
            summary: "this checkout never observed the forge: majordomus prs status".into(),
        },
    }
}

/// Whether the trunk is released (`release.currency`, I3070), without the version: a brief is
/// composed at every session's start, and the version reads the last release's registry.
fn release_section(ctx: &Context) -> BriefReleaseSection {
    let answer: Result<crate::release::currency::Currency, String> = ctx
        .execute(super::release::CURRENCY_ID, json!({ "next": false }))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    match answer {
        Ok(a) => BriefReleaseSection {
            state: match a.stance {
                crate::release::currency::Stance::Unknown => BriefSectionState::Unread,
                _ => BriefSectionState::Read,
            },
            stance: Some(a.stance),
            landed: a.landed,
            summary: a.summary,
        },
        Err(why) => BriefReleaseSection {
            summary: format!("release.currency did not answer: {why}"),
            ..BriefReleaseSection::default()
        },
    }
}

/// The derive lock and its priority marker, where `lib/machine_lock.sh` puts them: beside the
/// primary checkout, or where `MAJORDOMUS_DERIVE_LOCK` and `MAJORDOMUS_DERIVE_PRIORITY` say.
fn derive_section(root: &Path) -> BriefDeriveSection {
    let declared = root.join("lib/machine_lock.sh").is_file();
    let lock = std::env::var_os("MAJORDOMUS_DERIVE_LOCK")
        .map(PathBuf::from)
        .or_else(|| primary_home(root).map(|h| h.join(".derive.lock")));
    let priority = std::env::var_os("MAJORDOMUS_DERIVE_PRIORITY")
        .map(PathBuf::from)
        .or_else(|| {
            lock.as_ref()
                .and_then(|l| l.parent().map(|p| p.join(".derive.priority")))
        });
    derive_from(declared, lock.as_deref(), priority.as_deref())
}

/// The directory holding the primary checkout, found from git's common directory.
fn primary_home(root: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    // the common directory's parent's parent: the directory the primary checkout sits in
    common.ancestors().nth(2).map(Path::to_path_buf)
}

/// The derive section from the files alone: kept apart from where they are found so that a
/// test can hand it a lock of its own.
fn derive_from(declared: bool, lock: Option<&Path>, priority: Option<&Path>) -> BriefDeriveSection {
    let mut section = BriefDeriveSection {
        state: BriefSectionState::Undeclared,
        holder: None,
        pid: None,
        alive: None,
        held_seconds: None,
        priority: None,
        summary: "the repository declares no derive lock".into(),
    };
    if !declared {
        return section;
    }
    section.state = BriefSectionState::Read;
    section.priority = priority.filter(|p| p.exists()).map(|p| {
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    });
    let owner = lock.map(|l| l.join("owner"));
    match owner.as_deref().map(std::fs::read_to_string) {
        Some(Ok(line)) => {
            let line = line.trim().to_string();
            section.pid = owner_pid(&line);
            section.alive = section.pid.map(lease::alive);
            section.held_seconds = lock
                .and_then(|l| std::fs::metadata(l).ok())
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs());
            let who = line.split_whitespace().next().unwrap_or("?").to_string();
            let mut s = format!("lock held by {who}");
            match (section.pid, section.alive) {
                (Some(pid), Some(true)) => s.push_str(&format!(" (pid {pid}, running")),
                (Some(pid), _) => s.push_str(&format!(
                    " (pid {pid}, NOT running: the next derive reclaims it"
                )),
                (None, _) => s.push_str(" (no pid in its line: nobody can check it"),
            }
            if let Some(secs) = section.held_seconds {
                s.push_str(&format!(", {} min", secs / 60));
            }
            s.push(')');
            section.holder = Some(line);
            section.summary = s;
        }
        _ => section.summary = "lock free".into(),
    }
    if let Some(p) = &section.priority {
        section.summary.push_str(&format!(
            "; priority to {}",
            if p.is_empty() { "?" } else { p }
        ));
    }
    section
}

/// The holder's pid: the line's `pid=<n>`, else its last word, only when it is a number —
/// the reading `lib/machine_lock.sh` gives the same line.
fn owner_pid(line: &str) -> Option<u32> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix("pid=").and_then(|n| n.parse().ok()))
        .or_else(|| line.split_whitespace().last().and_then(|w| w.parse().ok()))
}

/// `session.brief_interval_minutes`, or the default when the policy declares none or
/// cannot be read.
fn interval_minutes(root: &Path) -> u64 {
    crate::repository::Repository::open(root)
        .ok()
        .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
        .and_then(|p| p.policy.session.brief_interval_minutes)
        .filter(|m| *m > 0)
        .unwrap_or(DEFAULT_INTERVAL_MINUTES)
}

/// What a session is told at its start about keeping itself awake.
pub(crate) fn keep_awake(interval: u64) -> String {
    format!(
        "Keep yourself awake and reporting (ADR 0136): every {interval} minutes, without being asked, \
         give a brief report of two to four lines (this session's task and state, the peers of \
         every board, the mesh, the integration queue, the derive queue) read from `majordomus \
         brief`, never from memory. If your client can schedule its own prompts (Claude Code: \
         `/loop {interval}m majordomus brief`, or a session cron), schedule that now. Where the \
         majordomus-brief mod is loaded it shows the one-line brief as the status line and puts \
         the full brief in front of you on the same interval."
    )
}

/// The brief from its sections: the line, the lines and the keep-awake text.
#[allow(clippy::too_many_arguments)]
fn compose(
    at: String,
    interval_minutes: u64,
    answered_by: BriefOrigin,
    session: BriefSessionSection,
    peers: BriefPeersSection,
    mesh: BriefMeshSection,
    integration: BriefIntegrationSection,
    release: BriefReleaseSection,
    derive: BriefDeriveSection,
) -> SessionBrief {
    let line = one_line(&session, &peers, &mesh, &integration, &release, &derive);
    let mut lines = vec![
        format!("session      {}", session.summary),
        format!("peers        {}", peers.summary),
    ];
    lines.extend(peers.named.iter().map(|n| format!("             {n}")));
    lines.push(format!("mesh         {}", mesh.summary));
    lines.push(format!("integration  {}", integration.summary));
    lines.push(format!("release      {}", release.summary));
    lines.push(format!("derive       {}", derive.summary));
    if let Some(n) = &answered_by.note {
        lines.push(format!("answered by  {} ({n})", answered_by.process));
    }
    SessionBrief {
        at,
        line,
        lines,
        interval_minutes,
        keep_awake: keep_awake(interval_minutes),
        answered_by,
        session,
        peers,
        mesh,
        integration,
        release,
        derive,
    }
}

/// The one line: a few words per section, every section present whatever its state.
fn one_line(
    session: &BriefSessionSection,
    peers: &BriefPeersSection,
    mesh: &BriefMeshSection,
    integration: &BriefIntegrationSection,
    release: &BriefReleaseSection,
    derive: &BriefDeriveSection,
) -> String {
    let who = match (&session.task_id, &session.episode) {
        (Some(t), _) => task_label(t, session.task.as_deref().unwrap_or_default()),
        (None, Some(e)) => e.clone(),
        (None, None) => "no task".into(),
    };
    let s = if session.state == BriefSectionState::Unread {
        "session ?".to_string()
    } else {
        format!("{who} @{}", session.branch)
    };
    let p = match peers.state {
        BriefSectionState::Unread => "peers ?".to_string(),
        _ => {
            let mut p = format!("peers {}/{}b", peers.attached, peers.boards);
            if !peers.unread.is_empty() {
                p.push_str(&format!(" ({} unread)", peers.unread.len()));
            }
            if peers.superseded.is_some() {
                p.push_str(" SUPERSEDED");
            }
            p
        }
    };
    let m = match mesh.state {
        BriefSectionState::Read => {
            let mut m = format!("mesh {}/{}", mesh.connected, mesh.links);
            let addressed = mesh.reviews_for_you + mesh.handovers_waiting + mesh.overlaps_for_you;
            if addressed > 0 {
                m.push_str(&format!(" +{addressed} for you"));
            }
            m
        }
        _ => "mesh off".into(),
    };
    let i = match integration.state {
        BriefSectionState::Read => format!("prs {}", prs_words(&integration.summary)),
        _ => "prs ?".into(),
    };
    let d = match (&derive.state, &derive.holder) {
        (BriefSectionState::Undeclared, _) => None,
        (_, None) => Some("derive free".to_string()),
        (_, Some(h)) => Some(format!(
            "derive {}{}",
            clip(h.split_whitespace().next().unwrap_or("?"), 28),
            derive
                .held_seconds
                .map(|s| format!(" {}m", s / 60))
                .unwrap_or_default()
        )),
    };
    let r = match (release.state, release.stance) {
        (BriefSectionState::Read, Some(crate::release::currency::Stance::Owed)) => {
            format!("release owed {}", release.landed)
        }
        (BriefSectionState::Read, Some(crate::release::currency::Stance::Overdue)) => {
            format!("release OVERDUE {}", release.landed)
        }
        (BriefSectionState::Read, _) => "release current".into(),
        _ => "release ?".into(),
    };
    let mut parts = vec![s, p, m, i, r];
    parts.extend(d);
    parts.join(" | ")
}

/// A task as the line names it: the issue its words open with (`I2399: ...`), else its id.
fn task_label(id: &str, words: &str) -> String {
    let first = words.split([':', ' ']).next().unwrap_or_default();
    let is_issue =
        first.len() > 1 && first.starts_with('I') && first[1..].chars().all(|c| c.is_ascii_digit());
    if is_issue {
        first.to_string()
    } else {
        id.to_string()
    }
}

/// The integration queue in a few words, from the sentence `prs brief` writes: its open count
/// and its next merge.
fn prs_words(summary: &str) -> String {
    let open = summary
        .split(": ")
        .nth(1)
        .and_then(|rest| rest.split(" open").next())
        .and_then(|n| n.trim().parse::<usize>().ok());
    let next = summary
        .split("next merge #")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .filter(|n| !n.is_empty());
    match (open, next) {
        (Some(o), Some(n)) => format!("{o} open, next #{n}"),
        (Some(o), None) => format!("{o} open"),
        _ => clip(summary, 40),
    }
}

/// The first `n` characters, with an ellipsis when something was cut.
fn clip(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    }
}

/// The section's state word, for a renderer.
pub(crate) fn state_word(state: BriefSectionState) -> &'static str {
    state.as_str()
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "brief",
        title: "The session brief",
        description: "What surrounds one working session, in one line and in a few: its episode and task, the peers of every board, the mesh, the integration queue and the derive queue, each section asked of its owner once and saying whether it could be read. The answer a session reports on itself from without being asked, and the one a provider mod shows as its status line (ADR 0136). Writes nothing.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "brief.session",
                title: "The session brief",
                description: "One line and a few: this session's episode and task (the one the given provider session owns, else the pointer's), the peers of every board with the boards that could not be read named, the mesh's links and what its journal addresses to this runtime, the integration queue as `majordomus prs brief` says it, and the machine's derive lock. Composed by the server holding this checkout's lease, which holds the board and the mesh: the command line asks it and composes the brief itself only when none answers, saying so; a superseded server says its own board is unreachable by the repository's gather. Carries the reporting interval the policy declares (`session.brief_interval_minutes`) and the keep-awake text a session is told at its start. Writes nothing.",
                input: BriefInput,
                output: SessionBrief,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_brief"),
                    http: get("/api/v1/brief"),
                    cli: Some(CliExposure { path: vec!["brief".into()] }),
                },
                tags: ["session", "peers", "coordination", "brief"],
                // every source carries its own cache policy, and the board is live memory
                cache: CachePolicy::Disabled,
                handler: brief,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::currency::Stance;

    fn session(task: Option<&str>) -> BriefSessionSection {
        BriefSessionSection {
            state: BriefSectionState::Read,
            episode: Some("s-1".into()),
            provider_session: None,
            branch: "feature/x".into(),
            head: "abc1234567".into(),
            task_id: task.map(str::to_string),
            task: task.map(|_| "do the thing".to_string()),
            caller: None,
            summary: "episode s-1 on feature/x at abc1234567; task I1: do the thing".into(),
        }
    }

    fn peers(unread: usize, superseded: bool) -> BriefPeersSection {
        BriefPeersSection {
            state: if unread > 0 || superseded {
                BriefSectionState::Partial
            } else {
                BriefSectionState::Read
            },
            attached: 3,
            boards: 5,
            unread: (0..unread)
                .map(|i| format!("/wt/{i}: did not answer"))
                .collect(),
            overlaps: 0,
            superseded: superseded.then(|| "this server is superseded by x".to_string()),
            named: vec!["p1 claude-code @master: integrating".into()],
            summary: "3 attached on 5 board(s)".into(),
        }
    }

    fn mesh_off() -> BriefMeshSection {
        BriefMeshSection {
            state: BriefSectionState::Undeclared,
            active: false,
            reason: Some("disabled".into()),
            links: 0,
            connected: 0,
            failing: 0,
            claims_held: 0,
            own_claims: 0,
            reviews_for_you: 0,
            handovers_waiting: 0,
            overlaps_for_you: 0,
            summary: "not active here: disabled".into(),
        }
    }

    fn origin() -> BriefOrigin {
        BriefOrigin {
            process: "leaseholder".into(),
            url: Some("http://127.0.0.1:1".into()),
            note: None,
        }
    }

    #[test]
    fn every_section_is_in_the_line_and_the_lines() {
        let integration = BriefIntegrationSection {
            state: BriefSectionState::Read,
            summary: "master at abc observed 3 min ago (t): 52 open — ready 1; next merge #902; lease free".into(),
        };
        let derive = derive_from(false, None, None);
        let b = compose(
            "t".into(),
            20,
            origin(),
            session(Some("I2399")),
            peers(0, false),
            mesh_off(),
            integration,
            release(Stance::Owed, 2),
            derive,
        );
        assert_eq!(
            b.line,
            "I2399 @feature/x | peers 3/5b | mesh off | prs 52 open, next #902 | release owed 2"
        );
        assert!(b
            .lines
            .iter()
            .any(|l| l.starts_with("release      2 landed")));
        assert!(b.lines[0].starts_with("session      episode s-1"));
        assert!(b.lines[1].starts_with("peers        3 attached"));
        assert!(b.lines[2].contains("p1 claude-code @master"));
        assert!(b
            .lines
            .iter()
            .any(|l| l.starts_with("mesh         not active")));
        assert!(b.lines.iter().any(|l| l.starts_with("integration  master")));
        assert!(b
            .lines
            .iter()
            .any(|l| l == "derive       the repository declares no derive lock"));
        assert!(b.keep_awake.contains("every 20 minutes"));
        assert!(b.keep_awake.contains("/loop 20m majordomus brief"));
    }

    #[test]
    fn an_unread_board_and_a_superseded_server_are_said_in_the_line() {
        let integration = BriefIntegrationSection {
            state: BriefSectionState::Undeclared,
            summary: "never observed".into(),
        };
        let b = compose(
            "t".into(),
            5,
            BriefOrigin {
                process: "this_process".into(),
                url: None,
                note: Some("no server answers".into()),
            },
            session(None),
            peers(2, true),
            mesh_off(),
            integration,
            BriefReleaseSection::default(),
            derive_from(false, None, None),
        );
        assert!(b.line.contains("release ?"), "{}", b.line);
        assert!(b
            .line
            .starts_with("s-1 @feature/x | peers 3/5b (2 unread) SUPERSEDED"));
        assert!(b.line.contains("prs ?"));
        assert!(b
            .lines
            .last()
            .is_some_and(|l| l.starts_with("answered by  this_process (no server answers)")));
    }

    #[test]
    fn the_derive_lock_is_read_as_the_machine_lock_writes_it() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let lock = dir.path().join(".derive.lock");
        let priority = dir.path().join(".derive.priority");
        // free
        let free = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(free.state, BriefSectionState::Read);
        assert_eq!(free.summary, "lock free");
        assert!(free.holder.is_none());
        // held by this very process: running
        std::fs::create_dir(&lock).expect("the lock directory");
        let me = std::process::id();
        std::fs::write(
            lock.join("owner"),
            format!("derive:feature/x pid={me} branch=feature/x\n"),
        )
        .expect("the owner line");
        std::fs::write(&priority, "release/v1\n").expect("the marker");
        let held = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(held.pid, Some(me));
        assert_eq!(held.alive, Some(true));
        assert!(held
            .summary
            .starts_with("lock held by derive:feature/x (pid"));
        assert!(held.summary.contains("running"));
        assert!(held.summary.ends_with("; priority to release/v1"));
        // a line whose last word is the pid, as `<who>:<branch> <pid>` is written
        assert_eq!(owner_pid("derive:feature/x 4242"), Some(4242));
        // a line with no numeric pid is a holder nobody can check
        assert_eq!(owner_pid("someone at 10:00"), None);
        std::fs::write(lock.join("owner"), "someone by hand\n").expect("the owner line");
        let unchecked = derive_from(true, Some(&lock), None);
        assert!(unchecked.summary.contains("no pid in its line"));
        // the line names the holder and the age
        let ctx_line = one_line(
            &session(None),
            &peers(0, false),
            &mesh_off(),
            &BriefIntegrationSection {
                state: BriefSectionState::Undeclared,
                summary: String::new(),
            },
            &BriefReleaseSection::default(),
            &held,
        );
        assert!(
            ctx_line.ends_with("| derive derive:feature/x 0m"),
            "{ctx_line}"
        );
    }

    #[test]
    fn the_prs_words_keep_the_count_and_the_next_merge() {
        assert_eq!(
            prs_words("master at abc observed 1 min ago (t): 7 open — ready 2; next merge #12; lease free"),
            "7 open, next #12"
        );
        assert_eq!(
            prs_words("master at abc observed 1 min ago (t): 7 open — ready 0; nothing ready"),
            "7 open"
        );
        assert_eq!(
            prs_words("observed x at t; no queue built since"),
            "observed x at t; no queue built since"
        );
    }

    #[test]
    fn the_line_names_a_task_by_its_issue_when_its_words_open_with_one() {
        assert_eq!(task_label("t-1", "I2399: do it"), "I2399");
        assert_eq!(task_label("t-1", "I12 do it"), "I12");
        assert_eq!(task_label("t-1", "Improve things"), "t-1");
        assert_eq!(task_label("t-1", ""), "t-1");
    }

    #[test]
    fn clip_and_encode() {
        assert_eq!(clip("a  b\nc", 10), "a b c");
        assert_eq!(clip("abcdefghij", 5), "abcd…");
        assert_eq!(percent_encode("a b/c-d_e.f~"), "a%20b%2Fc-d_e.f~");
        assert_eq!(state_word(BriefSectionState::Partial), "partial");
        assert!(keep_awake(7).contains("every 7 minutes"));
    }

    #[test]
    fn the_input_defaults_to_every_board_and_forwarding() {
        let asked: BriefInput = serde_json::from_str("{}").expect("an empty object");
        assert_eq!(asked.checkouts, Checkouts::Repository);
        assert!(!asked.local);
        assert!(asked.session.is_none());
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "brief");
        let c = &m.capabilities[0].capability;
        assert_eq!(c.id.as_str(), "brief.session");
        assert_eq!(
            c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_brief")
        );
        assert_eq!(
            c.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/brief")
        );
        assert_eq!(
            c.exposure.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["brief".to_string()])
        );
    }

    // ------------------------------------------------ owners that answer what a test says

    use crate::peers::Transport;
    use crate::synthetic::SyntheticRepository;
    use std::io::{Read as _, Write as _};

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct Anything {}

    impl BenchmarkCases for Anything {
        fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
            vec![NamedCase::new("default", Anything {})]
        }
    }

    thread_local! {
        /// What each stand-in owner answers on this test's thread; an owner with nothing
        /// here refuses.
        static CANNED: std::cell::RefCell<std::collections::BTreeMap<&'static str, serde_json::Value>> =
            Default::default();
    }

    fn can(id: &'static str, answer: serde_json::Value) {
        CANNED.with(|c| {
            c.borrow_mut().insert(id, answer);
        });
    }

    fn canned(id: &str) -> Result<serde_json::Value, CapabilityError> {
        CANNED
            .with(|c| c.borrow().get(id).cloned())
            .ok_or_else(|| CapabilityError::Refused(format!("{id} does not answer")))
    }

    fn canned_continuity(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        canned("continuity.state")
    }

    fn canned_episodes(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        canned("lifecycle.episodes")
    }

    fn canned_peers(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        canned("peers.list")
    }

    /// A context whose registry holds the brief and stand-ins for the three owners it asks
    /// through the registry, the board's with the HTTP route a leaseholder is asked over.
    fn canned_context(repo: &SyntheticRepository) -> Context {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![module()])
            .with_builtin(vec![
                capability! {
                    id: "continuity.state", title: "Continuity", description: "Canned.",
                    input: Anything, output: serde_json::Value,
                    stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
                    handler: canned_continuity,
                },
                capability! {
                    id: "lifecycle.episodes", title: "Episodes", description: "Canned.",
                    input: Anything, output: serde_json::Value,
                    stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
                    handler: canned_episodes,
                },
                capability! {
                    id: "peers.list", title: "Peers", description: "Canned.",
                    input: Anything, output: serde_json::Value,
                    stability: Stability::Experimental,
                    exposure: Exposure { http: get("/api/v1/peers"), ..Exposure::default() },
                    tags: [],
                    handler: canned_peers,
                },
            ])
            .build()
            .expect("a registry of stand-ins");
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        )
    }

    fn bare_context(repo: &SyntheticRepository) -> Context {
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .build()
                    .expect("an empty registry"),
            ),
        )
    }

    fn continuity_with_task() -> serde_json::Value {
        json!({
            "present": true,
            "worktree": "/w",
            "branch": "feature/x",
            "head": "0123456789abcdef",
            "task": {
                "id": "t-1",
                "task": "I2399: cover the brief",
                "profile": "implementation",
                "outcome": "active",
            },
            "tallies": {},
        })
    }

    fn two_episodes() -> serde_json::Value {
        json!({
            "present": true,
            "store": ".ai/local/state/sessions-open",
            "worktree": "/w",
            "current": "s-0001",
            "episodes": [
                {"session_id": "s-0001", "path": "a.yaml", "standing": "current",
                 "events": 1, "note": "the pointer resolves to it"},
                {"session_id": "s-0002", "path": "b.yaml", "provider": "claude-code",
                 "provider_session": "prov-2", "standing": "open", "events": 3,
                 "note": "this worktree, not pointed at"},
            ],
        })
    }

    /// A loopback HTTP server that answers each request by its path, for the rest of the
    /// test binary's life; its URL.
    fn serve(answer: impl Fn(&str) -> (u16, String) + Send + 'static) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&head);
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = answer(&path);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        url
    }

    /// An address nothing listens on.
    fn dead_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        drop(listener);
        url
    }

    /// The canonical root the handler reads its lease under.
    fn canonical(repo: &SyntheticRepository) -> PathBuf {
        repo.root()
            .canonicalize()
            .unwrap_or_else(|_| repo.root().to_path_buf())
    }

    /// Write a lease of another process into the checkout, naming `url`.
    fn lease_of_another(repo: &SyntheticRepository, url: Option<&str>) {
        let root = canonical(repo);
        let file = lease::lease_file(&root, &server::local_half(&root));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            json!({
                "schema": lease::SCHEMA,
                "pid": 0,
                "token": "another-process",
                "root": root,
                "url": url,
                "started_at": "2026-10-11T00:00:00Z",
            })
            .to_string(),
        )
        .unwrap();
    }

    fn input(checkouts: Checkouts, session: Option<&str>) -> BriefInput {
        BriefInput {
            session: session.map(str::to_string),
            checkouts,
            local: false,
        }
    }

    // ------------------------------------------------ the session section

    #[test]
    fn the_session_is_the_episode_its_provider_session_owns_else_the_pointers() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("continuity.state", continuity_with_task());
        can("lifecycle.episodes", two_episodes());

        let me = ctx.peers.attach(Transport::Http);
        let mine = session_section(&ctx.for_caller(me.clone()), Some("prov-2"));
        assert_eq!(mine.state, BriefSectionState::Read);
        assert_eq!(mine.episode.as_deref(), Some("s-0002"));
        assert_eq!(mine.provider_session.as_deref(), Some("prov-2"));
        assert_eq!(mine.head, "0123456789", "the head, abbreviated");
        assert_eq!(mine.task_id.as_deref(), Some("t-1"));
        assert_eq!(
            mine.summary,
            format!(
                "episode s-0002 on feature/x at 0123456789; task t-1: I2399: cover the brief; \
                 this connection is {} on this checkout's board",
                me.as_str()
            )
        );

        // no provider session: the pointer's episode, which no provider session owns
        let pointed = session_section(&ctx, None);
        assert_eq!(pointed.episode.as_deref(), Some("s-0001"));
        assert_eq!(pointed.provider_session, None, "an empty owner is no owner");
        assert!(pointed.caller.is_none());
        assert!(pointed.summary.starts_with("episode s-0001 on feature/x"));
    }

    #[test]
    fn a_session_whose_episodes_cannot_be_read_is_partial_and_one_without_continuity_unread() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("continuity.state", continuity_with_task());
        // an answer that is not the episodes' shape
        can("lifecycle.episodes", json!({ "not": "episodes" }));
        let partial = session_section(&ctx, Some("prov-2"));
        assert_eq!(partial.state, BriefSectionState::Partial);
        assert_eq!(partial.episode, None);
        assert!(
            partial
                .summary
                .starts_with("no open episode for provider session prov-2 on feature/x"),
            "{}",
            partial.summary
        );

        // continuity that answers in a shape nobody can read is no continuity
        can("continuity.state", json!({ "not": "continuity" }));
        let unread = session_section(&ctx, None);
        assert_eq!(unread.state, BriefSectionState::Unread);
        assert!(unread.branch.is_empty() && unread.head.is_empty());
        assert!(unread.task_id.is_none() && unread.task.is_none());
        assert!(
            unread
                .summary
                .starts_with("unread: continuity.state did not answer ("),
            "{}",
            unread.summary
        );

        // and owners that are not there at all
        let bare = bare_context(&repo);
        let nobody = session_section(&bare, None);
        assert_eq!(nobody.state, BriefSectionState::Unread);
        assert!(
            nobody.summary.contains("continuity.state"),
            "{}",
            nobody.summary
        );
    }

    // ------------------------------------------------ the peers section

    /// The board of a real `peers.list`: nine workers, two of whom claim scopes that meet.
    fn nine_on_a_board() -> serde_json::Value {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        for i in 0..9 {
            let p = ctx.peers.attach(Transport::Http);
            let scope = match i {
                0 => "src/cockpit".to_string(),
                1 => "src/cockpit/pages.rs".to_string(),
                _ => format!("docs/{i}.md"),
            };
            ctx.peers
                .announce(&p, &format!("worker {i} at work"), vec![scope]);
        }
        ctx.execute("peers.list", json!({ "checkouts": "this" }))
            .expect("the board")
    }

    #[test]
    fn the_peers_section_names_eight_counts_the_rest_and_says_what_it_could_not_read() {
        let mut list = nine_on_a_board();
        let mut unread = list["boards"][0].clone();
        unread["reason"] = json!("its server is starting and has no board to read yet");
        // a board carries its checkout's fields beside its own
        let at = if unread.get("checkout").is_some() {
            &mut unread["checkout"]
        } else {
            &mut unread
        };
        at["worktree"] = json!("/elsewhere");
        at["this_checkout"] = json!(false);
        list["boards"].as_array_mut().unwrap().push(unread);
        list["covered"] = json!(2);

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("peers.list", list);
        let s = peers_section(&ctx, Checkouts::Repository, None);
        assert_eq!(s.state, BriefSectionState::Partial);
        assert_eq!((s.attached, s.boards, s.overlaps), (9, 2, 1));
        assert_eq!(
            s.unread,
            ["/elsewhere: its server is starting and has no board to read yet"]
        );
        assert_eq!(s.named.len(), PEERS_NAMED);
        assert!(
            s.named[0].ends_with("@?: worker 0 at work"),
            "a checkout with no branch is named `?`: {}",
            s.named[0]
        );
        assert_eq!(
            s.summary,
            "9 attached on 2 board(s); 1 overlapping claim(s): majordomus_peers; \
             1 board(s) unread; 1 more not named"
        );
    }

    #[test]
    fn a_board_nobody_answers_for_is_unread() {
        let repo = SyntheticRepository::small().expect("a repository");
        let bare = bare_context(&repo);
        let s = peers_section(&bare, Checkouts::This, None);
        assert_eq!(s.state, BriefSectionState::Unread);
        assert_eq!((s.attached, s.boards, s.overlaps), (0, 0, 0));
        assert!(s.unread.is_empty() && s.named.is_empty() && s.superseded.is_none());
        assert!(
            s.summary.starts_with("unread: peers.list did not answer ("),
            "{}",
            s.summary
        );

        // an answer in a shape that is not a board is unread too
        let ctx = canned_context(&repo);
        can("peers.list", json!({ "peers": "nobody" }));
        let s = peers_section(&ctx, Checkouts::This, None);
        assert_eq!(s.state, BriefSectionState::Unread);
    }

    // ------------------------------------------------ a superseded server

    /// A server that lost the lease answers its own MCP sessions: it says so, adds the
    /// leaseholder's board to its own, and names whose board it could not reach.
    #[test]
    fn a_superseded_server_adds_the_leaseholders_board_and_says_it_is_superseded() {
        let leaseholder = {
            let repo = SyntheticRepository::small().expect("a repository");
            let ctx = repo.context().expect("a context");
            let p = ctx.peers.attach(Transport::Http);
            ctx.peers
                .announce(&p, "the leaseholder's worker", vec!["lib".into()]);
            serde_json::to_value(ctx.peers.list()).expect("its board")
        };
        let board = json!({ "peers": leaseholder }).to_string();
        let url = serve(move |path| {
            if path.starts_with("/api/v1/peers") {
                (200, board.clone())
            } else {
                (404, "{}".into())
            }
        });

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = canned_context(&repo);
        can("continuity.state", continuity_with_task());
        can("lifecycle.episodes", two_episodes());
        // this server's own board: one worker, read from its memory
        let mut own = nine_on_a_board();
        let first = own["peers"][0].clone();
        own["peers"] = json!([first]);
        own["overlaps"] = json!([]);
        can("peers.list", own);
        lease_of_another(&repo, Some(&url));

        let me = ctx.peers.attach(Transport::Http);
        let b = brief(&ctx.for_caller(me), input(Checkouts::This, None)).expect("a brief");
        assert_eq!(b.answered_by.process, "superseded");
        assert_eq!(b.answered_by.url, None);
        assert_eq!(
            b.answered_by.note.as_deref(),
            Some(format!("this server does not hold the lease; the leaseholder is {url}").as_str())
        );
        assert_eq!(b.peers.state, BriefSectionState::Partial);
        assert_eq!(b.peers.attached, 2, "its own worker and the leaseholder's");
        assert!(
            b.peers
                .named
                .iter()
                .any(|n| n.contains("the leaseholder's worker")),
            "{:?}",
            b.peers.named
        );
        let said = b.peers.superseded.clone().expect("superseded");
        assert!(said.contains(&format!("superseded by {url}")), "{said}");
        assert!(said.contains("the 1 peer(s) attached here"), "{said}");
        assert!(
            b.peers.summary.ends_with(&format!("; {said}")),
            "{}",
            b.peers.summary
        );
        assert!(b.line.contains(" SUPERSEDED"), "{}", b.line);
        assert!(b.line.starts_with("I2399 @feature/x"), "{}", b.line);

        // a leaseholder that does not answer: its board is named unread
        lease_of_another(&repo, Some(&dead_url()));
        let b = brief(
            &ctx.for_caller(ctx.peers.attach(Transport::Http)),
            input(Checkouts::This, None),
        )
        .expect("a brief");
        assert_eq!(b.peers.unread.len(), 1, "{:?}", b.peers.unread);
        assert!(
            b.peers.unread[0].starts_with("the leaseholder's board: "),
            "{:?}",
            b.peers.unread
        );
        assert!(b.peers.summary.contains("; 1 board(s) unread"));
        assert!(b.line.contains("(1 unread) SUPERSEDED"), "{}", b.line);

        // a leaseholder still binding: no address to name, and no board to ask
        lease_of_another(&repo, None);
        let b = brief(
            &ctx.for_caller(ctx.peers.attach(Transport::Http)),
            input(Checkouts::This, None),
        )
        .expect("a brief");
        assert_eq!(b.answered_by.process, "superseded");
        assert_eq!(
            b.answered_by.note.as_deref(),
            Some("this server does not hold the lease; the leaseholder is still binding")
        );
        assert_eq!(b.peers.superseded, None);
    }

    // ------------------------------------------------ asking the leaseholder

    fn a_brief() -> SessionBrief {
        compose(
            "t".into(),
            20,
            origin(),
            session(None),
            peers(0, false),
            mesh_off(),
            BriefIntegrationSection {
                state: BriefSectionState::Undeclared,
                summary: "never observed".into(),
            },
            BriefReleaseSection::default(),
            derive_from(false, None, None),
        )
    }

    /// A release section as `release.currency` would make it.
    fn release(stance: Stance, landed: usize) -> BriefReleaseSection {
        BriefReleaseSection {
            state: BriefSectionState::Read,
            stance: Some(stance),
            landed,
            summary: format!("{landed} landed commit(s) wait past v0.1.0"),
        }
    }

    /// The line says the release in two words, whatever the stance.
    #[test]
    fn the_release_is_said_in_the_line() {
        let line = |r: BriefReleaseSection| {
            compose(
                "t".into(),
                20,
                origin(),
                session(None),
                peers(0, false),
                mesh_off(),
                BriefIntegrationSection {
                    state: BriefSectionState::Undeclared,
                    summary: "never observed".into(),
                },
                r,
                derive_from(false, None, None),
            )
            .line
        };
        assert!(line(release(Stance::Overdue, 4)).ends_with("release OVERDUE 4"));
        assert!(line(release(Stance::Current, 0)).ends_with("release current"));
        assert!(line(release(Stance::Owed, 1)).ends_with("release owed 1"));
    }

    fn canned_owed(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({
            "stance": "owed", "summary": "3 landed commit(s) wait", "landed": 3, "landings": 1,
            "overdue_after_seconds": 21600,
        }))
    }

    fn canned_unreadable(_: &Context, _: Anything) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({ "stance": "nowhere" }))
    }

    /// A context whose `release.currency` answers through `handler`.
    fn currency_context(
        repo: &SyntheticRepository,
        handler: fn(&Context, Anything) -> Result<serde_json::Value, CapabilityError>,
    ) -> Context {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(vec![capability! {
                id: "release.currency", title: "Currency", description: "Canned.",
                input: Anything, output: serde_json::Value,
                stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
                handler: handler,
            }])
            .build()
            .expect("a registry of one stand-in");
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        )
    }

    /// The release section is what `release.currency` answered, without the version; a
    /// capability that is not there, or an answer of another shape, leaves it unread with why.
    #[test]
    fn the_release_section_is_what_release_currency_answered() {
        let repo = SyntheticRepository::small().expect("a repository");
        let measured = release_section(&repo.context().expect("a context"));
        assert_eq!(
            (measured.state, measured.stance),
            (BriefSectionState::Unread, Some(Stance::Unknown)),
            "{}",
            measured.summary
        );
        let owed = release_section(&currency_context(&repo, canned_owed));
        assert_eq!(
            (owed.state, owed.stance, owed.landed),
            (BriefSectionState::Read, Some(Stance::Owed), 3)
        );
        for broken in [
            bare_context(&repo),
            currency_context(&repo, canned_unreadable),
        ] {
            let section = release_section(&broken);
            assert_eq!(section.state, BriefSectionState::Unread);
            assert!(
                section
                    .summary
                    .starts_with("release.currency did not answer:"),
                "{}",
                section.summary
            );
        }
        assert_eq!(
            BriefReleaseSection::default().state,
            BriefSectionState::Unread
        );
    }

    #[test]
    fn the_leaseholder_is_asked_over_the_declared_route_and_every_failure_is_named() {
        let repo = SyntheticRepository::small().expect("a repository");
        let asked = input(Checkouts::This, None);

        // a registry that declares no route for the brief cannot ask
        let e = forward(&bare_context(&repo), "http://127.0.0.1:1", &asked).expect_err("no route");
        assert_eq!(e, "brief.session declares no HTTP route to ask over");

        let ctx = canned_context(&repo);
        let dead = dead_url();
        let e = forward(&ctx, &dead, &asked).expect_err("nobody listens");
        assert!(
            e.starts_with(&format!("the leaseholder at {dead} did not answer (")),
            "{e}"
        );
        assert!(e.ends_with("; composed by this process"), "{e}");

        let old = serve(|_| (404, "{}".into()));
        let e = forward(&ctx, &old, &asked).expect_err("an older server");
        assert_eq!(
            e,
            format!(
                "the leaseholder at {old} answered 404 (a server older than this executable has no brief); composed by this process"
            )
        );

        let garbled = serve(|_| (200, "not a brief".into()));
        let e = forward(&ctx, &garbled, &asked).expect_err("an unreadable brief");
        assert!(
            e.starts_with(&format!(
                "the leaseholder at {garbled} answered a brief this executable cannot read ("
            )),
            "{e}"
        );

        // the leaseholder's answer is the brief, asked for locally and for this checkout
        let theirs = a_brief();
        let body = serde_json::to_string(&theirs).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let heard = std::sync::Arc::clone(&seen);
        let good = serve(move |path| {
            *heard.lock().unwrap() = path.to_string();
            (200, body.clone())
        });
        assert_eq!(forward(&ctx, &good, &asked), Ok(theirs));
        assert_eq!(
            seen.lock().unwrap().as_str(),
            "/api/v1/brief?local=true&checkouts=this",
            "no session asked, none sent"
        );
    }

    /// The command line asks the leaseholder; one that answers the probe and not the brief
    /// leaves the brief to this process, which says why.
    #[test]
    fn a_leaseholder_that_cannot_brief_leaves_the_brief_to_this_process() {
        let repo = SyntheticRepository::small().expect("a repository");
        let id = crate::repository::identity(&canonical(&repo));
        let url = serve(move |path| {
            if path == "/" {
                (
                    200,
                    json!({ "name": "majordomus", "repository_id": id }).to_string(),
                )
            } else {
                (404, "{}".into())
            }
        });
        lease_of_another(&repo, Some(&url));
        let ctx = canned_context(&repo);
        can("continuity.state", continuity_with_task());
        can("lifecycle.episodes", two_episodes());
        can("peers.list", nine_on_a_board());
        let b = brief(&ctx, input(Checkouts::This, Some("prov-2"))).expect("a brief");
        assert_eq!(b.answered_by.process, "this_process");
        let note = b.answered_by.note.clone().expect("why");
        assert!(
            note.contains(&format!("the leaseholder at {url} answered 404")),
            "{note}"
        );
        assert!(
            b.lines
                .last()
                .is_some_and(|l| l.starts_with("answered by  this_process (")),
            "{:?}",
            b.lines
        );
        assert_eq!(b.session.episode.as_deref(), Some("s-0002"));
    }

    // ------------------------------------------------ the mesh section

    /// Two runtimes of one process, linked over an in-memory transport: the socket is the
    /// only thing replaced.
    mod mesh_world {
        pub(super) use crate::mesh::testing::{runtime, Net};
    }

    #[test]
    fn an_active_mesh_is_read_for_its_links_its_claims_and_what_it_addresses_here() {
        use crate::mesh::journal::{ClaimMode, HandoverBody, SessionInfo};
        let net = std::sync::Arc::new(mesh_world::Net::default());
        let (a, b) = (
            mesh_world::runtime(&net, "a:1"),
            mesh_world::runtime(&net, "b:1"),
        );
        a.dial(&["b:1".into()]).expect("B welcomes A");
        // two of this runtime's workers claim scopes that meet, and one asks this runtime
        // itself for a review; its own handover waits on nobody here
        a.claim(
            &SessionInfo::named("s1", "cli"),
            vec!["apps".into()],
            None,
            ClaimMode::Advisory,
            None,
        )
        .unwrap();
        a.claim(
            &SessionInfo::named("s2", "cli"),
            vec!["apps/majordomus-cli".into()],
            None,
            ClaimMode::Advisory,
            None,
        )
        .unwrap();
        let me = a.status().runtime.clone().expect("a runtime");
        a.request_review("s1", "feature/x".into(), vec![], None, Some(me), None)
            .unwrap();
        let body = "# Objective\ncover the brief\n".to_string();
        a.publish_handover(HandoverBody {
            id: HandoverBody::digest_of(&body),
            task: None,
            issue: None,
            milestone: None,
            branch: Some("feature/x".into()),
            head: None,
            created_at: None,
            name: None,
            body,
        })
        .unwrap();

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&a));

        let fresh = mesh_section(&ctx);
        assert_eq!(fresh.state, BriefSectionState::Read);
        assert_eq!((fresh.links, fresh.connected, fresh.failing), (1, 1, 0));
        assert_eq!((fresh.claims_held, fresh.own_claims), (2, 2));
        assert_eq!(
            (
                fresh.reviews_for_you,
                fresh.handovers_waiting,
                fresh.overlaps_for_you
            ),
            (1, 0, 1)
        );
        assert_eq!(
            fresh.summary,
            "1 of 1 link(s) connected; 2 claim(s) held, 2 of them this runtime's; \
             for this runtime: 1 review(s), 0 handover(s) waiting, 1 overlap(s)"
        );

        // the link outlives its expiry with no exchange: it is failing
        std::thread::sleep(Duration::from_millis(3500));
        let stale = mesh_section(&ctx);
        assert_eq!((stale.connected, stale.failing), (0, 1));
        assert!(
            stale
                .summary
                .starts_with("0 of 1 link(s) connected, 1 failing; "),
            "{}",
            stale.summary
        );

        // a runtime nothing is addressed to says so
        let quiet = SyntheticRepository::small().expect("a repository");
        let quiet_ctx = quiet.context().expect("a context");
        let c = mesh_world::runtime(&net, "c:1");
        quiet_ctx.mesh.attach_cooperation(std::sync::Arc::clone(&c));
        let calm = mesh_section(&quiet_ctx);
        assert_eq!(
            calm.summary,
            "0 of 0 link(s) connected; 0 claim(s) held, 0 of them this runtime's; \
             nothing addressed to this runtime"
        );
        for r in [a, b, c] {
            r.stop();
        }
    }

    /// What another runtime holds is not this one's: its handover waits here, its claims
    /// that meet are no overlap of this runtime's, and an answered review is for nobody.
    #[test]
    fn another_runtimes_handover_waits_here_and_its_overlaps_and_answered_reviews_do_not() {
        use crate::mesh::journal::{ClaimMode, HandoverBody, SessionInfo};
        let net = std::sync::Arc::new(mesh_world::Net::default());
        let (a, b) = (
            mesh_world::runtime(&net, "a:1"),
            mesh_world::runtime(&net, "b:1"),
        );
        let key = a.dial(&["b:1".into()]).expect("B welcomes A");
        for (session, scope) in [("s1", "apps"), ("s2", "apps/majordomus-cli")] {
            b.claim(
                &SessionInfo::named(session, "cli"),
                vec![scope.into()],
                None,
                ClaimMode::Advisory,
                None,
            )
            .unwrap();
        }
        let body = "# Objective\nhand it on\n".to_string();
        b.publish_handover(HandoverBody {
            id: HandoverBody::digest_of(&body),
            task: None,
            issue: None,
            milestone: None,
            branch: Some("feature/y".into()),
            head: None,
            created_at: None,
            name: None,
            body,
        })
        .unwrap();
        let me = a.status().runtime.clone().expect("a runtime");
        a.request_review("s1", "feature/x".into(), vec![], None, Some(me), None)
            .unwrap();
        let request = a.state().reviews[0].key.clone();
        a.answer_review(&request, "s9", "approved".into(), None)
            .unwrap();
        for _ in 0..2 {
            b.journal().beat_own();
            a.journal().beat_own();
            a.sync_with(&key).unwrap();
        }

        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.attach_cooperation(std::sync::Arc::clone(&a));
        let section = mesh_section(&ctx);
        assert_eq!((section.claims_held, section.own_claims), (2, 0));
        assert_eq!(
            (
                section.reviews_for_you,
                section.handovers_waiting,
                section.overlaps_for_you
            ),
            (0, 1, 0)
        );
        for r in [a, b] {
            r.stop();
        }
    }

    #[test]
    fn a_mesh_that_is_not_running_says_why() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        ctx.mesh.decline_cooperation("no endpoint to be dialed at");
        let off = mesh_section(&ctx);
        assert_eq!(off.state, BriefSectionState::Undeclared);
        assert_eq!(off.summary, "not active here: no endpoint to be dialed at");
    }

    // ------------------------------------------------ integration and derive

    #[test]
    fn an_observed_forge_is_the_integration_sections_answer() {
        use crate::integration::{store_observation, ForgeObservation, OBSERVATION_SCHEMA};
        let repo = SyntheticRepository::small().expect("a repository");
        let never = integration_section(repo.root());
        assert_eq!(never.state, BriefSectionState::Undeclared);
        store_observation(
            repo.root(),
            &ForgeObservation {
                schema: OBSERVATION_SCHEMA,
                repository: "owner/repo".into(),
                base: "master".into(),
                base_sha: "0".repeat(40),
                observed_at: "2026-10-01T00:00:00Z".into(),
                required_checks: Some(vec!["ci".into()]),
                review_policy: Some(Default::default()),
                up_to_date_required: Some(true),
                merge_methods: Some(vec!["merge".into()]),
                pull_requests: Vec::new(),
                resolved: Default::default(),
                delete_branch_on_merge: None,
            },
        )
        .expect("an observation");
        let read = integration_section(repo.root());
        assert_eq!(read.state, BriefSectionState::Read);
        assert_eq!(
            Some(read.summary.clone()),
            crate::commands::prs::brief(repo.root()),
            "the section is what `prs brief` says"
        );
    }

    #[test]
    fn a_derive_lock_whose_holder_is_gone_is_said_to_be_reclaimable() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let lock = dir.path().join(".derive.lock");
        let priority = dir.path().join(".derive.priority");
        std::fs::create_dir(&lock).expect("the lock directory");
        // a pid above every system's pid range names no process
        std::fs::write(
            lock.join("owner"),
            "derive:feature/y pid=99999999 branch=feature/y\n",
        )
        .expect("the owner line");
        // a marker that names nobody
        std::fs::write(&priority, "\n").expect("the marker");
        let gone = derive_from(true, Some(&lock), Some(&priority));
        assert_eq!(gone.pid, Some(99_999_999));
        assert_eq!(gone.alive, Some(false));
        assert!(
            gone.summary.starts_with(
                "lock held by derive:feature/y (pid 99999999, NOT running: the next derive reclaims it"
            ),
            "{}",
            gone.summary
        );
        assert_eq!(gone.priority.as_deref(), Some(""));
        assert!(
            gone.summary.ends_with("; priority to ?"),
            "{}",
            gone.summary
        );

        // a lock dated in the future has no age to tell
        std::fs::File::open(&lock)
            .and_then(|f| f.set_modified(std::time::SystemTime::now() + Duration::from_secs(3600)))
            .expect("the lock's time");
        let ageless = derive_from(true, Some(&lock), None);
        assert_eq!(ageless.held_seconds, None);
        assert!(
            ageless
                .summary
                .ends_with("NOT running: the next derive reclaims it)"),
            "{}",
            ageless.summary
        );
    }

    // ------------------------------------------------ the line

    #[test]
    fn the_line_says_what_could_not_be_read_and_what_is_addressed_here() {
        let mut s = session(None);
        s.state = BriefSectionState::Unread;
        let mut p = peers(0, false);
        p.state = BriefSectionState::Unread;
        let mut m = mesh_off();
        m.state = BriefSectionState::Read;
        m.links = 2;
        m.connected = 1;
        m.reviews_for_you = 1;
        m.handovers_waiting = 1;
        m.overlaps_for_you = 1;
        let free = BriefDeriveSection {
            state: BriefSectionState::Read,
            holder: None,
            pid: None,
            alive: None,
            held_seconds: None,
            priority: None,
            summary: "lock free".into(),
        };
        let line = one_line(
            &s,
            &p,
            &m,
            &BriefIntegrationSection {
                state: BriefSectionState::Unread,
                summary: String::new(),
            },
            &BriefReleaseSection::default(),
            &free,
        );
        assert_eq!(
            line,
            "session ? | peers ? | mesh 1/2 +3 for you | prs ? | release ? | derive free"
        );
        // a mesh that is read and addresses nothing here shows its links alone
        m.reviews_for_you = 0;
        m.handovers_waiting = 0;
        m.overlaps_for_you = 0;
        let line = one_line(
            &session(None),
            &peers(0, false),
            &m,
            &a_brief().integration,
            &a_brief().release,
            &free,
        );
        assert!(line.contains("| mesh 1/2 |"), "{line}");
    }

    #[test]
    fn every_state_has_its_word() {
        assert_eq!(state_word(BriefSectionState::Read), "read");
        assert_eq!(state_word(BriefSectionState::Unread), "unread");
        assert_eq!(state_word(BriefSectionState::Undeclared), "undeclared");
        for state in [
            BriefSectionState::Read,
            BriefSectionState::Partial,
            BriefSectionState::Unread,
            BriefSectionState::Undeclared,
        ] {
            assert_eq!(
                serde_json::to_value(state).unwrap(),
                json!(state_word(state)),
                "the word is the one it serialises under"
            );
        }
    }
}
