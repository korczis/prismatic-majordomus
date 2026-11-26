---
schema: adr/v1
id: adr-0199
kind: adr
title: "A hub is a declared host service; a working checkout's server stays a client's child"
status: accepted
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md
  - file:.ai/repo/adrs/0044-cooperation-is-repository-wide-the-board-is-gathered-not-shared.md
  - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
  - file:.ai/repo/adrs/0224-no-ordinary-command-builds.md
  - file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md
  - file:.ai/repo/adrs/0035-a-checkout-s-server-is-one-of-the-repository-s-and-its-state.md
  - file:.ai/repo/adrs/0059-the-mesh-is-on-for-this-repository-and-every-session-start-h.md
  - file:.ai/repo/adrs/0161-a-mesh-node-follows-the-published-release.md
  - file:docs/MESH.md
  - rule:project.optional-complexity
  - file:.ai/repo/project/milestones/the-host-knows-its-runtimes.yaml
  - file:.ai/repo/project/issues/I3184.yaml
  - file:.ai/repo/project/intents/nothing-in-the-mesh-waits-on-something-dead.yaml
  - file:.ai/repo/project/milestones/the-runtime-supervises-itself.yaml
  - file:.ai/repo/knowledge/curated/lundra-is-the-always-on-hub.md
provenance:
  origin: authored
  derived_from:
    - session:9ae82e7a-b0e4-44c0-a305-9c69cd1acf6a
    - issue:I3100
    - file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md
    - file:.ai/repo/adrs/0035-a-checkout-s-server-is-one-of-the-repository-s-and-its-state.md
    - file:.ai/repo/adrs/0059-the-mesh-is-on-for-this-repository-and-every-session-start-h.md
    - file:.ai/repo/adrs/0161-a-mesh-node-follows-the-published-release.md
    - file:docs/MESH.md
    - file:.ai/repo/project/milestones/the-host-knows-its-runtimes.yaml
    - file:.ai/repo/project/issues/I3184.yaml
    - file:.ai/repo/project/intents/nothing-in-the-mesh-waits-on-something-dead.yaml
    - file:.ai/repo/project/milestones/the-runtime-supervises-itself.yaml
    - file:.ai/repo/knowledge/curated/lundra-is-the-always-on-hub.md
---

# 199. A hub is a declared host service; a working checkout's server stays a client's child

## Context

ADR 0003 (accepted) refuses a daemon or system service; README says Majordomus is 'not a daemon'. Yet the lundra hub runs as a systemd user unit with --idle 0 (docs/MESH.md), ADR 0161 and I2722 restart hubs under their service manager, and the owner asked for a host-level resident (10-12) and an OS-supervised watchdog (10-17). The 10-07 audit warned that the tool had already grown daemon-like parts its own EXTRACTION_REPORT refused.

## Decision

A machine the mesh declaration names as a hub may run one user-scoped service per repository (systemd user unit or launchd agent written by one typed capability) that ignores the idle exit; every other server remains a client's child under ADR 0003. A host-level resident, if ever built, is the same executable in a declared role with a minimal dependency closure and user-only IPC, never a second server, registry, discovery or update system.

## Alternatives rejected

- A separate general-purpose host server
- A daemon per repository on every machine
- Root or system-wide services
- Keeping the hub's unit hand-made outside the model

## Consequences

ADR 0003 is narrowed, not reversed; README's 'not a daemon' gains its exception; I3186, I3193 and the OS half of supervision wait for this decision.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). Accepted by the owner on 2026-11-25, with the amendment below, answering the gate-G0 question recorded verbatim in the knowledge record `fabric-starts-with-p00-s00-s01` ("Accept, set accepted (Recommended)").

- `chatgpt-majordomus:c240#2`
- `chatgpt-majordomus:c118#3`
- `chatgpt-majordomus:c193#0`

## Amendment, 2026-11-24: the resident is Majordomus Fabric's opt-in host agent

