<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `episodes` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `episodes` — Episodes

The execution episodes this shared server holds: one per client that asked for one, opened when the client attaches, kept alive by its own traffic, detached rather than closed when the connection goes, and closed on a deliberate detach, on shutdown, or by the reaper when nothing came back. The episode boundary for a client with no provider hooks of its own (ADR 0043).

Stability: behaviorally_verified. Capabilities: 3.

## `episodes.attach` — Attach this connection to an episode

Open an execution episode for the calling client, or resume the one it already had. 'external_id' is the client's own durable name for the sitting — its conversation or thread id — and must survive a reconnect: a client that comes back under the same identity is given its own episode again, under whatever peer id it now has, rather than a second one. This server's peer id will not do; it is handed out per connection. After this the client's own traffic keeps the episode alive, losing the connection detaches rather than closes it, and it is closed by majordomus_session_detach, by the server stopping, or by the reaper once the reattach grace has passed. The repository's episode is opened by the same command a provider hook runs, and what it reported is in the answer. Needs an MCP session: over plain HTTP there is no connection to bind to.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_session_attach` |
| HTTP | `POST /api/v1/episodes/attach` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::episodes |
| tags | episodes, session, coordination |

| input | type | required | description |
|---|---|---|---|
| `external_id` | string | yes | The client's own durable name for this episode — its conversation id, thread id, or
whatever it calls the sitting it is in.

**It must survive a reconnect**, because that is the whole of its job: a client that
comes back under the same identity is given its own episode again rather than a
second one. Anything the client can reproduce will do; what will not do is this
server's peer id, which is handed out per connection and is a different string every
time the client reconnects. |

Output: `Attached`.

## `episodes.detach` — Close this connection's episode

Close an execution episode deliberately: the work is done, not merely interrupted, and the repository's record says so. Without 'external_id' it is whichever episode the calling connection holds. A client that simply goes away does not need this — its episode detaches and the reaper closes it as interrupted — and the difference between those two records is the one thing about an ended episode that changes what somebody does next.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_session_detach` |
| HTTP | `POST /api/v1/episodes/detach` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::episodes |
| tags | episodes, session, coordination |

| input | type | required | description |
|---|---|---|---|
| `external_id` | string or null | no | The episode to close. Omitted, it is whichever one this connection holds, which is
what a client ending its own sitting means. |

Output: `Detached`.

## `episodes.list` — List episodes

Every execution episode this shared server holds, open and detached, with the connection holding each, when its client last spoke, how many connections have carried it, and what the repository's own episode command reported. In-memory: what survives the process is the repository's session record.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_episodes` |
| HTTP | `GET /api/v1/episodes` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::episodes |
| tags | episodes, session, coordination |

Input: none.

Output: `EpisodeList`.

