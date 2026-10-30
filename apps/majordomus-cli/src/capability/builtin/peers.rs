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

use std::collections::BTreeMap;
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
/// The evidence for that is `the_default_board_is_the_repositorys` below rather than a doc
/// example here: this module is `pub(crate)`, so a doc test — which compiles as an external
/// crate — cannot name the type at all.
pub struct PeerListInput {
    // The whole of "why `this` exists" is in this module's own documentation: kept to one
    // line here because a field's doc is rendered into a table cell, and a paragraph
    // break in it breaks the row (docs/generated/modules/peers.md).
    /// Which checkouts the board covers. Absent means `repository`; `this` is the checkout the call reached and nothing else, and it is what a sibling server is asked so that a gather can never ask back.
    #[serde(default)]
    pub checkouts: Checkouts,
    /// How many of the covered checkouts are listed in `boards`. Absent means `summary`: every checkout with a server, a worker, or a board that could not be read, the rest counted in `omitted`; `full` lists every one.
    #[serde(default)]
    pub view: PeerListView,
}

/// How many of the covered checkouts `peers.list` lists one by one — the same two words, and
/// the same default, as the mesh's own `view` (`mesh::page::View`), for the same reason.
///
/// On 2026-10-22 this repository registered 625 checkouts, 601 of them with no server and
/// nobody attached, and the default answer was 168 kB: past what an MCP client shows, so
/// the read every worker is told to make before it starts went through a saved file. A
/// checkout with no server holds no board (`ServerStanding::Absent`, or a `Stale` lease that
/// names a process that is gone), so listing it says only that it exists; the summary counts
/// those by standing and lists every other. The evidence is
/// `the_summary_counts_only_checkouts_that_hold_no_board` below.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PeerListView {
    /// This checkout, every checkout with a worker attached or a server standing, and every
    /// board that could not be read; the checkouts with no server and nobody attached are
    /// counted in `omitted` by standing.
    #[default]
    Summary,
    /// Every covered checkout, one entry each.
    Full,
}

impl BenchmarkCases for PeerListInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // the narrow answer: a benchmark may not depend on how many worktrees the machine
        // running it happens to have registered, nor probe servers that are not its own
        vec![NamedCase::new(
            "default",
            PeerListInput {
                checkouts: Checkouts::This,
                view: PeerListView::Summary,
            },
        )]
    }
}

