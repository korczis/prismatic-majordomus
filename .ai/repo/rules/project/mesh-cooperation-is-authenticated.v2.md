---
id: project.mesh-cooperation-is-authenticated
version: 2
kind: rule
title: Mesh cooperation is admitted per link, replicated through one journal, and projected by every surface alike
description: A runtime cooperates only over a link whose handshake proved the peer's key live, its repository identical, its protocol compatible and its key trusted; everything cooperative travels as signed events of the one journal, folds into one state, and reaches the CLI, HTTP, OpenAPI, MCP and the Cockpit through the mesh capability module alone. Version 2, 2026-10-23 - an exclusive claim is suspended while its session makes no progress (I3990).
statement: Discovery never admits a link; a link is admitted only by the handshake in mesh::cooperation — signature, freshness, nonce, protocol, self, repository identity, trust — and every refusal is typed and recorded; sessions, claims, handovers and reviews cross runtimes only as origin-signed events of mesh::journal::Journal, deduplicated by (stream, seq) and folded by mesh::state::fold, whose exclusivity rule is the same for a claim of this machine and of any other; liveness is a beat that expires on the local clock — a stream's beat for its runtime and, for a session that ever beat, a session beat written only while the session does real work, so an exclusive claim is suspended and refuses nobody when its worker stops even while its server beats on; and no surface holds peers, links, claims or connection state of its own.
status: active
class: blocking
depends_on: [project.mesh-is-observation-not-authority@1]
tags: [mesh, security, cooperation, doctrine]

x-majordomus:
  tests: [scripts/ci/mesh-check, apps/majordomus-cli/tests/mesh_cooperation.rs, test/mesh-lab/run, test/cases/3990_a_claim_lapses_when_its_session_stops_progressing.sh]
---

# Rationale

Discovery says a key exists somewhere on a network; it cannot say that the key is
answering now, serves the same repository, speaks a compatible protocol, or belongs to
someone this repository trusts. A mesh that treats being heard as being one of us lets any
host on the segment inject claims and handovers; a mesh whose transports, surfaces and
tests each keep their own idea of who is linked ends with the Cockpit showing a green dot
for a runtime the CLI knows is gone. ADR 0067 closes both: admission is one handshake with
typed refusals, replication is one signed journal folded by one pure function, and every
surface is a projection of those.

The exclusivity rule is the product's promise to people running several sessions and
several machines on one repository: an exclusive claim excludes everywhere or nowhere.
Two implementations — one for the local board, one for remote peers — would make that
promise depend on where the other worker happens to run.

## An exclusive claim lives by its holder's progress (version 2)

Version 1 let a claim live while its session was open and its stream beat. A stream's beat
proves the runtime is up and nothing about the worker behind one of its sessions, and on
2026-10-22 that difference stalled the repository: the executor of seat 5d held the
integration lease — an exclusive claim — for hours and never answered, because its server beat
on while the worker waited on nobody. The owner ordered the same day that no work may stall
because a peer waits on a permission dialog, crashed or hung. Version 2 adds the second layer
of liveness (I3990, ADR 0067 amendment of 2026-10-23): a session that does real work — an MCP
request other than `ping` and `notifications/*`, an announcement, a claim, or a `mesh beat`
naming the phase a long-running process is in — has a `session_beat` carrying a ttl written
for it; every runtime ages the newest one on its own clock; and an exclusive claim whose
session's beat is older than its ttl (plus the expiry, on any runtime but the holder's) is
suspended: it refuses nobody until the session beats again. Nothing is closed and nothing is
taken over — suspension stops the refusal and says so, and fencing a resumed holder waits for
an epoch (I3174). A session that never beat keeps version 1's rule, so a command-line claim
and a claim of an older runtime are judged exactly as before.

# Required behaviour

1. A link is established only by `Cooperation::dial` and `Cooperation::accept_hello`,
   which verify, in order: message size and shape, the Ed25519 signature under the hello
   (or welcome) domain, the timestamp against the skew window, the nonce against replay,
   the negotiated protocol range, that the peer is not this runtime, that the peer's mesh
   repository id equals this runtime's, and that the peer's key is trusted by the declared
   policy. Each failure is a `RefusalCode`, counted and listed with its detail.
2. The mesh repository id is derived from the repository's root commits or declared as
   `cooperation.repository`; it is never derived from a filesystem path, an address or a
   host name. Runtimes of different repository ids never link.
3. Sessions, claims, handovers and reviews cross runtimes only as `MeshEvent`s signed by
   their origin's node key. A relay stores and forwards bytes; an event whose signature,
   key-to-stream binding, repository or bounds do not hold is refused at every consumer.
   Events are identified by `(stream, seq)`, applied in sequence order per stream, and a
   duplicate changes nothing.
4. Claims are decided by `mesh::state::fold` and `admission_conflicts`, over paths that
   meet by `peers::claims_meet`. A claim lives while it is unreleased, its session open and
   its stream's beat rose within the expiry. An exclusive claim of a session that ever wrote
   a `session_beat` with a ttl above 0 is, in addition, `suspended` — not live, refused by
   nobody, the session and its advisory claims untouched — while that session's newest beat
   is older than its ttl on the holder's runtime, or its ttl and the expiry on any other
   (`mesh::state::fold_with`, `Journal::session_progress`), and holds again at its next
   beat — unless another session's exclusive claim over its scope was admitted meanwhile:
   then the holder's runtime releases it, saying why, before that beat, so two sessions
   never both hold one scope. A beat is written only for real work — an MCP request other than `ping`, an
   announcement, a claim, or `mesh.beat` with its phase; a `ping` never counts. Concurrent
   exclusive claims resolve to the lowest `(lamport, stream, seq)` everywhere and the others
   are named conflicts.
5. The CLI, HTTP, OpenAPI, MCP and the Cockpit read and write cooperation only through the
   capabilities of `capability::builtin::mesh`. No surface keeps a peer list, a link
   state, a claim or a liveness verdict of its own; the Cockpit renders `mesh.peers`,
   `mesh.state` and `mesh.cooperation`.
6. Nothing a linked peer sends executes anything: the journal carries metadata about
   work; the only file a remote event can cause to be written is a consumed handover, into
   this checkout's handovers directory, by an explicit local command.

# Failure behaviour

`scripts/ci/mesh-check` exits 10 naming a TCP link client or a link handler outside
`src/mesh/` and the mesh capability module, a `Journal` constructed outside `src/mesh/`,
or a Cockpit page that renders mesh state without asking a mesh capability. What a static
check cannot see is held by `apps/majordomus-cli/tests/mesh_cooperation.rs` (separate
processes over TCP: links, isolation, trust refusal, crash expiry and restart, three-node
convergence, hostile messages, two worktrees) and `test/mesh-lab/run` (separate Linux
nodes on a real bridge: multicast discovery, partition and crash recovery), both run in CI.

# Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh` (journal
deduplication and ordering, fold convergence and exclusivity as properties, handshake
refusals, three-runtime relay, partition reconciliation through the in-process transport,
a silent session's exclusive claim suspended while its runtime beats),
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation`,
`test/mesh-lab/run`, and `bash test/run.sh 3990_a_claim_lapses_when_its_session_stops_progressing`
(a served runtime: an exclusive claim over MCP held by tool calls and suspended by pings alone,
an advisory claim untouched, a claim kept by `mesh beat --phase`).
