---
schema: adr/v1
id: adr-0067
kind: adr
title: Mesh cooperation is authenticated links between runtimes and one replicated journal folded the same way everywhere
status: proposed
date: 2026-09-15
tags: [mesh, cooperation, security, distributed]
related:
  - "file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md"
  - "file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md"
  - "file:.ai/repo/adrs/0044-cooperation-is-repository-wide-the-board-is-gathered-not-shared.md"
  - "rule:project.mesh-cooperation-is-authenticated"
  - "rule:project.mesh-is-observation-not-authority"
  - "file:docs/MESH.md"
provenance:
  origin: authored
---

# 67. Mesh cooperation is authenticated links between runtimes and one replicated journal folded the same way everywhere

## Context

ADR 0050 gave the mesh discovery: signed advertisements, one registry, trust as a label.
It deliberately stopped there — "whoever later builds remote operations must bring their
own authorization decision" — and so did the code. Measured on 2026-09-15, the mesh could
not support the product it was presented as:

1. **Nothing cooperative crossed a machine.** Claims lived on one server's peer board
   (ADR 0044 gathers the boards of one machine's checkouts through their lease files);
   handovers were files under one checkout's `.ai/local/`; no event, session or review
   travelled anywhere.
2. **The same repository on two machines was two repositories.** Advertisements carried
   `sha256(git common directory path)`: `/Users/k/repo` and `/home/k/repo` never matched.
3. **Two worktrees of one machine could not see each other.** Both servers hold the same
   node key; the manager skipped the second one's datagrams as "self-heard", and its
   multicast socket could not bind the port the first held.
4. **Presence was a sighting, not a relationship.** A node was "present" for sixty seconds
   after an advertisement, whether or not anything could be exchanged with it.

## Decision

Cooperation is a layer above discovery inside `apps/majordomus-cli/src/mesh/`, with these
load-bearing rules.

1. **Identities are typed and distinct.** A *node* is a machine's key (ADR 0050). A
   *runtime* is one checkout's server on that node: `runtime = digest(checkout id)`, so two
   worktrees are two runtimes and a restarted server is the same runtime. An *instance* is
   one process run. A *stream* is `node-runtime-instance`: the unit that writes events. A
   *session* is named within its stream. The discovery protocol goes to version 2 to carry
   the runtime; a version-2 reader reads version 1, a version-1 reader refuses version 2 as
   `version`.
2. **The repository identity is content, not a path.** `digest(root commits)`, or a declared
   `cooperation.repository` (a shallow clone must declare one; a fork that should not
   cooperate with its origin may). Runtimes of different repository identities never link.
3. **A link is admitted by a handshake, never by discovery.** Hello → welcome, each an
   Ed25519-signed message under its own domain, checking size, shape, signature, freshness,
   nonce replay, protocol range (link protocol 1..1), self, repository identity and trust —
   in both directions — with a typed, counted, listed refusal for each. Candidates come from
   trusted registry records of the same repository and from declared seeds.
4. **Transport is an adapter.** `LinkTransport::post(endpoint, path, message)`; the shared
   server answers HTTP through the crate's existing minimal client (ADR 0032: no HTTP client
   dependency, no TLS, no async). Unit tests put runtimes behind an in-process transport
   running the same protocol; integration tests and the lab use sockets.
5. **Everything cooperative is an event of one journal.** Session opened/closed, claim
   acquired/released, handover published/consumed, review requested/answered — each signed
   by its origin, identified by `(stream, seq)`, stamped with a Lamport clock, persisted to
   `.ai/local/state/mesh/journal.jsonl`. A relay forwards bytes and cannot forge. An event of
   an unknown kind is stored and relayed but not interpreted, so a newer peer never breaks an
   older one's replication.
6. **Replication is marks, not flooding.** A sync round (every heartbeat, signed, under the
   link id with a rising counter) carries the sender's high-water mark per stream and the
   events the receiver lacks; the answer carries the same the other way. Delivery is
   at-least-once and application idempotent; within a stream events apply in sequence order
   (an early arrival waits, bounded, for its gap); across streams the fold orders by
   `(lamport, stream, seq)`. The first sync after a handshake is a full state synchronisation
   by the same mechanism: a newcomer receives what its marks lack, which after compaction is
   the live state, not an infinite history.
