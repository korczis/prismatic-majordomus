<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `devtask` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.8 -->
# Module `devtask` — Executable development scopes

An issue and a milestone read as work to be done rather than as metadata: what a person authored, where the plan graph puts it, what happened to it locally, where its external projection stands, and whether a worker may start it — with the origin of every value on the value. Nothing here is a second project model: the records are the canonical ones, the status and the graph are `plan`'s, the branches and commits are `trace`'s, the readiness vocabulary refines three canonical statuses without replacing any, and the projection onto GitHub stays `scripts/github-sync`'s. The two readers write nothing, which is what makes a user-authored GitHub value safe from them; `devtask.implement` writes only through the capabilities it composes (`worktree.create`, `executions.start`), and `devtask.run` is the run it starts.

Stability: behaviorally_verified. Capabilities: 4.

## `devtask.implement` — Implement an issue: readiness, preflight, claim, worktree and one run

Take one issue as one request (I4003): a key that already started an execution for it answers that execution (`deduplicated`) and creates nothing; otherwise the issue's readiness as `devtask.issue` derives it (refused `not_ready` with the reason and the blockers), `intents.preflight` (refused `invalid_intent` with its refusal), whether the issue's exclusive mesh claim would be admitted now (refused `claim_conflict` naming each holder), the worker — the attached MCP peer that asked; with none the answer is `pending` and nothing is created — then `worktree.create` on `feature/<id>-<slug>` and `executions.start` of `devtask.run` for the issue under the key (default `implement-<issue>`), whose execution takes the issue's claim before it runs (I4004). A refusal creates nothing. The answer is the standing, every step with what it found, the branch, the worktree, the worker and the execution with its links.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_devtask_implement` |
| HTTP | `POST /api/v1/devtask/implement` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::devtask |
| tags | devtask, issue, executions, worktree, mesh, intent |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue to implement, as the canonical model spells it (`I4003`). |
| `idempotency_key` | string or null | no | What makes a repeated request one execution (I4000): sent again, it is answered the
execution it started, as that execution stands now, and nothing new is created.
Default: `implement-<issue>`, so that a double click is one implementation; send a
new key to start the issue again after its execution ended. |
| `deadline_minutes` | integer or null | no | How many minutes the worker's run may last before it fails on its deadline; 60 when
absent, a day at most. |

Output: `ImplementAnswer`.

## `devtask.issue` — One issue as an executable development task

Identity, title, intent, status, milestone, acceptance criteria, dependencies, blockers, the commits its own evidence names and the commits git found, its branches, the sessions that worked on it, its readiness and everything wrong with its records — each field carrying whether a person authored it (`explicit`), a machine worked it out by a rule that cannot be wrong (`derived`), a machine worked it out by a rule that can (`inferred`, with the rule stated), or it is not available at all (`unknown`, with the reason). A key the record does not carry is `unknown`, never an empty string: `objective: ""` and a record with no objective are the same string and different facts. An id the model does not declare is answered, not refused. `git: false` answers from the records alone, which is what a comparison across machines wants.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_devtask` |
| HTTP | `GET /api/v1/devtask/issue` |
| CLI | `majordomus devtask issue` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::devtask |
| tags | devtask, plan, issue, provenance, readiness, git |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue id, as the canonical model spells it (`I0901`). An id the model does not
declare is answered with `declared: false` and every canonical field `unknown`,
never refused and never answered with empty strings that read as authored. |
| `git` | boolean or null | no | Consult git for the branches and commits that realised it. Default: yes. Set it to
false for a deterministic answer that depends on the records alone — which is what a
generated document or a comparison across machines wants. |

Output: `DevTask`.

## `devtask.milestone` — One milestone as an executable dependency graph

Every issue of the milestone as a node with its readiness, its wave and what waits on it; every dependency edge with at least one end inside, the crossing ones marked; the issues partitioned into ready, blocked, waiting, active, review, completion-blocked, complete and cancelled; the critical blockers ordered by how much unfinished work each holds back; the startable work partitioned into subsets that may genuinely run at the same time — same wave, each parallel-safe, no two sharing a scope path — with each serialisation naming the path that caused it; every dependency cycle as its strongly connected component; and every finding about the milestone or its issues. A pure function of the canonical records: no git, no clock, no network, every list in canonical order, so two runs on two machines produce the same bytes. A work surface reading this derives nothing itself.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_devtask_milestone` |
| HTTP | `GET /api/v1/devtask/milestone` |
| CLI | `majordomus devtask milestone` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::devtask |
| tags | devtask, plan, milestone, graph, readiness, provenance |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string | yes | The milestone id, as the canonical model spells it. An id the model does not declare
is answered with an empty graph and every field `unknown`, not refused. |

Output: `MilestoneGraph`.

## `devtask.run` — A worker's run on an issue's worktree

The execution `devtask.implement` starts, and nothing else: the run handed to the attached peer that asked, in the issue's worktree, under the issue's mesh claim. It lasts while that peer stays attached — the same position id, attached at the same instant — and ends `succeeded` when it detaches, `cancelled` when asked to stop, and `failed` on its deadline; refused when the worker is not attached when it starts. What the worker did is in the worktree and is verified by what verifies a branch, never by this. Reachable only through `executions.start`: it declares no route, tool or command of its own.

| | |
|---|---|
| kind | command |
| stability | experimental |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::devtask |
| tags | devtask, issue, executions, worker |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue. |
| `branch` | string | yes | The branch the work is on. |
| `worktree` | string | yes | The worktree the worker works in. |
| `worker` | string | yes | The worker: the board's id of the attached peer the run was handed to (`p3`). |
| `worker_attached_at` | string | yes | When that peer attached, RFC 3339: with the id, which peer it is, since a position id
is reused by the next client to connect after it. |
| `deadline_minutes` | integer | yes | The run's deadline, in minutes. |

Output: `RunReport`.

