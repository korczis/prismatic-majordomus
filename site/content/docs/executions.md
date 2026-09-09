+++
title = "Executions"
description = "the execution plane: running a capability as work that can be watched — the model, the lifecycle, the typed event protocol, the live channel and its guarantees, what each interface offers, the limits and what happens at each, and what it deliberately does not do"
weight = 36
[extra]
source = "docs/EXECUTIONS.md"
+++

{% raw %}

How the Rust executable under [`apps/majordomus-cli/`](../apps/majordomus-cli/) runs a
capability as work that can be watched: the model, the lifecycle, the typed event protocol,
the live channel, what each interface offers, and what it deliberately does not do.
Behaviour as implemented and tested; where this document and the executable disagree, the
document is wrong and changes in the same commit. The decision is
[ADR 22](../.ai/repo/adrs/0031-an-execution-is-a-watched-capability-call-not-a-second-registry.md);
the rules are [`project.execution-events-are-typed`](../.ai/repo/rules/project/execution-events-are-typed.v1.md)
and [`project.executions-carry-no-secret`](../.ai/repo/rules/project/executions-carry-no-secret.v1.md);
the registry it runs on is [`CAPABILITIES.md`](@/docs/capabilities.md).

```text
ONE CANONICAL DECLARATION   capability! { … handler: verify }.cancellable()
        ↓
CAPABILITY REGISTRY         the same registry MCP, HTTP, OpenAPI, the CLI and the Cockpit read
        ↓
EXECUTION ENGINE            validate · create · queue · run on a worker · catch · complete
        ↓
EXECUTION STORE             the snapshot, the retained events, the subscribers — all bounded
        ↓
DERIVED PROJECTIONS         the live channel (WebSocket) · executions.get · executions.events
                            · the Cockpit's Executions pages · `majordomus run` · MCP tools
```

There is no action registry, no action definition and no second implementation of anything.
An execution is a capability call this process gave an identity to.

## The model

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| **Execution** | one call of one capability, with an id, a lifecycle, an input as it was stored, an output or an error, its steps, its progress, its diagnostics, who asked for it and which repository it ran against |
| **Execution event** | one typed fact about an execution, in an envelope with a protocol version and a dense sequence number |
| **Execution policy** | what running a capability means: its effect and whether two may overlap, classified from the kind; and whether cancelling it achieves anything, which only its handler can say |
| **Progress** | the handle every context carries; silent outside an execution, so a handler is written once for every caller |

</div>


### The lifecycle

```text
  queued ──► running ──┬──► succeeded
     │                 ├──► failed
     │                 └──► cancelling ──┬──► cancelled
     │                                   ├──► succeeded   (it finished before it noticed)
     └──► cancelled                      └──► failed
```

`ExecutionState::may_move_to` is the whole contract and the store refuses anything else, so
a client that reads a final state never sees it move again. There is no `starting`: an
in-process engine claims an execution and enters its handler in the same instant, and a
state nothing can reach is a state every client would have to handle and never see.

### What is not here

An execution does not outlive the process that accepted it. There is no durable queue, no
retry, no schedule and no execution that survives a restart. Every limit is typed on
`execution::store::Limits` — how many executions are remembered, how many events each, how
long a log line may be, how many run at once, how far a subscriber may fall behind — and
each has a defined behaviour at the bound rather than an unwritten one. Nothing above the
store names a `Mutex`, so a durable store is a substitution rather than a rewrite.

## Running one

Any executable capability of the registry can be started as an execution. There is no
second list of what may be run: `capabilities.list` already answers it, and the descriptor's
own `execution` policy says what running it means.

```bash
majordomus run health.report                       # start it here and follow it to the end
majordomus run objects.verify --input '{"limit":50}' --format json
majordomus executions list                          # what the server serving this repository ran
majordomus executions show   <id>
majordomus executions events <id> --after 42
majordomus executions cancel <id>
majordomus executions protocol                      # the live channel's contract
```