7. **Liveness is a beat, judged on the local clock.** Each runtime raises its own beat every
   heartbeat; marks relay beats with how long ago the sender saw them rise. A stream whose
   beat has not risen within the expiry is expired on that runtime: its sessions are expired
   and its claims stop excluding. No two machines' wall clocks are compared. Links have their
   own standing (connecting, connected, degraded, unreachable, expired) with exponential
   backoff capped at the expiry, and a restarted peer re-handshakes into the same table row.
8. **State is one pure fold.** `mesh::state::fold(events, liveness)` yields sessions, claims,
   overlaps, handovers, reviews and a digest. A claim holds while unreleased, its session open
   and its stream alive. Two live exclusive claims of different sessions whose paths meet
   (`peers::claims_meet`, the board's own predicate) are a conflict won by the lowest
   `(lamport, stream, seq)`; admission refuses a claim that would conflict with what this
   runtime already knows. Under a partition both sides may claim; after healing every
   runtime names the same winner. Never "last packet wins".
9. **The board is projected, not replaced.** Every heartbeat, attached board sessions become
   mesh sessions and their announcements advisory claims; ADR 0044's local gathering stays
   the machine-local view.
10. **Every surface is a capability.** `mesh.cooperation`, `mesh.peers`, `mesh.peer`,
    `mesh.state`, `mesh.events`, `mesh.verify` and the commands (sessions, claims,
    handovers, reviews, the two link routes) are declarations of the mesh module; the CLI,
    HTTP, OpenAPI, MCP and the Cockpit are their projections. `scripts/ci/mesh-check` refuses
    a link handler, a transport or a journal outside the subsystem and a Cockpit page that
    reads the runtime directly.

## Alternatives rejected

- **Gossiping the peer board.** The board is positional (`p1` is reused), in memory and
  unsigned; replicating it would replicate its ambiguities. The journal is signed and typed;
  the board is projected into it.
- **A consensus protocol (Raft) for claims.** It would buy strong consistency with a
  majority requirement a two-laptop fleet cannot meet, and an async runtime ADR 0032 refuses.
  Deterministic conflict resolution with named conflicts fits sessions that must keep working
  offline.
- **A central coordinator.** A single point of failure and a canonical store elsewhere —
  refused for the same reasons ADR 0050 refused a mandatory rendezvous.
- **Wall-clock lease expiry.** Machines' clocks disagree; relayed beats aged on the local
  monotonic clock do not need them to agree.
- **mTLS links.** A TLS stack is the dependency ADR 0032 keeps out; signed messages give
  authenticity, and confidentiality is the private network's or the overlay's.

## Consequences

- The mesh now carries metadata about work between trusted runtimes of one repository. The
  `SECURITY.md` posture holds by default — nothing opens without an enabled declaration, and
  a link needs a trusted key — but an enabled mesh with trusted peers sends session intents,
  claim scopes, review subjects and published handover bodies to those peers, unencrypted
  unless the network encrypts. `docs/MESH.md` states it.
- ADR 0050's "a trusted node gains nothing" is refined, not reversed: a linked peer's events
  can refuse a local exclusive claim and can offer a handover a local command may write into
  this checkout. Nothing a peer sends executes anything.
- Discovery protocol 2 is not readable by an executable older than this change: a mixed fleet
  upgrades together, and the refusal is counted as `version` rather than silent.
- A per-checkout journal file exists under `.ai/local/state/mesh/`. It is bounded by
  compaction of long-expired streams (handovers are kept) and never committed.
- The acceptance of these guarantees is automated at three levels: unit and property tests,
  multi-process integration tests over TCP (`tests/mesh_cooperation.rs`), and a container lab
  on a real bridge that partitions and kills nodes (`test/mesh-lab/run`, CI job `mesh-lab`).
