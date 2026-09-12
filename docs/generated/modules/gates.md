<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `gates` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `gates` — Completion gates

Completion as a state the repository decides rather than a claim a worker makes: the validation gates this repository declares in its CI model, which of them a task's own change set selects, what each one last reported, whether that verdict still describes the tree it was taken over, and therefore whether the task may be called finished. A gate that has never reported is `queued` and not `pass`, a run whose files have changed since is `stale` and refuses exactly as a failure does, and a gate the change cannot affect is `exempt` rather than unknown.

Stability: behaviorally_verified. Capabilities: 3.

## `gates.completion` — Whether this task may be called finished

The active task's own change set — from the commit it started at to the working tree, uncommitted files included — put through the CI model: which gates it selects and why, what each one last reported and over which files, which verdicts have gone stale because the tree moved underneath them, which required gates have never reported at all, and which obligations the change implies whether or not the task declared them. `finishable` is false only when a required gate is known to be failing, stale or blocked by one that is; absence of a verdict is reported as unverified rather than silently accepted.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_completion` |
| MCP resource | `majordomus://gates/completion` |
| HTTP | `GET /api/v1/gates/completion` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, completion, tasks, evidence |

| input | type | required | description |
|---|---|---|---|
| `changed` | array or null | no | The changed paths, repository-relative. Absent means the task's own change set:
everything between the commit the task started at and the working tree. |
| `base` | string or null | no | The commit to compare with, when the task's own starting commit is not wanted. |
| `on_demand` | boolean or null | no | Also plan the gates the model marks on-demand. Off by default, because a plan that
selects a gate whose runner is unavailable is a plan whose verdict never arrives. |
| `live` | boolean or null | no | Settle the obligations something can establish — a commit, a push, the trunk, the
published site, the deployed surfaces — live, at HEAD. On by default; a report that
passes `false` reads the ledger alone and says a live token is owed, never proven. |

Output: `Completion`.

## `gates.model` — The CI model

Every validation gate this repository declares — the job that runs it, the command it runs, what it proves, whether every plan selects it and whether its runner can be had on demand at all — with the path classes that decide which change selects which gate, and, derived from those classes, the pathspecs each gate's evidence is taken over.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_gates` |
| MCP resource | `majordomus://gates` |
| HTTP | `GET /api/v1/gates` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, ci, completion, introspection |

Input: none.

Output: `GateModelReport`.

## `gates.policy` — The completion policy

The one definition of done: the lifecycle stages in order, every question a task must answer before it may be called finished, and the source each answer is taken from — an obligation of share/obligations.yaml, a gate of the CI model, the structural release analysis, the change set, the task record or the continuity store. Read from share/completion.yaml; every surface that states what done means, the generated section of the provider bootstraps included, is a projection of this answer. A source the policy names and the repository lacks is reported as a problem rather than silently unanswerable.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_completion_policy` |
| MCP resource | `majordomus://gates/policy` |
| HTTP | `GET /api/v1/gates/policy` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, completion, policy, introspection |

Input: none.

Output: `CompletionPolicyReport`.

