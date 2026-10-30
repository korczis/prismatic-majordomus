<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `accumulation` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `accumulation` — Accumulation

What this repository has piled up without anyone deciding that it should, against the budgets its policy declares: the worktrees git registers, the local branches the trunk already reaches, and the free space on the volume under them. Each quantity carries its reading, its budget, its state — ok, over, unbudgeted when the policy declares no budget, unknown when the reading could not be taken — and the command that reduces it, which re-measures every candidate before it removes one. A provider's episode events read the same answer and say what is over budget before a worker's first tool call (ADR 0150).

Stability: experimental. Capabilities: 1.

## `accumulation.status` — Is anything piling up past its budget?

Every accumulating quantity of this repository with its reading, the budget the policy declares for it under `accumulation:`, where it stands and the command that reduces it; how many are over budget and how many could not be measured; what the policy lets a crossed budget do (report, or reduce) and how often; and what the watch last removed here. Read from git and the volume on every call, with no subprocess per worktree. Reads only: nothing is removed by asking.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_accumulation` |
| MCP resource | `majordomus://accumulation` |
| HTTP | `GET /api/v1/accumulation` |
| CLI | `majordomus accumulation` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::accumulation |
| tags | accumulation, worktree, git, disk, operations |

Input: none.

Output: `AccumulationReport`.

