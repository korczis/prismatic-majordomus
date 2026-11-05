<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `executions` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.10 -->
# Module `executions` — Executions

Running a capability of this registry as work that can be watched: started, followed event by event over the live channel, read back afterwards, and asked to stop. One plane per checkout: `majordomus run` executes in its own process and reports the execution to the server already serving the checkout, so that server lists it beside its own; with no server it says that nothing else can see the run. In memory; an execution does not outlive the server that holds it.

Stability: behaviorally_verified. Capabilities: 8.

## `executions.cancel` — Ask an execution to stop

Set the execution's cancellation flag and say so on its stream. Cancellation is cooperative: a task looks at its flag and stops, and a capability whose policy says it is not cancellable runs to completion — which the answer says rather than pretending otherwise.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_execution_cancel` |
| HTTP | `POST /api/v1/executions/cancel` |
| CLI | `majordomus executions cancel` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The execution's id. |

Output: `CancelReport`.

## `executions.demonstrate` — Demonstrate an execution

Walk a given number of steps, reporting each one, logging a line and advancing progress, then finish — or fail at a step you name. It exists so that an operator, a probe and an end-to-end test can prove the whole path works without waiting for real work: it reads nothing, writes nothing, and its only effect is the events it produces. It looks at its cancellation flag between steps and while it waits, so cancelling it stops it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_demonstrate_execution` |
| HTTP | `GET /api/v1/executions/demonstrate` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane, diagnostic |

| input | type | required | description |
|---|---|---|---|
| `steps` | integer | no | How many steps to walk through. |
| `delay_ms` | integer | no | How long each step takes, in milliseconds. Bounded at ten seconds a step, so this
cannot be used to hold a worker. |
| `fail_at` | integer or null | no | Fail on this step instead of completing, to show what a failure looks like. |

Output: `DemonstrateReport`.

## `executions.events` — An execution's event history

The retained events of one execution, oldest first, after a sequence number. This is what a browser reads after a reload and what a client reads after a reconnect: the page carries the cursor to open the live channel with, so nothing is missed between the history and the stream.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_execution_events` |
| HTTP | `GET /api/v1/executions/events` |
| CLI | `majordomus executions events` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The execution's id. |
| `after` | integer or null | no | Only events after this sequence number: the cursor a reconnecting client holds. |
| `limit` | integer or null | no | How many, oldest first; the default and the bound are both `500`. |

Output: `EventHistory`.

## `executions.get` — One execution

The whole of what is known about one execution: its state, its input as it was stored, its steps, its progress, its diagnostics, and its output or its error. Taken under one lock, so a snapshot that says it succeeded carries what it produced.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_execution` |
| HTTP | `GET /api/v1/executions/get` |
| CLI | `majordomus executions show` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The execution's id. |

Output: `ExecutionView`.

## `executions.list` — List executions

Every execution this process remembers, newest first, narrowed by state or by capability: the ones it ran and the ones `majordomus run` in another process of the checkout reported to it, each with who started it. The counts beside them — remembered, active, queued, live channels — are what a control plane shows without asking a second question.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_executions` |
| MCP resource | `majordomus://executions` |
| HTTP | `GET /api/v1/executions` |
| CLI | `majordomus executions list` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `state` | object | no | Only executions in this state. |
| `capability` | string or null | no | Only executions of this capability. |
| `limit` | integer or null | no | How many, newest first; the default and the bound are both `200`. |

Output: `ExecutionList`.

## `executions.protocol` — The live channel's contract

Where the WebSocket is, how a subscription and a reconnect are expressed, what the server writes, every state an execution may be in with the states it may move to (the transition table the store holds every event to), and the JSON Schema of every message — derived from the Rust types that implement it, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket; this is where that contract lives.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_execution_protocol` |
| MCP resource | `majordomus://executions/protocol` |
| HTTP | `GET /api/v1/executions/protocol` |
| CLI | `majordomus executions protocol` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane, protocol |

Input: none.

Output: `ProtocolReport`.

## `executions.report` — Report an execution another process runs

What `majordomus run` posts while it runs a capability in its own process and a server already serves the checkout: the execution's description and its events after the last sequence this server took. The server holds it as an execution started by the command line, with the pid, in the same store, listing, Cockpit pages and live channel as its own; applies each event once and in order, answering where the next report starts; tells the reporter when somebody asked it to stop; and, once the reporter has been silent for 30 s, says its outcome is unknown (`unknown_outcome`, code `unavailable`) — never completed — until the reporter's own outcome reconciles it; a `started` after that is passed over, since nothing runs again before the outcome is known. Every event is held to the state machine `executions.protocol` lists, and an event of a newer schema version is refused by name. Nothing runs here. Changes this process's memory only.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| HTTP | `POST /api/v1/executions/report` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The execution's id, as the reporting process minted it. |
| `capability` | string | yes | The capability it runs, by its canonical id. |
| `title` | string | yes | That capability's title. |
| `input` | object | no | The input it was given, as the reporting process stored it: already redacted there,
and redacted again here by this registry's schema when this registry knows it. |
| `cancellable` | boolean | no | Whether the capability stops when asked. When it does, a cancel asked of this server
reaches the reporter in the answer to its next report. |
| `pid` | integer | yes | The process that runs it. |
| `events` | array | no | Its events after the last sequence this server answered `through`, in its own
sequence, oldest first; at most 500. Empty is a heartbeat. |

Output: `ReportAnswer`.

## `executions.start` — Start a capability as an execution

Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. With an idempotency_key, a repeated start is one execution: the same key with the same capability and input is answered the execution it started (deduplicated) and nothing new runs, and the same key with another input is refused as a conflict naming that execution. With an issue, the execution holds that issue's exclusive mesh claim while it runs: taken before the handler is entered (queued, claimed, running), refused with the holder named when another session holds the scope so that nothing runs, given back at the final state, and kept for an outcome that is not known; with no mesh in the serving process it runs unclaimed and says so. The capability runs through the same executor every other interface calls, so there is no second implementation of anything. On the command line (`majordomus run`) it runs in that process and is reported to the server serving the checkout, when one is, through executions.report; a run never starts a server.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_execution_start` |
| HTTP | `POST /api/v1/executions/start` |
| CLI | `majordomus run` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::executions |
| tags | executions, control-plane |

| input | type | required | description |
|---|---|---|---|
| `capability` | string | yes | The canonical id of the capability to run (`health.report`, `objects.verify`). |
| `input` | object | no | Its input, as its own input schema describes it; an empty object when it takes none. |
| `idempotency_key` | string or null | no | A key that makes a repeated start one execution (I4000): sent again with the same
capability and input, it is answered the execution it started, `deduplicated`, and
nothing new runs; sent again with another input, it is refused 409 `conflict`,
naming that execution. 1 to 128 characters of `A-Z a-z 0-9 . _ : -`, scoped to this
repository and the capability, and held for as long as the server holds the
execution it started. Without one, every start starts an execution. |
| `issue` | string or null | no | The issue the execution works for (I4004): `I4004`, `#184`. Before the handler is
entered the execution takes the issue's exclusive mesh claim — the issue record's
`scope`, or `issue/<id>` when it declares none — as the mesh session
`execution-<id>`, and moves `queued -> claimed -> running`; a claim another session
holds refuses it (`refused`, the holder named) and nothing runs. The claim is given
back at the final state and kept for an outcome that is not known. With no mesh in
the serving process it runs unclaimed and its `execution.issue_claim` event says so. |

Output: `StartedExecution`.

