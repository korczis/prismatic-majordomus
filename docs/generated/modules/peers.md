<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `peers` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `peers` — Peers

The workers of this repository, named by their own initialize, and what each announced it is working on — gathered from the board of every checkout, because a server serves a checkout and a repository worked on through linked worktrees has one board per worktree. In memory; gone with the processes.

Stability: behaviorally_verified. Capabilities: 2.

## `peers.announce` — Announce what this peer is working on

Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. A peer may hold several claims at once: name one with 'claim' and it stands beside the others, announce under that name again and it is updated, leave it out and this is the peer's one unnamed claim. Name your claims when one session is doing several things at once — subagents share their parent's session, so an unnamed announcement from each of them would replace the last rather than adding to it. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_announce` |
| HTTP | `POST /api/v1/peers/announce` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::peers |
| tags | peers, coordination |

| input | type | required | description |
|---|---|---|---|
| `intent` | string | yes | One line, in the peer's words: the task, the question, the intent. |
| `scope` | array | no | Repository-relative paths the peer expects to touch. Informational: other peers
read it to avoid a collision; nothing here enforces it. |
| `claim` | string or null | no | Which of this peer's claims this is, when the peer holds more than one.

One MCP session is not always one piece of work — a client that fans work out to
subagents shares its session with all of them — and without a name every
announcement replaces the last, so the board ends up describing whichever worker
spoke most recently and the rest of the scope silently stops being claimed. Name a
claim and it stands beside the others; announce under that name again and it is
updated. Leave it out and this is the peer's one unnamed claim, which is what a
single session announcing about itself wants. |

Output: `Announced`.

## `peers.list` — List peers

Every worker of this repository: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, what it announced, and which checkout it is attached to. A server serves one checkout, so the board of a repository worked on through linked worktrees is gathered: this checkout's board out of memory, every other checkout's from the server its lease names, asked for its own board alone. 'boards' says which checkouts were covered and 'complete' whether every one of them could be read, so a short board is never mistaken for an empty repository. 'checkouts: this' reads one board and enumerates, probes and asks nothing else. In-memory on every server; gone with the processes.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_peers` |
| HTTP | `GET /api/v1/peers` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::peers |
| tags | peers, coordination |

| input | type | required | description |
|---|---|---|---|
| `checkouts` | Checkouts | no | Which checkouts the board covers. Absent means `repository`; `this` is the checkout the call reached and nothing else, and it is what a sibling server is asked so that a gather can never ask back. |

Output: `PeerList`.