```http
POST /api/v1/executions/start        { "capability": "objects.verify", "input": { … } }
GET  /api/v1/executions              ?state=running&capability=objects.verify&limit=50
GET  /api/v1/executions/get          ?id=x-…
GET  /api/v1/executions/events       ?id=x-…&after=42&limit=500
POST /api/v1/executions/cancel       { "id": "x-…" }
GET  /api/v1/executions/protocol
GET  /api/v1/executions/demonstrate  ?steps=3&delay_ms=200
GET  /api/v1/objects/verify          ?kind=rule&limit=50
```

Over MCP the same capabilities are the tools `majordomus_execution_start`,
`majordomus_executions`, `majordomus_execution`, `majordomus_execution_events`,
`majordomus_execution_cancel` and `majordomus_execution_protocol`, with
`majordomus://executions` and `majordomus://executions/protocol` as resources. An agent
therefore discovers, starts, follows and reads an execution without parsing anything a
person was meant to look at.

`start` answers at once with the execution and the links to follow it; nothing waits for the
handler. The state in that first answer is `queued`, which is what it is: this server
answers `200` with the truth rather than `202` with a promise.

## The event protocol

One envelope, a closed set of payloads, a version. `type` and `data` are the only two
members that vary, so a client switches on one string.

```json
{
  "schema_version": "1",
  "event_id": "x-20260908T010203Z-0a1b2c3d#17",
  "execution_id": "x-20260908T010203Z-0a1b2c3d",
  "sequence": 17,
  "timestamp": "2026-09-08T01:02:07Z",
  "type": "execution.progress",
  "data": { "current": 17, "total": 42, "message": "137 file(s) read" }
}
```

<div class="overflow-x-auto" tabindex="0">

| type | when |
|---|---|
| `execution.created` | it exists; carries the capability and the input as it was stored |
| `execution.queued` | it is waiting for a worker, and how many are ahead of it |
| `execution.started` | the handler was entered |
| `execution.progress` | how far along, with a total when the handler knows one |
| `execution.step.started`, `execution.step.completed` | a named phase was entered, and how it went |
| `execution.log` | a line of output, from the handler or from a process it ran |
| `execution.diagnostic` | a structured finding, distinct from the outcome |
| `execution.cancelling`, `execution.cancelled` | it was asked to stop; it stopped |
| `execution.completed` | it finished, with what the handler returned |
| `execution.failed` | it finished, with a typed error and a correlation id |

</div>


The output travels on `execution.completed` rather than on an event of its own, so no client
can read "succeeded" and find no result. `sequence` is dense and starts at 1 within an
execution: a client that has seen `n` knows it missed something when the next it reads is
not `n + 1`, and asks for the gap by cursor rather than reloading the world.

The schema is derived from the Rust enum by `schemars`, exactly as every capability's input
and output schemas are, and is served — with the whole contract — by
`executions.protocol`. Nothing is written down twice.

**A client must ignore a `type` it does not know.** That is what lets a variant be added
without a version change; `schema_version` moves only for a change a version-1 client could
not ignore safely.

## The live channel

```text
GET /events                                  every execution of this process, live
GET /events?execution=<id>                   one execution: its retained history, then live
GET /events?execution=<id>&after=<sequence>   the same, from a cursor — what a reconnect sends
```

A native WebSocket (RFC 6455), upgraded on the same socket the rest of the surface is served
on: one process, one port, one startup path, no second daemon and no Socket.IO. The
subscription is the request target, so there is nothing to send and nothing to acknowledge.

The server writes one JSON document per text frame: either an execution event, or one of
three control messages — `stream.ready` (the first frame: what was subscribed to, how much
was replayed, whether the history before it had already been dropped, the heartbeat
interval), `stream.lagged` (this client fell behind and events were dropped rather than
buffered; read the snapshot and the history again from the last sequence seen) and
`stream.closing`.

**Order and gaps.** A connection subscribes first and replays second, and skips in the live
stream anything the replay already wrote. There is therefore no gap between the history and
the stream, and nothing is written twice.

