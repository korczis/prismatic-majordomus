<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `peers`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `peers` — Peers

The clients attached to this repository's shared server, named by their own initialize, and what each announced it is working on. In memory; gone with the process.

Stability: behaviorally_verified. Capabilities: 2.

## `peers.announce` — Announce what this peer is working on

Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller.

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

Output: `Peer`.

## `peers.list` — List peers

Every client attached to this shared server: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, and what it announced. In-memory, gone with the process.

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

Input: none.

Output: `PeerList`.

