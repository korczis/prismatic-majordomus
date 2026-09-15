<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `mesh` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `mesh` — Mesh

The mesh of this process: discovery — authenticated observations of other running Majordomus instances in one registry — and cooperation — authenticated links to trusted runtimes of the same repository, replicating sessions, claims, handovers and reviews through one journal every runtime folds into the same state. Discovery grants nothing; a link is admitted per peer, and nothing a peer sends executes anything here.

Stability: experimental. Capabilities: 21.

## `mesh.claim` — Claim a scope across the mesh

Claim repository paths for a session. An exclusive claim that meets a live exclusive claim of another session — on this runtime or any runtime this one has heard — is refused as `claim_conflict` with the claims it meets; an advisory claim is recorded and its overlaps reported. A claim lives while its session is open and its runtime beats: a crashed holder's claim expires everywhere on its own. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_claim` |
| HTTP | `POST /api/v1/mesh/claims` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `session` | string or null | no | The claiming session; the calling MCP session's own when omitted. Opened when new. |
| `client` | string or null | no | The client, for a session this opens; `cli` when omitted. |
| `scope` | array | yes | Repository-relative paths. |
| `intent` | string or null | no | What the claim is for. |
| `mode` | object | no | `exclusive` (the default) or `advisory`. |
| `issue` | string or null | no | The issue the claim is for. |
| `task` | string or null | no | The task, for a session this opens. |

Output: `Written`.

## `mesh.cooperation` — Cooperation, at a glance

Whether this runtime cooperates and why not when it does not: its runtime and stream, the repository identity links are matched on, the link protocol and features, the heartbeat and expiry, every linked peer with its link state, last exchange, round trip and failures, every refused candidate with the rule that refused it, and the counters of handshakes, syncs, reconnects, expiries, replicated and refused events.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_cooperation` |
| HTTP | `GET /api/v1/mesh/cooperation` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

Input: none.

Output: `CooperationStatus`.

## `mesh.doctor` — The mesh self-check

Every prerequisite proved on this machine alone: the declaration parses, the identity loads, the repository has a mesh identity, the advertised endpoints are reachable from beyond this machine, a UDP socket binds, the multicast group joins, broadcast enables, and the discovery and link protocols sign and verify end to end in memory. Each failed check names its impact and its remedy. Deterministic, no second node required.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_doctor` |
| HTTP | `GET /api/v1/mesh/doctor` |
| CLI | `majordomus mesh doctor` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, diagnostics |

Input: none.

Output: `MeshDoctorReport`.

## `mesh.events` — The cooperation journal

The journal's events above a Lamport stamp, in Lamport order, at most a page: each with its stream, sequence, stamp, repository, signing key, kind and body. The answer's lamport is the next page's `after`.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_events` |
| HTTP | `GET /api/v1/mesh/events` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `after` | integer or null | no | Only events whose Lamport stamp is above this. |
| `limit` | integer or null | no | At most this many (default 100, at most 1000). |

Output: `EventList`.

## `mesh.handover.consume` — Consume a handover from the mesh

Take a handover another runtime published: record the consumption on the mesh, and write it into this checkout's handovers directory as a record `majordomus handover --resolve` finds on the same branch — once, however often it is consumed.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_handover_consume` |
| HTTP | `POST /api/v1/mesh/handovers/consume` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, continuity, cooperation |

| input | type | required | description |
|---|---|---|---|
| `handover` | string | yes | The handover's id (its content digest). |
| `session` | string or null | no | The consuming session; the calling MCP session's own when omitted. |
| `materialize` | boolean or null | no | Whether to write it as a local handover record `majordomus handover --resolve`
finds (default true). |

Output: `ConsumeAnswer`.

## `mesh.handover.publish` — Publish a handover to the mesh

Publish a handover record of this checkout — the newest, or the one named under .ai/local/state/handovers/ — to every linked runtime: its task, branch, head and time from its front matter, the issue and milestone given, and its Markdown body, bounded and identified by the body's digest. No path of this machine travels, and no file outside the handovers directory is ever read.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_handover_publish` |
| HTTP | `POST /api/v1/mesh/handovers` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, continuity, cooperation |

