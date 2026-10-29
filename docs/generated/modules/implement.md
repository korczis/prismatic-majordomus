<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `implement` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `implement` — Implement

One issue implemented by one execution: readiness, lease, worktree, worker.

Stability: experimental. Capabilities: 1.

## `implement.issue` — Implement an issue

Judge the issue's readiness, claim its scope as the execution's lease, create its worktree and hand the work to the first available worker adapter, each a step of one execution (ADR 0225). Started only as an execution, through executions.start.

| | |
|---|---|
| kind | command |
| stability | experimental |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::implement |
| tags | issue, executions, worker |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue to implement, as the canonical model spells it (`I3983`). |
| `deadline_minutes` | integer or null | no | How many minutes the worker may run before its whole process group is stopped. |

Output: `ImplementReport`.