/// Whether the summary lists `board` rather than counting it: this checkout, any board with a
/// worker or a reason it was not read, and any checkout whose server stands. What is left is a
/// checkout with no server and nobody attached, which holds no board at all.
fn listed_in_summary(board: &BoardView) -> bool {
    board.checkout.this_checkout
        || board.attached > 0
        || board.reason.is_some()
        || !matches!(
            board.standing,
            ServerStanding::Absent | ServerStanding::Stale
        )
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// The mesh stream of the server whose memory holds this board, when its mesh runs: the
    /// stream it projects the board's peers into as `board-pN` sessions. A position such as
    /// `p1` is that server's alone, so only this stream ties a board peer to its session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
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
    /// How many checkouts the answer covers, listed in `boards` or counted in `omitted` — a
    /// count, where the input `checkouts` is a choice of which to cover.
    pub covered: usize,
    /// One entry per checkout the answer lists, reached or not: every covered checkout under
    /// `view: full`; under the default summary, every one but those `omitted` counts. A board
    /// that could not be read is always listed.
    pub boards: Vec<BoardView>,
    /// The covered checkouts the summary does not list, counted by standing (`absent`,
    /// `stale`): no server, nobody attached, no board. Empty under `view: full`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub omitted: BTreeMap<String, usize>,
    /// Whether every board the answer covers was read. `false` when a checkout's server
    /// could not be asked, and `BoardView::reason` says which and why — so that "nobody
    /// else is here" is never reported when the truth is "I could not ask".
    pub complete: bool,
    /// The mesh stream of the server that answered, when its mesh runs: the stream its own
    /// board's peers are projected into. A sibling's answer carries it so that the gatherer
    /// can name it on that sibling's `BoardView`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
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
pub(crate) fn board_of(ctx: &Context, url: &str) -> Result<Vec<Peer>, String> {
    answer_of(ctx, url).map(|(peers, _)| peers)
}

/// [`board_of`], with the mesh stream the sibling's server projects its board into, when it
/// says (a server older than this field says nothing, and its peers are then joined to no
/// session).
fn answer_of(ctx: &Context, url: &str) -> Result<(Vec<Peer>, Option<String>), String> {
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
        lease::timings().probe_timeout,
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
    let peers = serde_json::from_value(value["peers"].clone())
        .map_err(|e| format!("{url} answered a board this executable cannot read ({e})"))?;
    Ok((peers, value["stream"].as_str().map(str::to_string)))
}

/// The mesh stream this process projects its own board into, when its mesh runs.
fn own_stream(ctx: &Context) -> Option<String> {
    ctx.mesh
        .cooperation()
        .map(|c| c.journal().own_stream().as_str().to_string())
}

pub(crate) fn peers_list(ctx: &Context, input: PeerListInput) -> Result<PeerList, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let local_half = server::local_half(&root);
    let git = repository::git_identity(&root);
    let checkouts = server::checkouts_of(&root, git.as_ref(), input.checkouts);

    // every checkout is asked at once, a bounded number at a time (server::each_checkout,
    // I2987), and the answers are folded in the order git lists the checkouts
    let read = server::each_checkout(&checkouts, |(path, branch, _primary)| {
        let worktree = path.canonicalize().unwrap_or(path.clone());
        let this_checkout = worktree == root;
        let checkout = PeerCheckout {
            id: repository::identity(&worktree),
            worktree: worktree.clone(),
            branch: branch.clone(),
            this_checkout,
        };
        // this checkout's board is in this process's memory: it is never asked over the
        // wire, which also means the answer holds it even when this process is not the
        // leaseholder (a superseded server still has its own peers, and they are real)
        let (standing, url, found) = if this_checkout {
            let (standing, _, read) = server::standing_at(&worktree, &local_half);
            let url = read.document().and_then(|d| d.url.clone());
            (standing, url, Ok((ctx.peers.list(), own_stream(ctx))))
        } else {
            let (standing, reason, read) = server::standing_at(&worktree, &local_half);
            let url = read.document().and_then(|d| d.url.clone());
            let found = match (standing, url.as_deref()) {
                (
                    ServerStanding::Ready | ServerStanding::Outdated | ServerStanding::Skewed,
                    Some(url),
                ) => answer_of(ctx, url),
                // No server, no board, and nobody attached there — which is a complete
                // answer about that checkout and not a failure to reach one. A stale lease
                // belongs here too: it names a process that is gone, so its board went
                // with it. Calling that "unread" would leave `complete` false on this
                // machine permanently, because an abandoned worktree's lease outlives it —
                // and a flag that is always false says nothing when it matters. `standing`
                // already tells the reader the lease is stale, and `server.status` is the
                // capability that explains it.
                (ServerStanding::Absent | ServerStanding::Stale, _) => Ok((Vec::new(), None)),
                // Starting is the genuinely unknown one: a server is binding right now and
                // may hold peers a second from now. Brief, and reported rather than guessed.
                _ => Err(reason.unwrap_or_else(|| {
                    format!(
                        "its server is {} and has no board to read yet",
                        standing.as_str()
                    )
                })),
            };
            (standing, url, found)
        };
        (checkout, standing, url, found)
    });

    let mut peers: Vec<Peer> = Vec::new();
    let mut boards: Vec<BoardView> = Vec::with_capacity(read.len());
    let mut complete = true;
    for (checkout, standing, url, found) in read {
        let (found, stream, reason) = match found {
            Ok((found, stream)) => (found, stream, None),
            Err(why) => {
                complete = false;
                (Vec::new(), None, Some(why))
            }
        };
        boards.push(BoardView {
            standing,
            url,
            attached: found.iter().filter(|p| p.attached).count(),
            reason,
            stream,
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

    let covered = boards.len();
    let omitted = match input.view {
        PeerListView::Full => BTreeMap::new(),
        PeerListView::Summary => summarise(&mut boards),
    };

    Ok(PeerList {
        count: peers.iter().filter(|p| p.attached).count(),
        caller: ctx.caller.clone(),
        overlaps: overlaps_among(&peers),
        peers,
        covered,
        boards,
        omitted,
        complete,
        stream: own_stream(ctx),
    })
}

/// Keep the boards the summary lists and count the rest by standing.
fn summarise(boards: &mut Vec<BoardView>) -> BTreeMap<String, usize> {
    let mut omitted: BTreeMap<String, usize> = BTreeMap::new();
    boards.retain(|b| {
        let listed = listed_in_summary(b);
        if !listed {
            *omitted.entry(b.standing.as_str().to_string()).or_default() += 1;
        }
        listed
    });
    omitted
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
                description: "Every worker of this repository: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, what it announced, and which checkout it is attached to. A server serves one checkout, so the board of a repository worked on through linked worktrees is gathered: this checkout's board out of memory, every other checkout's from the server its lease names, asked for its own board alone. 'covered' says how many checkouts were covered, 'boards' lists each one with a server, a worker or a board that could not be read (the rest counted in 'omitted'; 'view: full' lists every one), and 'complete' whether every board could be read, so a short board is never mistaken for an empty repository. 'checkouts: this' reads one board and enumerates, probes and asks nothing else. In-memory on every server; gone with the processes.",
                input: PeerListInput,
                output: PeerList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_peers"), http: get("/api/v1/peers"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_list,
            }
            // the mesh's working state, not the repository's: a loopback caller only (ADR 0220)
            .reads_only_locally(),
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
    /// The default is the repository's board, and the narrow word still parses — the same
    /// question, the same word and the same default as `server::ServerStatusInput`.
    #[test]
    fn the_default_board_is_the_repositorys() {
        use super::*;
        let asked_nothing: PeerListInput = serde_json::from_str("{}").expect("an empty object");
        assert_eq!(
            asked_nothing.checkouts,
            Checkouts::Repository,
            "the board is the repository's"
        );
        let narrow: PeerListInput =
            serde_json::from_str(r#"{"checkouts":"this"}"#).expect("the narrow word");
        assert_eq!(narrow.checkouts, Checkouts::This);
        assert_eq!(
            asked_nothing.view,
            PeerListView::Summary,
            "the summary is the default"
        );
        let full: PeerListInput = serde_json::from_str(r#"{"view":"full"}"#).expect("full");
        assert_eq!(full.view, PeerListView::Full);
        assert!(serde_json::from_str::<PeerListInput>(r#"{"view":"everything"}"#).is_err());
    }

    /// The summary counts exactly the checkouts that hold no board — no server, nobody
    /// attached, nothing unread — and lists every other: this checkout even when it is
    /// absent, a live server with nobody on it, an attached worker, and above all a board
    /// that could not be read, because dropping that one is how "nobody else is here" gets
    /// reported when the truth is "I could not ask".
    #[test]
    fn the_summary_counts_only_checkouts_that_hold_no_board() {
        use super::*;
        let board = |n: &str, this: bool, standing, attached, reason: Option<&str>| BoardView {
            checkout: PeerCheckout {
                id: n.into(),
                worktree: PathBuf::from(format!("/r-wt/{n}")),
                branch: Some(n.into()),
                this_checkout: this,
            },
            standing,
            url: None,
            attached,
            reason: reason.map(Into::into),
            stream: None,
        };
        let mut boards = vec![
            board("here", true, ServerStanding::Absent, 0, None),
            board("idle-a", false, ServerStanding::Absent, 0, None),
            board("idle-b", false, ServerStanding::Absent, 0, None),
            board("gone", false, ServerStanding::Stale, 0, None),
            board("serving", false, ServerStanding::Ready, 0, None),
            board("old", false, ServerStanding::Outdated, 2, None),
            board(
                "binding",
                false,
                ServerStanding::Starting,
                0,
                Some("starting"),
            ),
        ];
        let omitted = summarise(&mut boards);
        let listed: Vec<&str> = boards.iter().map(|b| b.checkout.id.as_str()).collect();
        assert_eq!(listed, ["here", "serving", "old", "binding"]);
        assert_eq!(omitted.get("absent"), Some(&2));
        assert_eq!(omitted.get("stale"), Some(&1));
        assert_eq!(
            listed.len() + omitted.values().sum::<usize>(),
            7,
            "every covered checkout is listed or counted, never neither"
        );
        // a board that could not be read is listed whatever its standing says
        let mut unread = vec![board(
            "x",
            false,
            ServerStanding::Absent,
            0,
            Some("no answer"),
        )];
        assert!(summarise(&mut unread).is_empty());
        assert_eq!(unread.len(), 1);
    }

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

    /// A sibling checkout whose board cannot be read is listed with the reason, and the answer
    /// says it is not complete: a server still starting, and one that answers for the sibling
    /// with a board this executable cannot read.
    #[test]
    fn a_sibling_board_that_cannot_be_read_is_named_and_the_answer_is_incomplete() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = crate::monitor::testing::context(&repo);
        let root = repo.root().canonicalize().unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(["-c", "user.email=t@t", "-c", "user.name=t"])
                .args(args)
                .current_dir(&root)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["commit", "-q", "--allow-empty", "-m", "root"]);
        let sibling = root.join("side");
        git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            sibling.to_str().unwrap(),
        ]);
        let sibling = sibling.canonicalize().unwrap();
        let lease = lease::lease_file(&sibling, &server::local_half(&root));
        std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
        let read = || {
            let list = peers_list(
                &ctx,
                PeerListInput {
                    checkouts: Checkouts::Repository,
                    view: PeerListView::Full,
                },
            )
            .unwrap();
            let reason = list
                .boards
                .iter()
                .find(|b| b.checkout.worktree == sibling)
                .map(|b| b.reason.clone())
                .expect("the sibling's board is listed");
            (list.complete, reason.unwrap_or_default())
        };

        // a lease its owner has not written yet: the server is still starting
        std::fs::write(&lease, "").unwrap();
        let (complete, reason) = read();
        assert!(!complete);
        assert!(reason.contains("no board to read yet"), "{reason}");

        // a server that answers for the sibling with a board that is not a list of peers
        let url =
            crate::monitor::testing::http_stub(repository::identity(&sibling), r#"{"peers": 7}"#);
        let doc = lease::LeaseDocument {
            schema: lease::SCHEMA.into(),
            pid: std::process::id(),
            token: "t".into(),
            root: sibling.clone(),
            url: Some(url),
            started_at: "2026-10-18T00:00:00Z".into(),
            executable: None,
            version: Some(crate::VERSION.into()),
            share: None,
        };
        std::fs::write(&lease, serde_json::to_string(&doc).unwrap()).unwrap();
        let (complete, reason) = read();
        assert!(!complete);
        assert!(reason.contains("cannot read"), "{reason}");
    }
}
