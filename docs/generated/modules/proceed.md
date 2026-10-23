<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `proceed` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->
# Module `proceed` — What a session proceeds with

The decision a session starts from (ADR 0177): the issue to take, every READY issue it passed over with the constraint that issue failed and the hold that failed it, and the terms of the order, composed from the plan, every checkout's board and every linked machine's live claims. Writes nothing.

Stability: behaviorally_verified. Capabilities: 2.

## `proceed.plan` — What to proceed with, and why

The READY issues in the plan's hand-out order (the asked milestone first, then wave, priority and id, as `plan.next`), each refused when another worker holds it: a live mesh claim names it as its issue or a hold claims its record (`claimed`), a board or mesh announcement names its id as a word of its own (`named`, quoted beside it), or a live exclusive mesh claim of another session meets its scope (`scope_held`); advisory holds over its scope are listed as `shared_with` and refuse nothing. The first issue nothing refuses is `selected`, with what it unblocks, what finishes it and the commands that take it; `reason` says why in one sentence, and `sources` whether the board and the mesh could be read. A decision that could not read the board, or a declared mesh, is `provisional` and gives no commands. The caller's own session (its mesh claims on this runtime, and its board announcements) is never held against it; a session of the same name on another runtime is another worker. Composed by the server holding this checkout's lease, which holds the board and the mesh; the command line asks it. Writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_proceed_plan` |
| HTTP | `GET /api/v1/proceed/plan` |
| CLI | `majordomus proceed plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::proceed |
| tags | proceed, plan, coordination, session |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | The milestone whose READY issues come first; the active milestone when absent. The rest of the plan follows it, as for `plan.next`. |
| `session` | string or null | no | The mesh session the decision is for, whose own claims are not held against it. Absent, the calling MCP session's board session (`board-<peer>`); over the command line, none. |
| `local` | boolean | no | Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again. |

Output: `ProceedDecision`.

## `proceed.step` — Proceed: the next step, as one work order

One step of a session that proceeds, as a work order a loop acts on: `do`, the capability to run next and its input — `devtask.implement` for the issue `proceed.plan` selects, under the idempotency key `devtask.implement` defaults to, so that a repeated order is one execution; `fan_out`, the other free READY issues declared parallel safe that share no path with it, at most four; `report`, what to announce on the board, as `peers.announce` takes it; and `next_due_at`, the next five-minute boundary after `at`, when to step again. Given an `intent`, only the issues serving its unmet criteria (`intent_realization.work`) are ordered, and an intent whose every criterion has current evidence, or that is satisfied, cancelled or superseded, answers `done` with nothing to run and nothing scheduled; without one, the whole plan is stepped and is done when no unfinished issue is left. Each item names the model `models.route` selects for a worker and the effort of the issue's profile. A provisional decision orders nothing. The same state answers the same order: `key` is the digest of everything but the moment, so a step repeated over unchanged state carries the same key at any later time. Writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_proceed_step` |
| HTTP | `GET /api/v1/proceed/step` |
| CLI | `majordomus proceed step` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::proceed |
| tags | proceed, plan, coordination, session, loop |

| input | type | required | description |
|---|---|---|---|
| `intent` | string | no | The intent the session realises, by id. The step is `done` when its realisation is met, and only issues serving its unmet criteria are ordered. Empty: the whole plan, done when no unfinished issue is left. |
| `milestone` | string or null | no | The milestone whose READY issues come first, as for `proceed.plan`; the active one when absent. |
| `session` | string or null | no | The mesh session the step is for, whose own claims are not held against it, as for `proceed.plan`. |
| `at` | string or null | no | The moment the step is taken at, RFC 3339 in UTC to the second (`2026-10-23T10:00:00Z`); the clock when absent. Only `next_due_at` depends on it. |
| `local` | boolean | no | Compose in the process that answers and never ask the checkout's leaseholder, as for `proceed.plan`. |

Output: `WorkOrder`.

