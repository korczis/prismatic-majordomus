<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `ledger` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `ledger` — Ledger

The checkout's append-only record of what happened, and its one writer. An event is validated against share/events.yaml, wrapped in the envelope every line carries — the time, the event, the commit, the branch, the writer and the episode this process resolves to — and appended under the exclusive lock every writer of the file takes.

Stability: behaviorally_verified. Capabilities: 1.

## `ledger.append` — Append one event to the ledger

Validates the event against share/events.yaml — a declared name, every required field, no field the envelope owns — composes the envelope (ts, event, head, branch, by, and session when an episode resolves: the hook's key strictly, then the provider session this process runs inside, then the pointer), copies the payload's members into the line as they were written, and appends it under the ledger's exclusive lock. The shell tool's every recorded event goes through it, so the file has one writer and one lock. A refusal writes nothing.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus ledger append` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::ledger |
| tags | ledger, sessions, lifecycle, provenance |

| input | type | required | description |
|---|---|---|---|
| `event` | string | yes | The event name, as `share/events.yaml` declares it. |
| `payload` | object | no | The event's own fields, as one JSON object; the envelope is composed here. |

Output: `LedgerAppendReport`.