| input | type | required | description |
|---|---|---|---|
| `path` | string or null | no | The record, relative to the repository root and inside
`.ai/local/state/handovers/`; the newest record when omitted. Nothing outside that
directory is ever read by this command. |
| `issue` | string or null | no | The issue the handover belongs to. |
| `milestone` | string or null | no | The milestone it belongs to. |

Output: `Written`.

## `mesh.identity` — This machine's node identity

The node identity kept under the user's state directory, public half only: node id, public key, display name. The signing key appears in no projection. Absent is an answer, not an error — the identity is created when a mesh first activates.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_identity` |
| HTTP | `GET /api/v1/mesh/identity` |
| CLI | `majordomus mesh identity` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, identity |

Input: none.

Output: `MeshIdentityReport`.

## `mesh.link.hello` — Open a link (the handshake)

The link handshake another runtime posts: a signed hello carrying its runtime card, a fresh nonce and its protocol range. Refused, typed, when malformed, oversized, mis-signed, stale, replayed, of an unsupported protocol, of another repository, untrusted, or this runtime itself; otherwise answered with a signed welcome: this runtime's card, a link id, its marks and the echoed nonce. Changes this process's link table only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| HTTP | `POST /api/v1/mesh/link/hello` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, cooperation, link |

| input | type | required | description |
|---|---|---|---|
| `body` | object | yes | The message. |
| `sig` | string | yes | Hex Ed25519 over the domain and the canonical JSON of `body`. |

Output: `LinkReply`.

## `mesh.link.sync` — One sync round of a link

The replication round a linked runtime posts every heartbeat, signed under its link id with a rising counter: its marks and the events this runtime lacks. Ingested — verified end to end, deduplicated, applied in stream order — and answered, signed, with this runtime's marks and the events the peer lacks. An unknown link or a restarted peer is told to say hello again.

| | |
|---|---|
| kind | command |
| stability | experimental |
| HTTP | `POST /api/v1/mesh/link/sync` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, cooperation, link |

| input | type | required | description |
|---|---|---|---|
| `body` | object | yes | The message. |
| `sig` | string | yes | Hex Ed25519 over the domain and the canonical JSON of `body`. |

Output: `LinkReply`.

## `mesh.nodes` — The discovered nodes

Every runtime this process has observed, one record per node and runtime slot, deduplicated across every discovery source, in node-id order: identity, trust, presence, endpoints, capabilities, repositories, provenance and versions. In memory, gone with the process.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_nodes` |
| HTTP | `GET /api/v1/mesh/nodes` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

Input: none.

Output: `NodeList`.

## `mesh.peer` — One runtime

One runtime by its key (`<node>-<runtime>`, or a node id for its first runtime): the machine it runs on, its liveness, its link from here, its sessions and claims, and any refusal recorded against it. Not found is an answer with the reason.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_peer` |
| HTTP | `GET /api/v1/mesh/peer` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `runtime` | string | yes | The runtime, `<node>-<runtime>` (a node id alone matches its first runtime). |

Output: `PeerDetail`.

## `mesh.peers` — Who cooperates, machine by machine

Machine → runtime → session → claim: every runtime this one is linked to or has heard through the journal, grouped by machine, local first; each runtime with its liveness, the milliseconds since its beat rose and its link, each session with what it said and its claims. Remote and local runtimes share one shape, and the state digest closes the answer: equal digests, equal state.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_peers` |
| HTTP | `GET /api/v1/mesh/peers` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

Input: none.

Output: `PeerTree`.

## `mesh.register` — Register with this node's mesh

Present one signed envelope; it is verified exactly as a datagram — bounds, staleness, signature, trust policy — and recorded as a rendezvous observation when it holds. The answer carries this node's own envelope and the candidates its registry holds, each verifiable end to end on its own signature. Registration grants nothing: the caller becomes a record, never an authorization. Changes this process's memory only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_register` |
| HTTP | `POST /api/v1/mesh/register` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

| input | type | required | description |
|---|---|---|---|
| `envelope` | object | yes | The envelope. Verified here exactly as a datagram would be — bounds, staleness,
signature — before anything is recorded. |

Output: `RegisterAnswer`.

## `mesh.release` — Release a claim

