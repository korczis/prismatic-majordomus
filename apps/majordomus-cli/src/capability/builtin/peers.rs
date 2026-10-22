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
use crate::peers::{claims_meet, overlaps_among, Announced, Overlap, Peer, PeerCheckout, PeerId};
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
    /// could not be asked, and `BoardView::reason` says which and why — so that "nobody
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
pub(crate) fn board_of(ctx: &Context, url: &str) -> Result<Vec<Peer>, String> {
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
                    format!(
                        "its server is {} and has no board to read yet",
                        standing.as_str()
                    )
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

// ---------------------------------------------------------------- peers.board

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One worker on the board: a peer of one checkout's server, named so that two boards'
/// `p1` are two workers.
pub struct BoardWorker {
    /// `<checkout id>/<peer id>`: the worker's key in this answer. A position on a board is
    /// handed out again after a reconnect, so the key identifies a worker in this answer and
    /// not across time; the checkout is the durable half.
    pub key: String,
    /// Its id on its own board.
    pub peer: PeerId,
    /// The client's own name, from its initialize.
    pub client: String,
    /// The client's own version.
    pub version: String,
    /// The worktree its server serves.
    pub worktree: PathBuf,
    /// The branch that worktree has checked out; absent when its head is detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Whether that is the checkout this answer was asked of.
    pub this_checkout: bool,
    /// Whether it is attached. A worker that announced something and went away is kept,
    /// with `false`, because its claim is still what somebody said they were doing.
    pub attached: bool,
    /// When it attached, RFC 3339, UTC.
    pub connected_at: String,
    /// Seconds since its server last heard from it.
    pub last_seen_seconds_ago: u64,
    /// How many claims it holds. Zero is a worker that attached and said nothing.
    pub claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One claim on the board: every claim of every worker, not only each worker's latest.
pub struct BoardClaim {
    /// `<worker key>#<name>`, the name empty for a worker's unnamed claim.
    pub key: String,
    /// The worker that holds it.
    pub worker: String,
    /// What the worker called it; absent for its unnamed claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it is for, in the worker's words.
    pub intent: String,
    /// The paths it names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// When it was announced, RFC 3339, UTC.
    pub at: String,
    /// Whether its worker is attached.
    pub attached: bool,
    /// The claims of other workers whose scope meets this one's, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One path somebody named, and who named it.
pub struct BoardPath {
    /// The path, as it was written, without a trailing slash.
    pub path: String,
    /// The claims that name exactly this path, by key.
    pub claims: Vec<String>,
    /// The workers behind them, each once, by key.
    pub workers: Vec<String>,
    /// Whether two different workers hold it: either both named it, or one named it and
    /// another named a path above or below it.
    pub contested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One pair of paths two claims meet on.
pub struct BoardMeeting {
    /// The path the first claim names.
    pub first: String,
    /// The path the second claim names.
    pub second: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Two claims of two different workers whose scope meets: both sides named, each pair once.
pub struct BoardCollision {
    /// The claim announced earlier, by key.
    pub first: String,
    /// The claim announced later, by key.
    pub second: String,
    /// Whether both workers are attached. A collision with a worker that left is with what
    /// it said, not with what it is doing.
    pub live: bool,
    /// Where they meet.
    pub paths: Vec<BoardMeeting>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One exclusive claim the mesh holds: the kind a second claimant is refused by, where a
/// board claim only tells.
pub struct BoardExclusive {
    /// The claim's key on the mesh.
    pub key: String,
    /// The runtime that holds it.
    pub runtime: String,
    /// The session that holds it.
    pub session: String,
    /// The paths it excludes others from.
    pub scope: Vec<String>,
    /// What it is for, when the claimant said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The issue it is for, when the claimant said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The board claims whose scope meets it, by key: work somebody announced under a path
    /// another session holds exclusively.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meets: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The board in numbers. Every one is the length of a list in the same answer.
pub struct BoardCounts {
    /// Workers, attached or not.
    pub workers: usize,
    /// Workers that are attached.
    pub attached: usize,
    /// Workers that left and whose claims are kept.
    pub departed: usize,
    /// Attached workers that announced nothing.
    pub silent: usize,
    /// Claims.
    pub claims: usize,
    /// Distinct paths named.
    pub paths: usize,
    /// Paths two workers hold.
    pub contested: usize,
    /// Pairs of claims that meet.
    pub collisions: usize,
    /// Exclusive claims the mesh holds.
    pub exclusive: usize,
    /// Checkouts the answer covers.
    pub checkouts: usize,
    /// Checkouts whose board could not be read.
    pub unread: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `peers.board`: the board, whole, and everything that follows from it.
///
/// Nothing in it is stored. The workers and their claims are `peers.list`'s; the paths, the
/// collisions and the counts are derived from them on every read; the exclusive claims are
/// the mesh's own state, read as it is. A reader of `peers.list` could compute all of it,
/// and three surfaces each computed a different part: this is the one place it is said.
pub struct Board {
    /// Whether every board the answer covers was read. `false` means a checkout could not
    /// be asked, `checkouts` says which and why, and every list here is at best a part.
    pub complete: bool,
    /// The board in numbers.
    pub counts: BoardCounts,
    /// Every worker, attached first, then by checkout and by id.
    pub workers: Vec<BoardWorker>,
    /// Every claim of every worker, newest first.
    pub claims: Vec<BoardClaim>,
    /// Every path named, contested first, then by path.
    pub paths: Vec<BoardPath>,
    /// Every pair of claims that meet, live first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collisions: Vec<BoardCollision>,
    /// Whether the mesh is active in this checkout. When it is not, `exclusive` is empty
    /// because nothing can hold an exclusive claim, and `mesh_reason` says why.
    pub mesh: bool,
    /// Why the mesh is not active, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_reason: Option<String>,
    /// The exclusive claims the mesh holds now.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusive: Vec<BoardExclusive>,
    /// One entry per checkout the answer covers, reached or not.
    pub checkouts: Vec<BoardView>,
}

/// A worker's key: its checkout and its position there.
fn worker_key(peer: &Peer) -> String {
    let checkout = peer.checkout.as_ref().map_or("", |c| c.id.as_str());
    format!("{checkout}/{}", peer.id)
}

/// A path as the board compares it: trimmed, without a trailing slash.
fn named(path: &str) -> String {
    path.trim().trim_end_matches('/').to_string()
}

/// The board, derived from a listing and the exclusive claims held now.
///
/// Pure: the same listing and the same claims give the same board, in the same order. The
/// order of every list is stated on its field and decided here, by the keys the lists
/// themselves carry, so that two reads of an unchanged board are the same bytes.
pub(crate) fn board_from(
    list: PeerList,
    mesh: bool,
    mesh_reason: Option<String>,
    held: Vec<BoardExclusive>,
) -> Board {
    let mut workers: Vec<BoardWorker> = Vec::with_capacity(list.peers.len());
    let mut claims: Vec<BoardClaim> = Vec::new();
    for peer in &list.peers {
        let key = worker_key(peer);
        let checkout = peer.checkout.as_ref();
        workers.push(BoardWorker {
            key: key.clone(),
            peer: peer.id.clone(),
            client: peer.client.name.clone(),
            version: peer.client.version.clone(),
            worktree: checkout.map(|c| c.worktree.clone()).unwrap_or_default(),
            branch: checkout.and_then(|c| c.branch.clone()),
            this_checkout: checkout.is_some_and(|c| c.this_checkout),
            attached: peer.attached,
            connected_at: peer.connected_at.clone(),
            last_seen_seconds_ago: peer.last_seen_seconds_ago,
            claims: peer.claims.len(),
        });
        for claim in &peer.claims {
            claims.push(BoardClaim {
                key: format!("{key}#{}", claim.name.as_deref().unwrap_or("")),
                worker: key.clone(),
                name: claim.name.clone(),
                intent: claim.intent.clone(),
                scope: claim.scope.iter().map(|p| named(p)).collect(),
                at: claim.at.clone(),
                attached: peer.attached,
                collides: Vec::new(),
            });
        }
    }

    // every pair of claims of two different workers, each pair once, the earlier first
    let mut order: Vec<usize> = (0..claims.len()).collect();
    order.sort_by(|a, b| {
        (claims[*a].at.as_str(), claims[*a].key.as_str())
            .cmp(&(claims[*b].at.as_str(), claims[*b].key.as_str()))
    });
    let mut collisions: Vec<BoardCollision> = Vec::new();
    for (n, later) in order.iter().enumerate() {
        for earlier in order.iter().take(n) {
            let (first, second) = (&claims[*earlier], &claims[*later]);
            if first.worker == second.worker {
                continue;
            }
            let mut paths: Vec<BoardMeeting> = Vec::new();
            for a in &first.scope {
                for b in &second.scope {
                    if claims_meet(a, b) {
                        paths.push(BoardMeeting {
                            first: a.clone(),
                            second: b.clone(),
                        });
                    }
                }
            }
            if !paths.is_empty() {
                collisions.push(BoardCollision {
                    first: first.key.clone(),
                    second: second.key.clone(),
                    live: first.attached && second.attached,
                    paths,
                });
            }
        }
    }
    collisions.sort_by(|a, b| {
        (!a.live, a.first.as_str(), a.second.as_str()).cmp(&(
            !b.live,
            b.first.as_str(),
            b.second.as_str(),
        ))
    });
    for collision in &collisions {
        for (mine, theirs) in [
            (&collision.first, &collision.second),
            (&collision.second, &collision.first),
        ] {
            if let Some(claim) = claims.iter_mut().find(|c| &c.key == mine) {
                if !claim.collides.contains(theirs) {
                    claim.collides.push(theirs.clone());
                }
            }
        }
    }
    for claim in &mut claims {
        crate::order::canonical_strings(&mut claim.collides);
    }

    // every path named, with who named it; contested when a second worker holds it, by the
    // same path or by one above or below it
    let mut by_path: std::collections::BTreeMap<String, (Vec<String>, Vec<String>)> =
        std::collections::BTreeMap::new();
    for claim in &claims {
        for path in &claim.scope {
            if path.is_empty() {
                continue;
            }
            let entry = by_path.entry(path.clone()).or_default();
            if !entry.0.contains(&claim.key) {
                entry.0.push(claim.key.clone());
            }
            if !entry.1.contains(&claim.worker) {
                entry.1.push(claim.worker.clone());
            }
        }
    }
    let mut paths: Vec<BoardPath> = by_path
        .into_iter()
        .map(|(path, (mut holding, mut held_by))| {
            let contested = held_by.len() > 1
                || collisions.iter().any(|c| {
                    c.paths
                        .iter()
                        .any(|meeting| meeting.first == path || meeting.second == path)
                });
            crate::order::canonical_strings(&mut holding);
            crate::order::canonical_strings(&mut held_by);
            BoardPath {
                path,
                claims: holding,
                workers: held_by,
                contested,
            }
        })
        .collect();
    paths.sort_by(|a, b| (!a.contested, a.path.as_str()).cmp(&(!b.contested, b.path.as_str())));

    // the exclusive claims, each with the board claims announced under it
    let mut exclusive = held;
    for claim in &mut exclusive {
        let mut meets: Vec<String> = claims
            .iter()
            .filter(|c| {
                c.scope
                    .iter()
                    .any(|mine| claim.scope.iter().any(|theirs| claims_meet(mine, theirs)))
            })
            .map(|c| c.key.clone())
            .collect();
        crate::order::canonical_strings(&mut meets);
        claim.meets = meets;
    }
    exclusive.sort_by(|a, b| a.key.cmp(&b.key));

    workers.sort_by(|a, b| {
        (!a.attached, a.worktree.as_path(), a.peer.as_str()).cmp(&(
            !b.attached,
            b.worktree.as_path(),
            b.peer.as_str(),
        ))
    });
    claims.sort_by(|a, b| (b.at.as_str(), a.key.as_str()).cmp(&(a.at.as_str(), b.key.as_str())));

    let counts = BoardCounts {
        workers: workers.len(),
        attached: workers.iter().filter(|w| w.attached).count(),
        departed: workers.iter().filter(|w| !w.attached).count(),
        silent: workers
            .iter()
            .filter(|w| w.attached && w.claims == 0)
            .count(),
        claims: claims.len(),
        paths: paths.len(),
        contested: paths.iter().filter(|p| p.contested).count(),
        collisions: collisions.len(),
        exclusive: exclusive.len(),
        checkouts: list.boards.len(),
        unread: list.boards.iter().filter(|b| b.reason.is_some()).count(),
    };
    Board {
        complete: list.complete,
        counts,
        workers,
        claims,
        paths,
        collisions,
        mesh,
        mesh_reason,
        exclusive,
        checkouts: list.boards,
    }
}

fn peers_board(ctx: &Context, input: PeerListInput) -> Result<Board, CapabilityError> {
    let list = peers_list(ctx, input)?;
    let (mesh, mesh_reason, held) = match ctx.mesh.cooperation() {
        None => (false, ctx.mesh.cooperation_status().reason, Vec::new()),
        Some(cooperation) => {
            let held = cooperation
                .state()
                .claims
                .into_iter()
                .filter(|claim| {
                    claim.mode == crate::mesh::journal::ClaimMode::Exclusive
                        && matches!(claim.state, crate::mesh::state::ClaimState::Held)
                })
                .map(|claim| BoardExclusive {
                    key: claim.key,
                    runtime: claim.runtime,
                    session: claim.session,
                    scope: claim.scope,
                    intent: claim.intent,
                    issue: claim.issue,
                    meets: Vec::new(),
                })
                .collect();
            (true, None, held)
        }
    };
    Ok(board_from(list, mesh, mesh_reason, held))
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
                id: "peers.board",
                title: "The board, whole",
                description: "The board of this repository with everything that follows from it, derived on every read and stored nowhere: every worker of every checkout with where it works and whether it is still attached; every claim of every worker, not only each worker's latest, newest first; every path somebody named with who named it and whether two workers hold it; every pair of claims that meet, with both sides named; the exclusive claims the mesh holds now, each with the board claims announced under it; and the checkouts whose board could not be read. 'complete' is false when one could not, so a short board is never mistaken for an empty repository. 'checkouts: this' reads one board and asks nothing else.",
                input: PeerListInput,
                output: Board,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_board"), http: get("/api/v1/peers/board"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_board,
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
            ("peers.board", "majordomus_board", "/api/v1/peers/board"),
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
    fn checkout(id: &str, worktree: &str, branch: &str, this: bool) -> serde_json::Value {
        serde_json::json!({ "id": id, "worktree": worktree, "branch": branch, "this_checkout": this })
    }

    fn worker(
        id: &str,
        attached: bool,
        at: serde_json::Value,
        claims: &[(&str, &str, &[&str], &str)],
    ) -> Peer {
        let claims: Vec<serde_json::Value> = claims
            .iter()
            .map(|(name, intent, scope, when)| {
                let mut claim = serde_json::json!({ "intent": intent, "scope": scope, "at": when });
                if !name.is_empty() {
                    claim["name"] = serde_json::json!(name);
                }
                claim
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "id": id,
            "client": { "name": "unit", "version": "1" },
            "transport": "http",
            "connected_at": "2026-10-13T10:00:00Z",
            "last_seen_seconds_ago": 3,
            "attached": attached,
            "claims": claims,
            "checkout": at,
        }))
        .expect("a peer as the board serialises it")
    }

    fn listing(peers: Vec<Peer>, boards: Vec<BoardView>, complete: bool) -> PeerList {
        PeerList {
            count: peers.iter().filter(|p| p.attached).count(),
            caller: None,
            overlaps: overlaps_among(&peers),
            peers,
            boards,
            complete,
        }
    }

    /// Every claim of every worker is on the board, not only each worker's latest; two
    /// boards' `p1` are two workers; and a pair of claims that meet names both sides, once.
    #[test]
    fn the_board_holds_every_claim_and_names_both_sides_of_a_collision() {
        let a = checkout("aaa", "/r", "master", true);
        let b = checkout("bbb", "/r-wt/x", "feature/x", false);
        let board = board_from(
            listing(
                vec![
                    worker(
                        "p1",
                        true,
                        a.clone(),
                        &[
                            (
                                "ui",
                                "the screen",
                                &["src/ui", "docs/UI.md"],
                                "2026-10-13T10:01:00Z",
                            ),
                            ("plan", "the plan", &["plan/"], "2026-10-13T10:05:00Z"),
                        ],
                    ),
                    worker(
                        "p1",
                        true,
                        b.clone(),
                        &[(
                            "",
                            "one file of it",
                            &["src/ui/mesh.rs"],
                            "2026-10-13T10:03:00Z",
                        )],
                    ),
                    worker("p2", true, a, &[]),
                ],
                Vec::new(),
                true,
            ),
            false,
            Some("the mesh is off".into()),
            Vec::new(),
        );
        assert_eq!(board.counts.workers, 3);
        assert_eq!(board.counts.silent, 1, "p2 attached and said nothing");
        assert_eq!(
            board.counts.claims, 3,
            "both of the first worker's claims are there"
        );
        let keys: Vec<&str> = board.claims.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            ["aaa/p1#plan", "bbb/p1#", "aaa/p1#ui"],
            "newest first, and two boards' p1 are two workers"
        );
        assert_eq!(board.collisions.len(), 1);
        let pair = &board.collisions[0];
        assert_eq!(
            (pair.first.as_str(), pair.second.as_str()),
            ("aaa/p1#ui", "bbb/p1#")
        );
        assert!(pair.live);
        assert_eq!(
            pair.paths,
            [BoardMeeting {
                first: "src/ui".into(),
                second: "src/ui/mesh.rs".into()
            }]
        );
        let ui = board.claims.iter().find(|c| c.key == "aaa/p1#ui").unwrap();
        assert_eq!(ui.collides, ["bbb/p1#"]);
        let contested: Vec<&str> = board
            .paths
            .iter()
            .filter(|p| p.contested)
            .map(|p| p.path.as_str())
            .collect();
        assert_eq!(
            contested,
            ["src/ui", "src/ui/mesh.rs"],
            "both ends of the meeting"
        );
        assert_eq!(board.paths[0].path, "src/ui", "contested paths come first");
        assert!(board.paths.iter().any(|p| p.path == "plan" && !p.contested));
        assert!(!board.mesh);
        assert_eq!(board.mesh_reason.as_deref(), Some("the mesh is off"));
    }

    /// A worker never collides with itself, a collision with a worker that left is kept and
    /// marked, and an exclusive claim names the board claims announced under it.
    #[test]
    fn a_departed_worker_and_an_exclusive_claim_are_shown_for_what_they_are() {
        let a = checkout("aaa", "/r", "master", true);
        let board = board_from(
            listing(
                vec![
                    worker(
                        "p1",
                        false,
                        a.clone(),
                        &[("", "left with this", &["site"], "2026-10-13T09:00:00Z")],
                    ),
                    worker(
                        "p2",
                        true,
                        a,
                        &[
                            ("one", "the site", &["site/data"], "2026-10-13T10:00:00Z"),
                            ("two", "the same site", &["site"], "2026-10-13T10:01:00Z"),
                        ],
                    ),
                ],
                Vec::new(),
                true,
            ),
            true,
            None,
            vec![BoardExclusive {
                key: "s/c-1".into(),
                runtime: "r".into(),
                session: "landing".into(),
                scope: vec!["site/data/nav.toml".into()],
                intent: Some("landing".into()),
                issue: None,
                meets: Vec::new(),
            }],
        );
        assert_eq!(board.counts.departed, 1);
        assert_eq!(
            board.collisions.len(),
            2,
            "p2's two claims each meet p1's, never each other"
        );
        assert!(board
            .collisions
            .iter()
            .all(|c| !c.live && c.first == "aaa/p1#"));
        assert_eq!(
            board.workers[0].peer.as_str(),
            "p2",
            "attached workers come first"
        );
        assert_eq!(board.counts.exclusive, 1);
        assert_eq!(
            board.exclusive[0].meets,
            ["aaa/p1#", "aaa/p2#one", "aaa/p2#two"],
            "every board claim announced under the exclusive path"
        );
    }

    /// A board that could not be read is counted and the answer says it is not whole; the
    /// same listing gives the same board, byte for byte.
    #[test]
    fn an_unread_board_is_counted_and_the_derivation_is_stable() {
        let unread = || BoardView {
            checkout: serde_json::from_value(checkout("ccc", "/r-wt/y", "fix/y", false)).unwrap(),
            standing: ServerStanding::Starting,
            url: None,
            attached: 0,
            reason: Some("its server is starting".into()),
        };
        let build = || {
            board_from(
                listing(
                    vec![worker(
                        "p1",
                        true,
                        checkout("aaa", "/r", "master", true),
                        &[("", "x", &["a/", "a/"], "2026-10-13T10:00:00Z")],
                    )],
                    vec![unread()],
                    false,
                ),
                false,
                None,
                Vec::new(),
            )
        };
        let board = build();
        assert!(!board.complete);
        assert_eq!((board.counts.checkouts, board.counts.unread), (1, 1));
        assert_eq!(
            board.paths.len(),
            1,
            "a path named twice by one claim is one path"
        );
        assert_eq!(board.paths[0].path, "a", "without its trailing slash");
        assert_eq!(
            serde_json::to_string(&board).unwrap(),
            serde_json::to_string(&build()).unwrap()
        );
    }
}
