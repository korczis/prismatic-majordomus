<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `devtask` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `devtask` — Executable development scopes

An issue and a milestone read as work to be done rather than as metadata: what a person authored, where the plan graph puts it, what happened to it locally, where its external projection stands, and whether a worker may start it — with the origin of every value on the value. Nothing here is a second project model: the records are the canonical ones, the status and the graph are `plan`'s, the branches and commits are `trace`'s, the readiness vocabulary refines three canonical statuses without replacing any, and the projection onto GitHub stays `scripts/github-sync`'s. Nothing here writes anything, which is what makes a user-authored GitHub value safe from it.

Stability: behaviorally_verified. Capabilities: 2.

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