**Heartbeat and disconnection.** The server pings a quiet connection every twenty seconds
and never reads: `tiny_http` hands over one `Read + Write` value on upgrade that cannot be
split, so a reader thread would hold the lock the writer needs. A client that goes away is
noticed when the next write fails, which the heartbeat guarantees happens; what a client
sends is ignored. A close is therefore observed within one heartbeat rather than instantly.

**Backpressure.** A subscriber that falls behind its queue is never waited for: events are
dropped for that client alone, it is told `stream.lagged`, and the execution is not slowed
by anyone watching it. A subscriber whose reader has gone is forgotten at the next event.

**Refusals.** A plain `GET /events` answers `426` and says where the same events are
readable over HTTP. A request from another origin is refused `403` before the handshake — a
page elsewhere can open a WebSocket to a loopback server, and the same-origin policy does
not stop it. An `execution` parameter that is not an execution id is `400`, refused by
shape before anything is looked up.

## In the Cockpit

`/cockpit/executions` lists what this process has run, with the counts beside it;
`/cockpit/executions/<id>` is one execution, at a stable URL a person can reload, share
locally or open several of in tabs.

Both pages are complete HTML before any script runs — the state, the steps, the progress,
the diagnostics, the log, the output and the input are what the server rendered — and the
scripts add that they stop being a photograph. `socket.js` is the only place in the browser
that opens a socket: it reconnects with exponential backoff, a cap and jitter, resubscribes
from the sequence the page was rendered at, and shows the connection as a word and a shape
rather than as a colour alone. Every URL it uses was rendered into the markup by the server;
no path is written down in the browser.

On a capability's own page, **Run as an execution** sits beside the direct runner. Both use
the same generated form, and the button asks for confirmation when — and only when — the
descriptor's effect says something changes. What a page does with an event it does not
recognise is show it, not drop it.

A log line reaches the page through `textContent`, never `innerHTML`, and the server has
already removed control characters and terminal escape sequences from it. The log scrolls
inside its own box and is bounded in the browser as well as on the server, so a long path
in a line wraps rather than widening the page.

## Adding a capability that can be watched

Nothing extra. Write the `capability!` block as [`CAPABILITIES.md`](@/docs/capabilities.md)
describes, and it can be started as an execution the moment it exists. Two optional things
make it a better one:

1. **Report.** `ctx.progress` carries `step`, `step_done`, `progress`, `log`, `stream` and
   `diagnostic`. All are free to call outside an execution, so a handler reports
   unconditionally and is written once for every caller.
2. **Declare it cancellable** — `.cancellable()` after the `capability!` block — when the
   handler checks `ctx.progress.cancelled()` and stops. The Cockpit then offers a Cancel
   button that does something, `executions.cancel` says so in its answer, and a handler that
   ignores its flag never produces a control that lies.

```rust
fn verify(ctx: &Context, input: VerifyInput) -> Result<Report, CapabilityError> {
    let p = &ctx.progress;
    p.step("read", "Reading the layer");
    for (n, file) in files.iter().enumerate() {
        if p.cancelled() {
            return Err(p.cancellation());
        }
        p.progress(n as u64 + 1, Some(total), format!("read {}", file.path));
    }
    p.step_done("read", true, None);
    Ok(report)
}
```

**A cached capability run as an execution runs.** The cache makes one answer
indistinguishable from another, which is right for an answer and wrong for a record of
work: an execution answered from a cache would report no steps and claim to have succeeded
at something that did not happen. `Context::execute_observed` is the path the engine takes,
and it steps over the cache in both directions.

## Safety

**Only registered capabilities run.** The plane takes a capability id and an input; there is
no endpoint that takes a command, and the input a client may send is two fields whose schema
is in the OpenAPI document. A capability that one day runs a process would be a
`capability!` block with an allow-list of its own, reviewed like any other.

**Repository isolation.** A process serves the repository it read at start-up, and every
execution is stamped with it. Nothing can run a capability against another checkout, and
nothing a client sends names a directory.

