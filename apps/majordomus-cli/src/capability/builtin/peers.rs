//! The `peers` module: the workers of this repository, and what they say they are working
//! on. Nothing here writes to the repository.
//!
//! # The board is the repository's, gathered from every checkout's server
//!
//! A board is one server process's memory, and (ADR 0035) a server serves the checkout it
//! was started in: a linked worktree is a checkout of its own, elects a server of its own,
//! and keeps a board of its own. So the sentence every worker is bootstrapped with — "one
//! shared server serves this repository, and every worker attached to it is visible to
//! every other" — was true of one checkout and false of a repository, and on 2026-09-11
//! this repository had seven live servers over one git repository, seven boards, and nine
//! agents spread across sixty worktrees who could not see one another at all. Two of them
//! built the same subsystem in one afternoon.
//!
//! `peers.list` is therefore repository-wide by default. It enumerates the checkouts git
//! registers, exactly as `server.status` does and through the same reader, reads the lease
//! of each, and asks every server that answers for its own board — `checkouts=this`, which
//! is what makes the gather one hop deep and not a cycle. Its own board it reads from
//! memory. Every peer it returns is stamped with the checkout it came from, because `p1`
//! is the first session of every board and an unqualified id would be ambiguous the moment
//! two boards are merged.
//!
//! Nothing is written and no server is contacted twice: this is a read of what already
//! exists. There is no second authority, no registry of servers, no new port and no lease
//! but the one each checkout already had.
//!
//! # What it costs, and how to not pay it
//!
//! One lease read per registered checkout, one probe per checkout whose lease names an
//! address, and one more round trip per server that answers. Measured on this repository
//! on 2026-09-11 — 118 registered checkouts, 7 live servers — the enumeration and probing
//! `server.status` already does took 3.0s wall, and the boards are a handful of loopback
//! round trips on top. A caller that only wants the board of the checkout it is in says
//! `checkouts: this`, which enumerates nothing, probes nothing and reads one board out of
//! memory.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CapabilityKind, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::lease;
use crate::peers::{overlaps_among, Announced, Overlap, Peer, PeerCheckout, PeerId};
use crate::repository;
use crate::{capability, module};

use super::server::{self, Checkouts, ServerStanding};
use super::{get, mcp, post};

// ---------------------------------------------------------------- peers.list

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `peers.list`: how wide the board is.
///
/// The same one field, the same word and the same default as
/// [`super::server::ServerStatusInput`], because it is the same question asked of the same
/// set of checkouts, and a caller should not have to learn it twice.
///
/// ```
/// use majordomus_cli::capability::builtin::peers::PeerListInput;
/// use majordomus_cli::capability::builtin::server::Checkouts;
/// let asked_nothing: PeerListInput = serde_json::from_str("{}").unwrap();
/// assert_eq!(asked_nothing.checkouts, Checkouts::Repository, "the board is the repository's");
/// let narrow: PeerListInput = serde_json::from_str(r#"{"checkouts":"this"}"#).unwrap();
/// assert_eq!(narrow.checkouts, Checkouts::This);
/// ```
pub struct PeerListInput {
    // The whole of "why `this` exists" is in this module's own documentation: kept to one
    // line here because a field's doc is rendered into a table cell, and a paragraph
    // break in it breaks the row (docs/generated/modules/peers.md).
    /// Which checkouts the board covers. Absent means `repository`; `this` is the checkout the call reached and nothing else, and it is what a sibling server is asked so that a gather can never ask back.
    #[serde(default)]
    pub checkouts: Checkouts,
}

impl BenchmarkCases for PeerListInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // the narrow answer: a benchmark may not depend on how many worktrees the machine
        // running it happens to have registered, nor probe servers that are not its own
        vec![NamedCase::new(
            "default",
            PeerListInput {
                checkouts: Checkouts::This,
            },
        )]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// One checkout's board, as the gather found it.
