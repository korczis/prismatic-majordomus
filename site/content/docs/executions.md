+++
title = "Executions"
description = "the execution plane: running a capability as work that can be watched — the model, the lifecycle, the typed event protocol, the live channel and its guarantees, what each interface offers, the limits and what happens at each, and what it deliberately does not do"
weight = 49
[extra]
source = "docs/EXECUTIONS.md"
+++

{% raw %}

How the Rust executable under [`apps/majordomus-cli/`](https://github.com/korczis/prismatic-majordomus/tree/@source-ref@/apps/majordomus-cli) runs a
capability as work that can be watched: the model, the lifecycle, the typed event protocol,
the live channel, what each interface offers, and what it deliberately does not do.
Behaviour as implemented and tested; where this document and the executable disagree, the
document is wrong and changes in the same commit. The decision is
[ADR 22](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md);
the rules are [`project.execution-events-are-typed`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/execution-events-are-typed.v1.md)
and [`project.executions-carry-no-secret`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/executions-carry-no-secret.v1.md);
the registry it runs on is [`CAPABILITIES.md`](@/docs/capabilities.md).

<pre class="mermaid">
flowchart TD
  declaration["ONE CANONICAL DECLARATION&lt;br/&gt;&lt;small&gt;capability! { … handler: verify }.cancellable()&lt;/small&gt;"]
  registry["CAPABILITY REGISTRY&lt;br/&gt;&lt;small&gt;the same registry MCP, HTTP, OpenAPI, the CLI and the Cockpit read&lt;/small&gt;"]
  engine["EXECUTION ENGINE&lt;br/&gt;&lt;small&gt;validate · create · queue · run on a worker · catch · complete&lt;/small&gt;"]
  store["EXECUTION STORE&lt;br/&gt;&lt;small&gt;the snapshot, the retained events, the subscribers, all bounded&lt;/small&gt;"]
  declaration --&gt; registry --&gt; engine --&gt; store
  subgraph projections["DERIVED PROJECTIONS"]
    channel["the live channel&lt;br/&gt;&lt;small&gt;WebSocket&lt;/small&gt;"]
    get["executions.get"]
    events["executions.events"]
    pages["the Cockpit's Executions pages"]
    run["majordomus run"]
    tools["MCP tools"]
  end
  store --&gt; projections
</pre>


There is no action registry, no action definition and no second implementation of anything.
An execution is a capability call a process gave an identity to — and, since
[I1501](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/project/issues/I1501.yaml), one execution plane per checkout rather than
one per process: [`majordomus run` reports to the server](#one-plane-per-checkout-majordomus-run-and-the-server)
serving the checkout, so the history a surface shows is the history a person made.

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

<pre class="mermaid">
stateDiagram-v2
  queued --&gt; running
  queued --&gt; cancelled
  running --&gt; succeeded
  running --&gt; failed
  running --&gt; cancelling
  cancelling --&gt; cancelled
  cancelling --&gt; succeeded: it finished before it noticed
  cancelling --&gt; failed
</pre>


`ExecutionState::may_move_to` is the whole contract and the store refuses anything else, so
a client that reads a final state never sees it move again. There is no `starting`: an
in-process engine claims an execution and enters its handler in the same instant, and a
state nothing can reach is a state every client would have to handle and never see.

### What is not here

An execution does not outlive the server that holds it. There is no durable queue, no
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
majordomus run health.report                       # run it here, follow it to the end, and
                                                    # report it to the server serving this checkout
majordomus run objects.verify --input '{"limit":50}' --format json
majordomus executions list                          # what the server serving this repository ran
                                                    # or was told about by a run
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
POST /api/v1/executions/report       { "id": "x-…", "pid": 4242, "events": [ … ], … }  what `run` sends
```

Over MCP the same capabilities are the tools `majordomus_execution_start`,
`majordomus_executions`, `majordomus_execution`, `majordomus_execution_events`,
`majordomus_execution_cancel` and `majordomus_execution_protocol`, with
`majordomus://executions` and `majordomus://executions/protocol` as resources. An agent
therefore discovers, starts, follows and reads an execution without parsing anything a
person was meant to look at. `executions.report` has no MCP tool and no command of its own:
it is what a run sends to a server, not something an agent or a person asks for.

`start` answers at once with the execution and the links to follow it; nothing waits for the
handler. The state in that first answer is `queued`, which is what it is: this server
answers `200` with the truth rather than `202` with a promise.

A start may carry an `idempotency_key` (I4000; `majordomus run --idempotency-key`): 1 to 128
characters of `A-Z a-z 0-9 . _ : -`, scoped to this repository and the capability started.
Sent again with the same input — a double click, a retry after an answer that never
arrived — it is answered `200` with the execution it already started, as that execution
stands now, and `"deduplicated": true`; nothing new is queued. Sent again with a different
input it is refused `409` `conflict`, and the message names the execution the key started.
Every answer of `start` carries `deduplicated`, `false` for an execution the start began.
The key is looked up and the execution recorded under one hold of the store's lock, so two
starts sent at the same instant with one key are one execution. A key is held exactly as
long as the store holds the execution it started — the bound below forgets the oldest
finished one, and the server's memory ends with the server — and since `majordomus run`
executes in its own process, a key given to it is held there. The input is matched by the
digest of its canonical JSON, so the order a client wrote the keys in does not matter; the
digest never leaves the store. Without a key every start starts an execution, as before.

## One plane per checkout: `majordomus run` and the server

`majordomus run` executes in its own process — the cost and the failure modes of a run are
the same with and without a server — and asks one question first: does a server already
serve this checkout (its lease names a URL, and the server there answers for this root)?

- **A server answers.** The run reports itself to it with `executions.report`: its
  description, the pid of the process, and its events after the last sequence the server
  took, at the latest every two seconds and soon after anything happens. The server holds
  it in the same store as its own, so `executions list`, `/cockpit/executions`, the
  execution's own page, `executions.events` and the live channel show it as they show
  anything the server ran, with `actor: {"kind": "cli", "pid": …}`. Each event is applied
  once and in order, and the answer says how far the server got (`through`), so a report
  that was lost, cut short or sent twice is put right by the next. A stop asked of the
  server (`executions.cancel`, the Cockpit's button) comes back in the next answer and the
  run cancels its own execution; cancellation stays cooperative. An output or input over
  the live channel's frame bound travels as `{"withheld": "too_large", …}` and stays in
  the run's process, which prints it.
- **No server answers.** The run runs as before and says so, once, on stderr: *no server
  is serving this repository, so this execution is visible to this process only*. Nothing
  is started for it: a run never starts a server.
- **The server stops answering mid-run.** The first failed report, or three in a row after
  one succeeded, make the run say on stderr that it is visible to this process only from
  then on, and it carries on. The server, hearing nothing for 30 seconds, fails the
  execution with `unavailable` and a message naming the process, rather than showing it
  running for ever; a run that was killed is therefore never a phantom in the list.

What this does not change: the work runs where the run is, never in the server, and the
history is still the server's memory — it ends with that server.

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
GET /events?execution=<a>&execution=<b>       several executions: each one's history, then live
```

A native WebSocket (RFC 6455), upgraded on the same socket the rest of the surface is served
on: one process, one port, one startup path, no second daemon and no Socket.IO. The
subscription is the request target, so there is nothing to send and nothing to acknowledge.

The server writes one JSON document per text frame: either an execution event, or one of
four control messages — `stream.ready` (the first frame: what was subscribed to, how much
was replayed, whether the history before it had already been dropped, the heartbeat
interval), `stream.heartbeat` (the channel is quiet and alive), `stream.lagged` (this client
fell behind and events were dropped rather than buffered; read the snapshot and the history
again from the last sequence seen) and `stream.closing`.

**Order and gaps.** A connection subscribes first and replays second, and skips in the live
stream anything the replay already wrote. The replay is read in pages of 1000 until it is
caught up, so a cursor further behind than one page still receives every retained event
after it. There is therefore no gap between the history and the stream, and nothing is
written twice. The Cockpit's client (`share/cockpit/socket.js`) checks both on its side:
an event at or below the last sequence it saw is dropped, and on a channel that follows one
execution an event beyond the next one makes it reconnect with `after=<last seen>`, so the
server replays what was missed; the same gap twice is read from the history over HTTP
instead.

**Several executions, one cursor each.** `sequence` is dense per execution, so a connection
that names several executions keeps a cursor for each, and deduplicates each execution's
events against its own. The one `after` starts every cursor: each execution named is
replayed, page by page until it is caught up, from the events after it, and an execution
named twice is followed once. `stream.ready` counts the whole replay in `replayed`, and
`truncated` is true when the history of any execution followed had already been dropped. A
client reconnecting to several executions sends no cursor and drops, per execution, what it
already holds; the Cockpit's `share/cockpit/socket.js` does exactly that. A
single-execution connection sends the sequence it last saw, as it always has.

**Heartbeat and disconnection.** The server pings a quiet connection every twenty seconds
and never reads: `tiny_http` hands over one `Read + Write` value on upgrade that cannot be
split, so a reader thread would hold the lock the writer needs. A client that goes away is
noticed when the next write fails, which the heartbeat guarantees happens; what a client
sends is ignored. A close is therefore observed within one heartbeat rather than instantly.
A ping is a control frame a browser never shows to a page, so the same interval also writes
a `stream.heartbeat` text frame; the Cockpit's client retires a channel that has said
nothing for two and a half heartbeats and reconnects from its cursor, rather than showing a
half-open connection as live.

**A value too large for a frame.** One frame is at most 256 KiB. An event whose frame would
be larger — a completed execution's `output`, a created one's `input` — is written with that
value replaced by a reference, `{"withheld": "too_large", "bytes": n, "limit": 262144,
"self": "/api/v1/executions/get?id=…"}`, and the channel stays open; the snapshot it names
carries the whole value.

**Backpressure.** A subscriber that falls behind its queue is never waited for: events are
dropped for that client alone, it is told `stream.lagged`, and the execution is not slowed
by anyone watching it. A subscriber whose reader has gone is forgotten at the next event.

**Refusals.** A plain `GET /events` answers `426` and says where the same events are
readable over HTTP. A request from another origin is refused `403` before the handshake — a
page elsewhere can open a WebSocket to a loopback server, and the same-origin policy does
not stop it. So is an upgrade from this machine addressed to a host name that is neither an
address nor `localhost`: that is a page whose domain was made to resolve here (DNS
rebinding), refused before the upgrade as every other route refuses it. An `execution`
parameter that is not an execution id is `400`, refused by shape before anything is looked
up.

## In the Cockpit

`/cockpit/executions` lists what this server has run and what `majordomus run` reported to
it, with the counts beside it;
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
| one frame of the live channel | 256 KiB | an output or input over it is written as a reference to the snapshot |
| a serial capability (a command) | one at a time | a second execution of it waits for the first |
| events in one report from a run | 500, and three quarters of the 1 MiB body bound | refused above 500; the run sends the rest in the next report |
| silence of a reported execution | 30 s (the run's heartbeat is 2 s) | the server fails it `unavailable`; the run itself is not touched |

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
| `409 conflict: the idempotency key 'k' already started x-… of … with a different input` | a start reused a key with another input | send the same input to be answered that execution, or a new key to start another |
| `no execution x-… in this process` | it never ran here, or it has been forgotten | an execution lives in the server that holds it; `executions.list` says what is remembered |
| `majordomus run: no server is serving this repository, so this execution is visible to this process only…` | `run` found no server answering for this checkout | start `majordomus serve` before the run to see it there; a run never starts one |
| `majordomus run: the server at … did not take this run's report (…)` | the server refused or did not answer; an older server has no `executions.report` | the run carries on; the reason in parentheses is the server's |
| `unavailable: process N, which ran this execution and reported it here, has not reported for … s` | the run was killed, or lost its way to the server, for 30 s | the terminal that ran it has the outcome |
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
| a run reported to the server: applied once and in order, resumed after a gap, a stop handed back, a silent reporter failed | behaviourally verified (`execution::store`) |
| a run listed by the server while in flight, the same outcome after it, a stop asked of the server reaching the run, the plain line with no server | behaviourally verified, by polling the server (`test/cases/3024_a_run_is_seen_by_the_server.sh`) |
| durability, retry, scheduling, an execution outliving its process | not implemented, and `Limits` is where a durable store would be substituted |
| a capability backed by a process, with its output as `stdout`/`stderr` log events | not implemented; the event model carries the streams for it |
| reading the client's frames, and therefore an immediate close | not implemented; the reason is in `http::ws` |

</div>

{% endraw %}
