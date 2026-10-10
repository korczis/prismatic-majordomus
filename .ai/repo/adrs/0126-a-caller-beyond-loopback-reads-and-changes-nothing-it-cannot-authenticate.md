---
schema: adr/v1
id: adr-0126
kind: adr
title: A caller beyond loopback reads, and changes nothing it cannot authenticate
status: proposed
date: 2026-10-09
tags:
  - security
  - mesh
  - mcp
  - http
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md
  - file:.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
  - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
  - rule:project.mesh-cooperation-is-authenticated
  - file:apps/majordomus-cli/src/capability/model.rs
  - file:apps/majordomus-cli/src/http/router.rs
  - file:apps/majordomus-cli/src/mcp/surface.rs
  - test:apps/majordomus-cli/tests/remote_callers.rs
  - test:test/cases/1022_a_remote_caller_cannot_write.sh
---

# 126. A caller beyond loopback reads, and changes nothing it cannot authenticate

## Context

The HTTP server has no accounts and no tokens. Its one guard on a state-changing request was
the `Origin` check (`http/router.rs`), which refuses a browser page from another origin and
nothing else: a program sends no `Origin` and passed it. That was harmless while every server
listened on loopback. It stopped being harmless when the mesh needed servers reachable from
other machines (`docs/MESH.md`, "This repository's mesh") and when `MAJORDOMUS_HTTP_HOST`
let a developer bind every interface.

On 2026-10-09 three servers on the owner's machine listened on every interface: the mesh hub
on 8791 and two servers of the primary checkout. A read of the code (the runtime pack's
baseline audit, M01) found that any host on the LAN or the tailnet could:

- transition a tracked plan record (`plan.transition`), publish or resume continuity, and
  sweep orphans: the capabilities whose effect is `repository_mutation`;
- run any capability, writers included, through `executions.start`;
- post a mesh session, claim, release, handover publication or consumption, or review, which
  this node then signed with its own trusted key and replicated to every runtime of the
  repository. A peer cannot tell such an event from one its owner wrote: the node became a
  signing oracle, and the guarantee of `project.mesh-cooperation-is-authenticated` held only
  for the network hop, not for the author.

The bind warning said a remote host could "read". The documentation said there was no
authentication and that the writers were exposed; nobody acted on that sentence, because
the mesh documentation's security model did not mention it.

## Decision

A request is judged by the address it came from, because that is the one fact the server
has. A request from an address that is not loopback — in either family, including the
IPv4-mapped form a dual-stack socket reports — runs a capability only when:

1. the capability reads (`effect: read`), or
2. the capability's input is a signed message its handler verifies against the trust policy
   before anything changes. This is declared on the capability, as a third documented
   addition to its execution policy (`ExecutionPolicy::authenticates_its_input`, serialised as
   `signed_input`), and only a command whose effect is `process_state` may declare it: a
   signature proves who sent a message, not that its sender may change tracked files here.
   Today exactly three capabilities declare it: `mesh.link.hello`, `mesh.link.sync` and
   `mesh.register`.

Everything else is refused before its handler runs, with one new error category,
`forbidden` (HTTP 403), carrying one sentence on every transport
(`CapabilityError::remote`). The question is asked in one place, `ExecutionPolicy::admits_remote`,
and every transport that knows the caller's address asks it:

- the HTTP router, for every capability route;
- the MCP endpoint over HTTP, **per message** and not per session: a session id is a name,
  not a proof, and the next message of a session opened from loopback may come from
  elsewhere.

A request built inside the process (the Cockpit composing a read, a test) carries no
address and is this machine's. The stdio bridge reaches the shared server over loopback and
is this machine's. `executions.start` is a command, so a remote caller cannot use it to
reach a writer. A local one could, under a tool announced as changing only this process's
memory; `executions.start` therefore declares the strongest effect of what it can start,
`repository_mutation`, and a client that asks before running a writer asks before it too.

## Consequences

- The mesh keeps working: other runtimes reach this node only through the three signed
  routes, which are unchanged.
- A remote MCP client (a phone, another machine) still reads every tool and resource, and
  can no longer announce itself on the peer board or open a mesh session through this
  node. Writing from another machine goes through an SSH tunnel that ends on loopback, the
  route `docs/MCP.md` already recommended.
- Reads are still served to the whole network, unencrypted. This decision does not make a
  non-loopback bind private; it makes it unable to change anything.
- A capability added later is refused to remote callers by default unless it reads. Nobody
  has to remember to add it to a list, and `tests/remote_callers.rs` walks the registry.
- The OpenAPI schema of `ExecutionPolicy` gains the `signed_input` field; it is omitted when
  false, so the serialisation of every other capability is unchanged.

Not decided here, and recorded as the pack's next steps rather than claimed: a token or
key-based authorization that would let an authenticated remote client write; signing the
author of a mesh event separately from the node that relays it; encrypting the transport.

## Alternatives rejected

- **Refuse every non-GET request from a remote address.** Simpler, and wrong twice: the
  mesh's signed routes are POSTs, and so is every MCP message, reads included.
- **Bind the mesh hubs to loopback.** Removes cross-machine cooperation, which is the
  hubs' purpose, and leaves every other non-loopback bind exposed.
- **A list of protected routes.** A list goes stale the day after it is written; the effect
  is already on every capability, and a new writer must be refused without anyone
  remembering it.
- **Decide once per MCP session, at `initialize`.** A session id is guessable (it carries
  the pid and a counter) and is not bound to an address; a remote host could adopt a
  session opened from loopback.