///
/// Present for every checkout the answer covers, whether or not its board could be read,
/// because a listing that silently drops the checkouts it could not reach is the shape of
/// answer that made this capability wrong in the first place: a reader would see a short
/// board and conclude nobody else is working here.
pub struct BoardView {
    /// The checkout.
    #[serde(flatten)]
    pub checkout: PeerCheckout,
    /// Where its server stands, by the same reading `server.status` reports.
    pub standing: ServerStanding,
    /// The address its lease names, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// How many of this board's peers are attached.
    pub attached: usize,
    /// Why this board is not in the answer. `None` when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `peers.list`: every worker of this repository, from every checkout's board.
pub struct PeerList {
    /// How many peers are attached across every board that was read, the caller included.
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The caller's own peer id, when the call came through an MCP session. A position on
    /// this checkout's board, not a durable identity: it is handed out again after a
    /// reconnect, and [`Peer::checkout`] is the half of a worker's identity that is not.
    pub caller: Option<PeerId>,
    /// The peers, this checkout's board first and then the siblings in the order git
    /// registers them, each in its own board's attachment order. Every one carries the
    /// checkout it belongs to, because `p1` is the first session of every board. A peer
    /// that announced something and then went away is still here, with `attached: false`.
    pub peers: Vec<Peer>,
    /// Every pair of peers whose claimed scope meets, each pair once — now across
    /// checkouts as well as within one. Empty is the ordinary case, and a reader who sees
    /// an entry here is looking at two workers about to do the same work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlaps: Vec<Overlap>,
    /// One entry per checkout the answer covers, reached or not.
    pub boards: Vec<BoardView>,
    /// Whether every board the answer covers was read. `false` when a checkout's server
    /// could not be asked, and [`BoardView::reason`] says which and why — so that "nobody
    /// else is here" is never reported when the truth is "I could not ask".
    pub complete: bool,
}

/// One sibling server's own board, over the route the registry declares for `peers.list`.
///
/// Asked with `checkouts=this`, which is the whole of the cycle prevention: a server
/// answering that question enumerates no other checkout, reads no other lease and probes
/// no other server, so a gather is exactly one hop deep however many checkouts point at
/// each other. There is deliberately no retry without the parameter — a server too old to
/// know it (`Empty` refused unknown fields) would then answer its whole board, and a
/// server of *this* version that rejected the parameter for any other reason would answer
/// the repository, and the two of us would ask each other until a timeout. A board that
/// cannot be read narrowly is reported unread, with the reason.
fn board_of(ctx: &Context, url: &str) -> Result<Vec<Peer>, String> {
    let path = ctx
        .registry
        .get("peers.list")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or_else(|| "peers.list declares no HTTP route to ask over".to_string())?;
    let reply = crate::mcp::bridge::request(
        url,
        "GET",
        &format!("{path}?checkouts=this"),
        &[],
        None,
        lease::PROBE_TIMEOUT,
    )
    .map_err(|e| format!("{url} did not answer ({e})"))?;
    if reply.status != 200 {
        return Err(format!(
            "{url} answered {} for its own board (a server older than this one does not \
             know the 'checkouts' parameter and refuses it)",
            reply.status
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&reply.body).map_err(|e| format!("{url} answered non-JSON ({e})"))?;
    serde_json::from_value(value["peers"].clone())
        .map_err(|e| format!("{url} answered a board this executable cannot read ({e})"))
}

fn peers_list(ctx: &Context, input: PeerListInput) -> Result<PeerList, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let local_half = server::local_half(&root);
    let git = repository::git_identity(&root);
    let checkouts = server::checkouts_of(&root, git.as_ref(), input.checkouts);

    let mut peers: Vec<Peer> = Vec::new();
    let mut boards: Vec<BoardView> = Vec::with_capacity(checkouts.len());
    let mut complete = true;

    for (path, branch, _primary) in checkouts {
        let worktree = path.canonicalize().unwrap_or(path);
        let this_checkout = worktree == root;
        let checkout = PeerCheckout {
            id: repository::identity(&worktree),
            worktree: worktree.clone(),
            branch,
            this_checkout,
        };
        // this checkout's board is in this process's memory: it is never asked over the
        // wire, which also means the answer holds it even when this process is not the
        // leaseholder (a superseded server still has its own peers, and they are real)
        let (standing, url, found) = if this_checkout {
            let (standing, _, read) = server::standing_at(&worktree, &local_half);
            let url = read.document().and_then(|d| d.url.clone());
            (standing, url, Ok(ctx.peers.list()))
        } else {
            let (standing, reason, read) = server::standing_at(&worktree, &local_half);
            let url = read.document().and_then(|d| d.url.clone());
            let found = match (standing, url.as_deref()) {
                (ServerStanding::Ready | ServerStanding::Outdated, Some(url)) => board_of(ctx, url),
                // No server, no board, and nobody attached there — which is a complete
                // answer about that checkout and not a failure to reach one. A stale lease
                // belongs here too: it names a process that is gone, so its board went
                // with it. Calling that "unread" would leave `complete` false on this
                // machine permanently, because an abandoned worktree's lease outlives it —
                // and a flag that is always false says nothing when it matters. `standing`
                // already tells the reader the lease is stale, and `server.status` is the
                // capability that explains it.
                (ServerStanding::Absent | ServerStanding::Stale, _) => Ok(Vec::new()),
                // Starting is the genuinely unknown one: a server is binding right now and
                // may hold peers a second from now. Brief, and reported rather than guessed.
                _ => Err(reason.unwrap_or_else(|| {
                    format!("its server is {} and has no board to read yet", standing.as_str())
                })),
            };
            (standing, url, found)
        };

        let (found, reason) = match found {
            Ok(found) => (found, None),
            Err(why) => {
                complete = false;
                (Vec::new(), Some(why))
            }
        };
        boards.push(BoardView {
            standing,
            url,
            attached: found.iter().filter(|p| p.attached).count(),
            reason,
            checkout: checkout.clone(),
        });
        // every peer is stamped by the reader, its own included and whatever the sibling
        // said about itself: a sibling stamps `this_checkout: true` for its own board, and
        // from here that board is not this one
        peers.extend(found.into_iter().map(|mut p| {
            p.checkout = Some(checkout.clone());
            p
        }));
    }

    Ok(PeerList {
        count: peers.iter().filter(|p| p.attached).count(),
        caller: ctx.caller.clone(),
        overlaps: overlaps_among(&peers),
        peers,
        boards,
        complete,
    })
}

// ---------------------------------------------------------------- peers.announce

/// The MCP tool `peers.announce` is projected as. Named once, here, because the bridge
/// recognises the frame that carries an announcement by this name in order to replay it
/// after a takeover or a re-attach; the exposure below and the bridge read the same word.
pub const ANNOUNCE_TOOL: &str = "majordomus_announce";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `peers.announce`: what the calling peer is working on.
pub struct AnnounceInput {
    /// One line, in the peer's words: the task, the question, the intent.
    pub intent: String,
    /// Repository-relative paths the peer expects to touch. Informational: other peers
    /// read it to avoid a collision; nothing here enforces it.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Which of this peer's claims this is, when the peer holds more than one.
    ///
    /// One MCP session is not always one piece of work — a client that fans work out to
    /// subagents shares its session with all of them — and without a name every
    /// announcement replaces the last, so the board ends up describing whichever worker
    /// spoke most recently and the rest of the scope silently stops being claimed. Name a
    /// claim and it stands beside the others; announce under that name again and it is
    /// updated. Leave it out and this is the peer's one unnamed claim, which is what a
    /// single session announcing about itself wants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<String>,
}

impl BenchmarkCases for AnnounceInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            AnnounceInput {
                intent: "benchmark: announcing".into(),
                scope: vec!["apps/majordomus-cli".into()],
                claim: None,
            },
        )]
    }
}

