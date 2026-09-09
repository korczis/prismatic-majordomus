---
schema: adr/v1
id: adr-0033
kind: adr
title: An execution is a watched capability call, not a second registry
status: accepted
date: 2026-09-08
tags:
  - architecture
  - capabilities
  - rust
  - http
  - cockpit
  - agents
related:
  - file:.ai/repo/adrs/0002-canonical-capability-registry.md
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - file:.ai/repo/adrs/0013-every-web-surface-is-discovered-from-its-producer-resolved-o.md
  - rule:project.interfaces-are-projections
  - rule:project.rust-canonical-declaration
  - rule:project.execution-events-are-typed
  - rule:project.executions-carry-no-secret
  - rule:project.no-new-nouns
  - file:apps/majordomus-cli/src/execution/mod.rs
  - file:apps/majordomus-cli/src/execution/engine.rs
  - file:apps/majordomus-cli/src/execution/event.rs
  - file:apps/majordomus-cli/src/http/events.rs
  - file:apps/majordomus-cli/src/http/ws.rs
  - file:apps/majordomus-cli/src/capability/builtin/executions.rs
  - file:docs/EXECUTIONS.md
provenance:
  origin: authored
---

# 22. An execution is a watched capability call, not a second registry

## Context

The registry (ADR 2) answers a call and returns. That is right for everything it held:
reads of an immutable index, answered in microseconds, over MCP, HTTP, the command line
and the Cockpit's generated runner. A person or an agent asks, and the answer comes back
inside the request.

Two things it does not cover, and both of them were arriving:

1. **Work that takes longer than a request should be held open for.** Verifying every
   object of the layer against the working tree reads nine hundred files. A future
   capability that runs a gate, drives a provider or walks thirty worktrees takes minutes.
   Holding an HTTP connection open for that is a bad contract for a browser and an
   impossible one for a page a person may reload.
2. **Work whose middle is as interesting as its end.** "It is running" and "it succeeded"
   are two facts, and a control plane that only ever reports the second is a control plane
   people watch a terminal instead of.

The obvious shape — and the one this repository has refused before, in other guises — is a
second subsystem: an *action* registry with its own definitions, its own schemas, its own
route table, its own list in the Cockpit, and a browser implementation beside the CLI
implementation. That is four copies of every capability's identity, and the copies drift on
the first edit that forgets one.

## Decision

**An execution is a capability call this process gave an identity to.** There is no action
registry, no action definition and no second implementation of anything. What was added is:

```text
capability!  ──►  CapabilityRegistry  ──►  Context::execute  ──►  the handler
                          │                       ▲                    │
                          │                       │                    │ Progress
                          └──►  ExecutionEngine ──┘                    ▼
                                       │                        ExecutionEvent
                                       ▼                               │
                                 ExecutionStore ◄──────────────────────┘
                                       │
                    ┌──────────────────┼──────────────────┐
                    ▼                  ▼                  ▼
           executions.get      executions.events     the WebSocket
```

Four decisions inside that:

**The execution plane's own API is capabilities.** `executions.start`, `executions.list`,
`executions.get`, `executions.events`, `executions.cancel` and `executions.protocol` are
`capability!` blocks like any other, so the HTTP routes, the MCP tools and resources, the
OpenAPI operations, the command line, the Cockpit's runner and the generated reference are
derived exactly as they are for everything else. Nothing about executions is registered
anywhere a second time. There is deliberately **no** capability that lists what may be run:
that is `capabilities.list`, which already answers it.

**Reporting is a field of the context, not a kind of handler.** Every `Context` carries a
`Progress` handle. Outside an execution it is silent and free to call, so a handler is
written once and reports the same way whether a benchmark, an MCP client, a terminal or a
browser called it. `health.report` was made observable by changing seven `checks.push(…)`
calls into `record(&mut checks, &ctx.progress, …)`; its route, its schema, its answer and
its callers did not change at all.

**How long a call takes is not a kind.** The three kinds stay three: a read that walks nine
hundred files is still a read. What a declaration adds is one property of its *handler* —
`.cancellable()`, meaning it looks at its cancellation flag and stops — and the rest of the
`ExecutionPolicy`, the effect and whether two may overlap, is classified from the kind
exactly as availability and visibility already are. The Cockpit's Cancel button and its
confirmation prompt are read from that descriptor rather than decided in a template, and a
handler that ignores its flag never produces a control that lies.

**Transport is downstream of the event.** `ExecutionEvent` is a typed enum with a version,
a dense sequence and a `type`/`data` envelope; its JSON Schema is derived from the Rust
type by the same `schemars` path every input and output schema takes. The store keeps and
fans out; `http::events` renders. An audit trail, a notifier or a second protocol
subscribes to the store; none of them is threaded through the engine.

