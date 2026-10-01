---
schema: adr/v1
id: adr-0059
kind: adr
title: The mesh is on for this repository, and every session start holds it
status: proposed
date: 2026-09-30
tags:
  - mesh
  - coordination
  - operations
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
  - file:.ai/repo/adrs/0044-cooperation-is-repository-wide-the-board-is-gathered-not-shared.md
  - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
  - file:.ai/repo/mesh/majordomus.yaml
  - file:docs/MESH.md
  - file:apps/majordomus-cli/src/mesh/doctor.rs
  - file:lib/capture.sh
  - claim:mesh-declared-is-held
  - test:test/cases/494_the_mesh_is_declared_and_held.sh
  - test:test/cases/491_the_mesh_is_on_here.sh
---

# 59. The mesh is on for this repository, and every session start holds it

## Context

ADR 0050 built the mesh and switched it off: no socket opens until a repository commits
`enabled: true`, because "nothing leaves the machine" is the posture a fresh repository
inherits and an operator's fleet is that operator's decision. ADR 0044 gathered the peer
board across every checkout of one machine and stopped there; ADR 0067 linked the runtimes
of one repository across machines.

Measured on 2026-09-12, on the fleet this repository is developed on — a laptop, an x86_64
desktop and an aarch64 build board on one private network and one tailnet:

- Every machine ran its shared server with `enabled: true`, and every one of them carried
  that as an **uncommitted edit** of `.ai/repo/mesh/majordomus.yaml`. Master said `false`.
  The laptop's primary checkout had been reset to master; its server started from the
  committed declaration, reported `not active: the mesh declaration is disabled`, and the
  two other machines went on seeing a laptop that was not there, advertised by an orphaned
  server of an older session.
- The desktop had a hand-written unit for its server, restarting every two seconds, two
  thousand and seventy-six times, because a server started by hand held the port.
- Nothing said any of this. `mesh status` answered truthfully when asked; the briefing a
  session starts with named the server and not the mesh; `mesh doctor` proved the machine
  could run a mesh and said nothing about whether the server did. A worker had to know to
  ask, and the one who asked was told the mesh was disabled and had to work out that this
  was the drift, not the decision.

The declaration has since been committed enabled, with `deny_unknown`, the three machines'
keys and the two hubs, and `test/cases/491_the_mesh_is_on_here.sh` holds its least-privilege
shape; `mesh doctor` gained a `trust` check that fails on a machine whose key is not listed.
What is still missing is the other half: a server that does not do what the committed
declaration says is silent until somebody asks it the right question.

## Decision

**The mesh is on, as a committed fact of this repository.** `.ai/repo/mesh/majordomus.yaml`
carries `enabled: true`, `deny_unknown` with an allowlist of the fleet's public keys, multicast
on the local segment, and rendezvous endpoints naming the hubs on both networks. A change to
the fleet is a change to that file and to the fleet table of `docs/MESH.md`, in one commit.
The skeleton a fresh repository starts from ships no declaration: this decision is this
repository's, not the tool's.

**The doctor judges the server's decision, not only the machine's ability.** `mesh doctor`
ends with a `runtime` check that reads what the shared server decided: `active as <node>
since …` with providers and tallies; `off, as declared`; or the failure this ADR exists for,
*the declaration is enabled and this server's mesh is not active*, with the server's own
reason, its impact, and the restart that follows fixing it. The mesh runtime carries one bit,
`decided`, set when a server activates it or declines with a reason; a runtime nobody decided
on is the command line's, and the check reports that absence instead of judging it. The
verdict is the server's, so `majordomus mesh doctor` asks the running server for the report
(`GET /api/v1/mesh/doctor`) when one serves the checkout and runs in-process only when none
does. A failed check exits 10 — a mesh declared on and not running is a contract unmet, the
same exit every other unmet contract gets. A mesh that is active with every declared transport
failed is the other failed `runtime` verdict: it hears nothing and is heard by nobody.

**The session start says it.** The briefing the provider's start event injects carries one
`Mesh:` line directly under the `Shared server:` line whenever the repository tracks a
declaration and the server is ready: `active — …` with what the server sees, `off, as
declared`, or `DECLARED ENABLED BUT NOT ACTIVE — …` with the server's reason. A tracked
declaration the index cannot read is `DECLARATION NOT READ — …`, because silence about a
committed declaration is the failure being prevented. The line is the executable's `mesh
doctor` rendered by the hook library; the library sends no request of its own, so SECURITY.md's
one declared request stays one.

**A test holds the declaration and the answers.** `test/cases/494_the_mesh_is_declared_and_held.sh`
refuses a tree whose committed declaration is untracked, disabled, not `deny_unknown`, lists
fewer keys than `docs/MESH.md` names machines, or names no rendezvous hub; then it drives a
disposable repository through the start event to prove no line without a declaration, the three
answers, and exit 10 from `mesh doctor` when the declaration is enabled and the server could not
activate it. `apps/majordomus-cli/tests/mesh.rs` proves the failed verdict over a real server.
The claim is `mesh-declared-is-held`, guaranteed.

## Alternatives rejected

**Feeding mesh nodes into the peer board.** ADR 0050 refused it and the refusal stands: a
session peer and a machine's runtime are different things with different lifetimes, and the
question a worker asks of the board — who holds my paths — has no answer on a node record.

**A policy switch instead of a committed declaration.** `session.mesh_required: true` in the
policy would have been a second place to say the mesh is on, able to disagree with the
declaration. The declaration is the one place, and the doctor and the briefing read it.

**A briefing line from a request of the hook's own.** The hook library could have asked the
server's `/api/v1/mesh` itself. It would have been a second network client in `lib/`, which
SECURITY.md and `test/cases/08_no_forbidden_constructs.sh` refuse, and a second rendering of
the verdict the doctor already makes. The executable renders; the hook quotes.

**Refusing to start a server whose mesh could not activate.** A mesh that cannot start is a
reason, never a failed server (ADR 0050); a checkout must be served whether or not its machine
can reach the fleet. The reason is made loud instead, in the two places a worker already looks.

**Judging an undecided runtime.** The command line's own runtime is never activated, so under
an enabled declaration it is always inactive. Reporting that as a failure would make every
`mesh doctor` without a server exit 10 and teach everyone to ignore it.

## Consequences

- `mesh doctor` may exit 10 where it exited 0: whenever a check fails, and in particular when
  the declaration is enabled and the server's mesh is not active. Scripts that read the report
  keep `--format json`; the answer is printed either way.
- A self-check about the invoking process — its own state directory, its own identity — is
  `majordomus run mesh.doctor`, which never asks a server; case 491 reads it that way.
- A machine joining the fleet is a key in the allowlist and a row in the fleet table of
  `docs/MESH.md`, and for a hub an address in the endpoints; a lost identity is the same edits.
  Each is a commit, and case 494 fails when the table and the allowlist disagree in size.
- The hubs are servers started by hand beyond loopback (`docs/MESH.md`). A script that
  installed them as systemd user units was proposed with this decision and is not part of it:
  new shell automation is refused (ADR 0069, `docs/SHELL.md`), and a hub installer is owed as
  a typed capability or a Rhai workflow. Until then a hub that stops is noticed by the
  `present` count of the machines that registered with it, not by a check.
