---
schema: adr/v1
id: adr-0059
kind: adr
title: The mesh is on for this repository, and every session start holds it
status: proposed
date: 2026-09-12
tags:
  - mesh
  - coordination
  - operations
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0050
    - decision:adr-0044
    - file:.ai/repo/mesh/majordomus.yaml
    - test:test/cases/290_the_mesh_is_declared_and_held.sh
---

# 59. The mesh is on for this repository, and every session start holds it

## Context

ADR 0050 built the mesh and switched it off: no socket opens until a repository commits
`enabled: true`, because "nothing leaves the machine" is the posture a fresh repository
inherits and an operator's fleet is that operator's decision. ADR 0044 gathered the peer
board across every checkout of one machine and stopped there: a worker on another
machine is invisible to the board, and the mesh is the layer that was to say such a
machine exists at all.

Measured on 2026-09-12, on the fleet this repository is developed on — a laptop, an
x86_64 desktop and an aarch64 build board on one private network and one tailnet:

- Every machine ran its shared server with `enabled: true`, and every one of them
  carried that as an **uncommitted edit** of `.ai/repo/mesh/majordomus.yaml`. Master
  said `false`. The laptop's primary checkout had been reset to master; its server
  started from the committed declaration, reported `not active: the mesh declaration is
  disabled`, and the two other machines went on seeing a laptop that was not there,
  advertised by an orphaned server of an older session that no lease named and that
  held the multicast port the live server could not bind.
- The desktop had a systemd unit for the server, restarting every two seconds, two
  thousand and seventy-six times, because a server started by hand held the port and
  the unit's exited 0 at once each time.
- Nothing said any of this. `mesh status` answered truthfully when asked; the briefing
  a session starts with named the server and not the mesh; `mesh doctor` proved the
  machine could run a mesh and said nothing about whether the server did. A worker had
  to know to ask, and the one who asked was told the mesh was disabled and had to work
  out that this was the drift, not the decision.

The decision that was missing was not technical. It was whether this repository's mesh
is on — a reviewed, versioned answer — and, once it is, what refuses the tree when a
machine quietly disagrees.

## Decision

**The mesh is on, as a committed fact of this repository.** `.ai/repo/mesh/majordomus.yaml`
carries `enabled: true`, `deny_unknown` with an allowlist of the three machines' public
keys, multicast on for the segment a machine is on, and rendezvous endpoints naming the
two Linux machines as hubs on both networks. A change to the fleet is a change to that
file, and a commit. The skeleton a fresh repository starts from keeps `enabled: false`
and ADR 0050's posture: this decision is this repository's, not the tool's.

**The doctor judges the server's decision, not only the machine's ability.** `mesh doctor`
gains two checks. `trust` reads whether this machine's own key is on the allowlist it
just parsed — a node that is not on its own repository's allowlist is observed everywhere
and trusted nowhere, and nothing else on the machine would say so. `runtime` reads what
the shared server decided: `active` with providers and tallies; `off, as declared`; or
the failure this ADR exists for, *the declaration is enabled and this server's mesh is
not active*, with the server's reason and the restart that follows fixing it. The verdict
is the server's, so the command asks the running server for the report when one serves
the checkout and runs in-process only when none does, where an undecided runtime is an
absence and not a failure. A failed check exits 10 — a mesh declared on and not running
is a contract unmet, the same exit every other unmet contract gets.

**The session start says it.** The briefing the provider's start event injects carries one
`Mesh:` line whenever a declaration exists, in the block that names the server: `active`
with what the server sees, `off, as declared`, or `DECLARED ENABLED BUT NOT ACTIVE` with
the reason. The line is the executable's `mesh doctor` rendered by the hook library; the
library sends no request of its own, so SECURITY.md's one declared request stays one.

**The hubs are units.** `scripts/mesh-hub` writes, enables and starts a systemd user unit
that runs the checkout's shared server bound beyond loopback with `--idle 0`, restarting
at a sixty-second cadence — the bound that separates a unit polling for its turn from one
restarting two thousand times — and `mesh-hub deploy` fast-forwards, rebuilds, restarts
and waits for the mesh to be active. On macOS it explains why there is no hub there: the
application firewall drops inbound to the unsigned executable, and the laptop registers
outward.

**A test holds the declaration.** `test/cases/290_the_mesh_is_declared_and_held.sh`
refuses a tree whose committed declaration is disabled, untracked, without an allowlist
of at least the fleet's size or without a hub, and drives a disposable repository through
the start event to prove the three briefing answers and the exit code. The claim is
`mesh-declared-is-held`, guaranteed, in `docs/CLAIMS.yaml`.

## Alternatives rejected

**Feeding mesh nodes into the peer board.** ADR 0050 refused it and the refusal stands: a
session peer and a machine's runtime are different things with different lifetimes, and
the question a worker asks of the board — who holds my paths — has no answer on a node
record. The mesh says that a machine exists; the board says what a session is doing.

**A policy switch instead of a committed declaration.** `session.mesh_required: true` in
the policy would have been a second place to say the mesh is on, able to disagree with
the declaration. The declaration is the one place, and the doctor and the briefing read
it.

**A hub as a deployment object.** `--deployment <id>` binds the address a deployment
object declares, and the object generates a container image and a provider
configuration. A hub is a process on a machine the operator owns, not a hosted
deployment, and an object that generated a Dockerfile for it would restate a
`systemd` unit in a form nothing runs.

**Refusing to start a server whose mesh could not activate.** A mesh that cannot start
is a reason, never a failed server (ADR 0050); a checkout must be served whether or not
its machine can reach the fleet. The reason is made loud instead, in the two places a
worker already looks.

**Restarting the hub's unit on every exit at a short cadence.** That is what the desktop
was doing. A server that finds the checkout already served exits 0 at once, and a unit
that treats that as a crash is a loop. The cadence is a minute, so that a hook-started
server holding the lease is asked to yield once a minute and the checkout is back on its
hub within one of its idle-outs.

## Consequences

- This repository's shared servers send signed advertisements to the declared multicast
  group and register with the declared hubs; SECURITY.md names the exception, and the
  envelope carries public facts only (ADR 0050). A fresh repository still sends nothing.
- `mesh doctor` may exit 10 where it exited 0: with an enabled declaration and a server
  whose mesh did not activate, or a machine off its own allowlist. Both are facts a
  worker was previously left to discover.
- A machine joining the fleet is a key in the allowlist and, for a hub, an address in the
  endpoints; a lost identity is the same two edits. Each is a commit.
- One multicast socket per machine: a second server on the same machine (a linked
  worktree's) has a `Failed` multicast provider and rides the rendezvous; the doctor
  names it as degraded and holds. Every server on a machine advertises one node id
  with its own instance, so several servers are one record whose instance moves; the
  `replayed` counter says how often.
- The skeleton `init` writes declares no `mesh-declaration` source class, so a fresh
  repository cannot see a declaration until its `sources.yaml` names where one lives.
  The case copies this repository's; the skeleton's own class is the open end of this
  decision, not taken here because it changes what every repository discovers.
