<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `executions` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `executions` — Executions

Running a capability of this registry as work that can be watched: started, followed event by event over the live channel, read back afterwards, and asked to stop. In memory; an execution does not outlive the process that accepted it.

Stability: behaviorally_verified. Capabilities: 7.

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

Every execution this process remembers, newest first, narrowed by state or by capability. The counts beside them — remembered, active, queued, live channels — are what a control plane shows without asking a second question.

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

Where the WebSocket is, how a subscription and a reconnect are expressed, what the server writes, and the JSON Schema of every message — derived from the Rust types that implement it, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket; this is where that contract lives.

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

## `executions.start` — Start a capability as an execution

Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. The capability runs through the same executor every other interface calls, so there is no second implementation of anything.

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

Output: `ExecutionView`.