**Secrets.** A value the input schema marks sensitive — `format: "password"`, `writeOnly`,
or `x-majordomus-sensitive` — is replaced once, in the engine, before the input is stored,
so no snapshot, event, history, page, listing or generated document can carry it. A field's
name never decides this: `password_policy` may be a document, and a name-matching redactor
destroys it while missing the next field spelled differently.

**Failures.** A panicking handler is caught: the execution fails with `internal`, the
correlation id and a suggestion, and the panic itself goes to this process's log where the
operator can read it. No backtrace and no path inside this crate reaches a client.

**Origin.** The live channel and every state-changing route refuse a request from another
origin. The Cockpit's content-security policy allows connections to this origin only.

## Limits, and what happens at each

<div class="overflow-x-auto" tabindex="0">

| limit | default | at the bound |
|---|---|---|
| executions remembered | 200 | the oldest **finished** one is forgotten; a running one never is |
| events retained per execution | 2000 | the oldest are dropped, and the snapshot and every history page say `truncated` |
| log line | 2000 characters | cut, with a marker |
| executions running at once | 4 | further ones wait `queued`, and `execution.queued` says how many are ahead |
| a subscriber's queue | 512 | events are dropped for that client, which is told `stream.lagged` |
| live channels | 64 | a further connection is refused `503` with the reason |
| a serial capability (a command) | one at a time | a second execution of it waits for the first |

</div>


The defaults are for a development server a person is watching, and `executions.protocol`
answers the ones this process is running with.

## When something fails

<div class="overflow-x-auto" tabindex="0">

| message | meaning | remedy |
|---|---|---|
| `no capability 'x'; the executable ones are listed by capabilities.list` | the id is not in the registry | check `majordomus capabilities list` |
| `not executable: 'x' is a resource: it is read, never executed` | a declarative object was named | read it with `objects.get` |
| `invalid input: /steps: "many" is not of type "integer"` | the input does not satisfy the capability's own schema | the schema is in the OpenAPI document and on the capability's Cockpit page |
| `no execution x-… in this process` | it never ran here, or it has been forgotten | an execution lives in the process that accepted it; `executions.list` says what is remembered |
| `426 … /events is a WebSocket` | it was opened with a plain `GET` | open it with a WebSocket client, or read `executions.events` |
| `403 … a WebSocket from origin '…' is refused` | a page on another origin tried to follow this server | open the Cockpit from this server's own address |
| `stream.lagged` | this client could not keep up | read the snapshot and the history again from the last sequence seen |
| an execution that reports no steps | the capability does not report any | that is a fact about the capability, not about the plane; `executions.demonstrate` shows what one that does looks like |

</div>


## Stability

<div class="overflow-x-auto" tabindex="0">

| | status |
|---|---|
| the state machine, its transitions and the refusal of an invalid one | behaviourally verified (`execution::model`, `execution::store`) |
| the store's bounds and the behaviour at each: eviction, truncation, lag, a gone subscriber | behaviourally verified (`execution::store`) |
| redaction from the schema, through references, arrays and maps | behaviourally verified (`execution::redact`) |
| the handshake, the framing and SHA-1 against the published vectors | behaviourally verified (`http::ws`) |
| start, follow, reload, reconnect with a cursor, cancel, fail — over a real socket with a real WebSocket client | behaviourally verified (`tests/executions.rs`) |
| an existing capability observable with no second implementation | behaviourally verified (`tests/executions.rs`) |
| an execution never answered from the cache | behaviourally verified (`tests/executions.rs`) |
| the whole path from outside the executable | behaviourally verified (`test/cases/100_execution_plane.sh`) |
| durability, retry, scheduling, an execution outliving its process | not implemented, and `Limits` is where a durable store would be substituted |
| a capability backed by a process, with its output as `stdout`/`stderr` log events | not implemented; the event model carries the streams for it |
| reading the client's frames, and therefore an immediate close | not implemented; the reason is in `http::ws` |

</div>

{% endraw %}