Release a claim this runtime's current run holds, by its key. A claim written elsewhere is refused as `not_own`: only its holder releases it, and a dead holder's claim expires instead. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_release` |
| HTTP | `POST /api/v1/mesh/claims/release` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `claim` | string | yes | The claim's key, `<stream>/<claim>`, as `mesh.claim` answered it. |

Output: `Written`.

## `mesh.review.answer` — Answer a review request

Answer a review request from any runtime with approved, changes_requested or commented, and a note. Replicated to every linked runtime. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_review_answer` |
| HTTP | `POST /api/v1/mesh/reviews/answer` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `request` | string | yes | The request's key, `<stream>/<review>`. |
| `session` | string or null | no | The answering session; the calling MCP session's own when omitted. |
| `verdict` | string | yes | `approved`, `changes_requested` or `commented`. |
| `note` | string or null | no | The note. |

Output: `Written`.

## `mesh.review.request` — Ask the mesh for a review

Ask for a review of a branch, commit or pull request, optionally of one named runtime — which must be linked and carry the `reviews` feature, or the request is refused as `feature_unsupported`. Replicated to every linked runtime. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_review_request` |
| HTTP | `POST /api/v1/mesh/reviews` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `session` | string or null | no | The requesting session; the calling MCP session's own when omitted. |
| `subject` | string | yes | What to review: a branch, a commit, a pull request. |
| `scope` | array | no | The paths it covers. |
| `issue` | string or null | no | The issue. |
| `reviewer` | string or null | no | The runtime asked (`<node>-<runtime>`); anyone when omitted. |

Output: `Written`.

## `mesh.session.close` — Close a session on the mesh

End a session of this runtime; every claim it holds ends with it, on every linked runtime. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_session_close` |
| HTTP | `POST /api/v1/mesh/sessions/close` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `session` | string or null | no | The session; the calling MCP session's own when omitted. |

Output: `Written`.

## `mesh.session.open` — Open or update a session on the mesh

Say what a session of this runtime is: its client, worker, intent, task, issue, milestone, branch, head, dirtiness and context revision. Replicated to every linked runtime; a later word replaces an earlier one. Writes this runtime's journal only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_session_open` |
| HTTP | `POST /api/v1/mesh/sessions` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

| input | type | required | description |
|---|---|---|---|
| `session` | string or null | no | The session id within this runtime; the calling MCP session's own when omitted. |
| `client` | string or null | no | The client (`claude-code`, `codex`, `cli`); `cli` when omitted. |
| `worker` | string or null | no | The worker's name for itself. |
| `intent` | string or null | no | What it is doing. |
| `task` | string or null | no | The task id. |
| `issue` | string or null | no | The issue. |
| `milestone` | string or null | no | The milestone. |
| `branch` | string or null | no | The branch. |
| `head` | string or null | no | The head. |
| `dirty` | boolean or null | no | Whether the working tree is dirty. |
| `context` | string or null | no | The context revision it works from. |

Output: `Written`.

## `mesh.state` — The state every linked runtime converges on

The journal folded: every session, every claim with its standing (held, released, expired, conflicted with its winner), the advisory overlaps, every handover with who consumed it, every review request with its answers, and the digest two runtimes compare. Plus every stream's liveness on this runtime's clock.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_state` |
| HTTP | `GET /api/v1/mesh/state` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, cooperation |

Input: none.

Output: `MeshStateAnswer`.

## `mesh.status` — The mesh, at a glance

Whether the mesh runs in this process and why not when it does not; this node's public identity; every discovery provider with its state and counters; the registry's tallies; and the refused datagrams by reason. The one status every surface shows.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh` |
| MCP resource | `majordomus://mesh` |
| HTTP | `GET /api/v1/mesh` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

Input: none.

Output: `MeshStatus`.

## `mesh.verify` — Prove cooperation now

Run a live verification: cooperation is active, the repository identity resolves, the endpoints are reachable beyond loopback, the heartbeat beats, the journal holds no stuck gap; then one sync round with every peer this runtime dials, timed, and whether both sides now hold the same high-water marks. Each failed check names its impact and remedy. Changes nothing but the journal's replication.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_verify` |
| HTTP | `POST /api/v1/mesh/verify` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, diagnostics, cooperation |

Input: none.

Output: `MeshVerifyReport`.

