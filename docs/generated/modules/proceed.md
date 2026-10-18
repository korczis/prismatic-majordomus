<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `proceed` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.29.0 -->
# Module `proceed` — What a session proceeds with

The decision a session starts from (ADR 0177): the issue to take, every READY issue it passed over with the constraint that issue failed and the hold that failed it, and the terms of the order, composed from the plan, every checkout's board and every linked machine's live claims. Writes nothing.

Stability: behaviorally_verified. Capabilities: 1.

## `proceed.plan` — What to proceed with, and why

The READY issues in the plan's hand-out order (the asked milestone first, then wave, priority and id, as `plan.next`), each refused when another worker holds it: a live mesh claim names it as its issue or a hold claims its record (`claimed`), a board or mesh announcement names its id as a word of its own (`named`, quoted beside it), or a live exclusive mesh claim of another session meets its scope (`scope_held`); advisory holds over its scope are listed as `shared_with` and refuse nothing. The first issue nothing refuses is `selected`, with what it unblocks, what finishes it and the commands that take it; `reason` says why in one sentence, and `sources` whether the board and the mesh could be read. A decision that could not read the board, or a declared mesh, is `provisional` and gives no commands. The caller's own session (its mesh claims on any runtime, and its board announcements) is never held against it. Composed by the server holding this checkout's lease, which holds the board and the mesh; the command line asks it. Writes nothing.

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