**Native WebSocket, hand-written, over the server this repository already has.** The live
channel is `GET /events`, upgraded on the same `tiny_http` socket the rest of the surface
is served on: one process, one port, one startup path. The framing and the handshake are
about two hundred lines in `http::ws`, alongside the base64 and the percent-encoding this
crate already writes by hand.

## Alternatives considered

**An `ActionDefinition` registry beside the capability registry.** The thing this ADR
exists to refuse. Every action would carry an id, a title, a description, an input schema
and an output schema — all of which the capability descriptor already carries — and the two
would be reconciled by hand for ever. `project.no-new-nouns` and
`project.interfaces-are-projections` both forbid it, and neither needed to be consulted:
the second registry has no fact in it the first does not.

**Socket.IO.** A second protocol on top of WebSocket, a client library to ship into a
Cockpit whose content-security policy allows no remote origin, its own reconnection and
namespace semantics to learn, and a Rust server implementation to adopt or write — in
exchange for reconnection with backoff and a subscription model, which are forty lines of
`socket.js` and a query parameter. It was named in the request that prompted this work and
is refused on the merits: nothing this control plane needs is outside RFC 6455.

**Server-sent events.** Genuinely close, and simpler: one-way, text, reconnection and a
`Last-Event-ID` cursor in the protocol itself. Refused because the ceiling is lower — a
browser is limited to six concurrent SSE connections per origin over HTTP/1.1, which a
Cockpit with several execution tabs open reaches — and because a bidirectional channel is
what the next consumer (an agent that subscribes and then steers) will need. The cursor
idea was kept: `?after=<sequence>` is `Last-Event-ID` by another name.

**Polling `executions.get` from the browser.** Already possible, and it is what the HTTP
routes are for after a reload, a reconnect or a disagreement. As the primary mechanism it
is a fixed cost paid by every open tab whether anything is happening or not, and it cannot
show a log line at the moment it is written.

**An async runtime and an async server.** `tokio` and `axum` would give an upgrade, a
broadcast channel and a cancellation token for free. They would also replace the
synchronous server this whole crate is built on, put an executor under every handler, and
add a dependency tree larger than the crate. The synchronous store with `SyncSender`
subscribers does the same work in one file, and the bounds it enforces are visible in it.

**Reading the client's frames.** `tiny_http` hands over one `Read + Write` value on
upgrade, and it cannot be split: a reader thread blocking on it would hold the lock the
writer needs. So the subscription is the request target — which a reconnect re-sends with
its cursor — and the server ignores what the client sends. The cost is that a client's
close frame is noticed at the next write rather than immediately; the heartbeat bounds
that at twenty seconds.

**Durable executions.** Deliberately not built. An execution does not outlive the process
that accepted it, every limit is typed on `Limits`, and nothing above the store names a
`Mutex` — so a durable store is a substitution rather than a rewrite. Promising more than
that without a use case would be a queue nobody asked for.

## Consequences

Adding a browser-executable operation is adding a `capability!` block, and there is no
second step: the Cockpit discovers it, the schema-driven form renders it, the execution
plane runs it, and the live channel reports it. Adding progress to an existing capability
is calling `ctx.progress` inside its handler; nothing about its contract changes.

Two new words reach `docs/CONCEPTS.md`: *execution* and *execution event*. A fourth
capability kind was drafted and dropped — the obvious name for it, `task`, already means
"the one active unit of work in a checkout" in that same table, and a vocabulary with one
word for two things is exactly what this repository spends effort avoiding. What the kind
would have carried turned out to be one boolean about a handler, which is now a builder
method on the declaration.

`Capability` gained an `execution` field, so `docs/generated/registry.json`, the site
dataset and the OpenAPI document all carry it. They are generated, and the change was one
`majordomus generate`.

Three capabilities are benchmark-waived, with the typed reason `transient_state`: an
execution to read cannot be staged by a benchmark host, and the MCP transport measures a
separate process where no id of this one exists. That is the first real use of the waiver
mechanism, and it exposed that `capabilities validate` and `bench coverage --check` were
failing on any waiver at all — which the rule they enforce does not say. Both now decide on
what is *missing*, and report what is waived.

An execution runs on a worker thread of this process, and this process serves one
repository: nothing can run a capability against another checkout, and nothing can start
one without going through the registry. There is no shell endpoint and none is reachable
from the design — the plane runs registered typed capabilities, and a capability that ran
a command would be a `capability!` block with an allow-list, reviewed like any other.
