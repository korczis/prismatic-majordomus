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
    let mut summary = format!(
        "{} attached on {} board(s)",
        attached.len(),
        list.boards.len()
    );
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
        boards: list.boards.len(),
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
                .unwrap_or_else(|| "cooperation is not running in this process".into())
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
    common.parent()?.parent().map(Path::to_path_buf)
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
    derive: BriefDeriveSection,
) -> SessionBrief {
    let line = one_line(&session, &peers, &mesh, &integration, &derive);
    let mut lines = vec![
        format!("session      {}", session.summary),
        format!("peers        {}", peers.summary),
    ];
    lines.extend(peers.named.iter().map(|n| format!("             {n}")));
    lines.push(format!("mesh         {}", mesh.summary));
    lines.push(format!("integration  {}", integration.summary));
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
        derive,
    }
}

/// The one line: a few words per section, every section present whatever its state.
fn one_line(
    session: &BriefSessionSection,
    peers: &BriefPeersSection,
    mesh: &BriefMeshSection,
    integration: &BriefIntegrationSection,
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
    let mut parts = vec![s, p, m, i];
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
            derive,
        );
        assert_eq!(
            b.line,
            "I2399 @feature/x | peers 3/5b | mesh off | prs 52 open, next #902"
        );
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
            derive_from(false, None, None),
        );
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
}