The owner ordered the Majordomus Fabric program on 2026-11-24 and chose this ADR to carry its
decision rather than a new one (the owner's answers are recorded verbatim in the knowledge
record `fabric-starts-with-p00-s00-s01`, pull request #3121). The decision above left a door
open: "a host-level resident, if ever built, is the same executable in a declared role with a
minimal dependency closure and user-only IPC, never a second server, registry, discovery or
update system". This amendment decides what walks through that door. The owner accepted it on
2026-11-25 (gate G0). Host-agent code lands from then on, in the order of the gates below.

**What runs.** At most one *host agent* per OS user and machine, and only when that user opts
in with an explicit service install (a systemd user unit or a launchd agent written by one typed
capability, the same capability that brings the hub's unit into the model). It is this
executable in a declared role. It owns the local transport, the host's process inventory and the
routing between them. It owns no project policy. Inside it, each checkout is a logical
**context**, a client's attachment is a **session**, and work that needs isolation runs in a
**worker** process the host starts and bounds. A context's identity is the checkout's
(`repository.rs` `git_identity`). The node identity stays `mesh/identity.rs`'s key. None of
these is a second registry: the host's context inventory is a process inventory built from
leases (I3184), and `mesh/registry.rs` stays the only holder of peers.

**How it is reached.** A Unix-domain socket (a named pipe on Windows), mode 0600, in the user's
runtime directory, with the peer's credentials checked on accept, is the local control channel.
HTTP, REST, Swagger and MCP over HTTP are projections of the one capability registry, served only
where configured. Local trust is the OS user: another user's process is refused, and every
request is authorised against the context and capability it names. Being on the socket is not
authority over every checkout, as being on loopback is today.

**Between hosts.** Host-to-host links are authenticated and encrypted, the tailnet included. A
link admits the other host and nothing more. Each logical stream to a repository needs that
repository's own enrolment and the capability's own effect check (ADR 0067's per-repository
admission, per stream), and revocation ends live streams. Cleartext hub links remain only for
the legacy mode. The enrolment logic of pull request #3108 is reused as host-transport
admission, so that only the host binds a non-loopback address, and never every checkout.

**What it narrows, and what stays.**

- ADR 0001's "no port, no daemon", ADR 0003's "a lease, not a daemon" and ADR 0044's "one server
  per checkout, no registry of servers, no daemon" are narrowed for the opt-in host mode only.
- Without the service, every behaviour of today stands: the per-checkout server elected through
  its lease, `env enter` starting nothing it must wait for (ADR 0043), and no ordinary command
  that builds (ADR 0224).
- A checkout whose server a legacy binary owns stays that server's. The host proxies to it and
  never takes its mutable ownership while a writer that does not check the host's fencing could
  run. A cutover happens only when every writer of that context is proven to take part.

**Updates.** The host follows releases through ADR 0161's one installer and pull request #3110's
follower. An unattended activation requires verified provenance and refuses without it. A manual
`self update` keeps today's behaviour, with a warning.

**Gates.** The order and the proofs are the corrected program's gates:

- G0, governance: this amendment, accepted;
- G1, legacy measurement: `scripts/fabric-baseline`, case 4663;
- G2, no second mutator;
- G3, a hung context does not starve the host;
- G4, scoped authority;
- G5, the local pilot: 2 repositories, 10 worktrees, 20 MCP sessions, with standalone fallback.

Remote mutation waits for G6 and G7, and an unattended update for G8. No efficiency claim is
made beyond what G1 and G5 measure.

**Alternatives rejected.**

- A new ADR beside this one: the owner chose to amend this one.
- A resident that also schedules, parses and builds: a host is a small, responsive control
  plane, and heavy work goes to workers.
- Authorising by the OS user alone: one user's socket must not grant every context's
  capabilities.
- An authenticated host link standing for repository admission.
- Moving a live legacy checkout's ownership on a lock or an epoch that old binaries do not read.