fn peers_announce(ctx: &Context, input: AnnounceInput) -> Result<Announced, CapabilityError> {
    let Some(caller) = &ctx.caller else {
        return Err(CapabilityError::Refused(
            "announce needs an MCP session: this call came through an interface with no peer identity (call the majordomus_announce tool)".into(),
        ));
    };
    if input.intent.trim().is_empty() {
        return Err(CapabilityError::InvalidInput(
            "argument 'intent' is required and must not be blank".into(),
        ));
    }
    let scope: Vec<String> = input
        .scope
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let claim = input
        .claim
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    ctx.peers
        .announce_claim(caller, claim, input.intent.trim(), scope)
        .ok_or_else(|| CapabilityError::Internal(format!("peer {caller} is not attached")))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "peers",
        title: "Peers",
        description: "The workers of this repository, named by their own initialize, and what each announced it is working on — gathered from the board of every checkout, because a server serves a checkout and a repository worked on through linked worktrees has one board per worktree. In memory; gone with the processes.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "peers.list",
                title: "List peers",
                description: "Every worker of this repository: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, what it announced, and which checkout it is attached to. A server serves one checkout, so the board of a repository worked on through linked worktrees is gathered: this checkout's board out of memory, every other checkout's from the server its lease names, asked for its own board alone. 'boards' says which checkouts were covered and 'complete' whether every one of them could be read, so a short board is never mistaken for an empty repository. 'checkouts: this' reads one board and enumerates, probes and asks nothing else. In-memory on every server; gone with the processes.",
                input: PeerListInput,
                output: PeerList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_peers"), http: get("/api/v1/peers"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_list,
            },
            capability! {
                id: "peers.announce",
                kind: CapabilityKind::Command,
                title: "Announce what this peer is working on",
                description: "Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. A peer may hold several claims at once: name one with 'claim' and it stands beside the others, announce under that name again and it is updated, leave it out and this is the peer's one unnamed claim. Name your claims when one session is doing several things at once — subagents share their parent's session, so an unnamed announcement from each of them would replace the last rather than adding to it. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller.",
                input: AnnounceInput,
                output: Announced,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp(ANNOUNCE_TOOL), http: post("/api/v1/peers/announce"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_announce,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the benchmark target — is derived from
    /// it. A refactor that dropped an exposure or renamed a route would still compile, and
    /// the suites that exercise the behaviour behind it would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "peers");
        let expected: &[(&str, &str, &str)] = &[
            ("peers.list", "majordomus_peers", "/api/v1/peers"),
            (
                "peers.announce",
                "majordomus_announce",
                "/api/v1/peers/announce",
            ),
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
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
    }
}
