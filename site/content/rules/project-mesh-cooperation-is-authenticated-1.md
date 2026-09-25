+++
title = "Mesh cooperation is admitted per link, replicated through one journal, and projected by every surface alike"
description = "Mesh cooperation is admitted per link, replicated through one journal, and projected by every surface alike"
weight = 95
[extra]
kind = "rule"
slug = "project-mesh-cooperation-is-authenticated-1"
identity = "project.mesh-cooperation-is-authenticated@1"
status = "active"
source = ".ai/repo/rules/project/mesh-cooperation-is-authenticated.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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
   its stream's beat rose within the expiry; concurrent exclusive claims resolve to the
   lowest `(lamport, stream, seq)` everywhere and the others are named conflicts.
5. The CLI, HTTP, OpenAPI, MCP and the Cockpit read and write cooperation only through the
   capabilities of `capability::builtin::mesh`. No surface keeps a peer list, a link
   state, a claim or a liveness verdict of its own; the Cockpit renders `mesh.peers`,
   `mesh.state` and `mesh.cooperation`.
6. Nothing a linked peer sends executes anything: the journal carries metadata about
   work; the only file a remote event can cause to be written is a consumed handover, into
   this checkout's handovers directory, by an explicit local command.

## Failure behaviour

`scripts/ci/mesh-check` exits 10 naming a TCP link client or a link handler outside
`src/mesh/` and the mesh capability module, a `Journal` constructed outside `src/mesh/`,
or a Cockpit page that renders mesh state without asking a mesh capability. What a static
check cannot see is held by `apps/majordomus-cli/tests/mesh_cooperation.rs` (separate
processes over TCP: links, isolation, trust refusal, crash expiry and restart, three-node
convergence, hostile messages, two worktrees) and `test/mesh-lab/run` (separate Linux
nodes on a real bridge: multicast discovery, partition and crash recovery), both run in CI.

## Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh` (journal
deduplication and ordering, fold convergence and exclusivity as properties, handshake
refusals, three-runtime relay, partition reconciliation through the in-process transport),
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation`, and
`test/mesh-lab/run`.
{% endraw %}
