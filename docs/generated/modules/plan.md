<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `plan` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `plan` — The plan and its derivations

The milestone and issue model of this repository, and everything derived from it that nobody authored: the status of each record, the dependency graphs above and below the milestone boundary, the topological execution waves, the roadmap order, the milestone being executed and the one issue to take next. Status is never stored — a record says what happened to it and the status follows from that and from the state of its dependencies — so no file can contradict the graph. The four operations that write a lifecycle marker into a record stay on the command line: a capability of this registry never writes to the repository.

Stability: behaviorally_verified. Capabilities: 8.

## `plan.issues` — The issues, filtered by what the graph derived

One record per issue with its derived status, its wave, the dependencies it declares, the ones that are not DONE (plus `milestone:<id>` when the gate holds the whole outcome back), the issues that depend on it, the paths it touches and its evidence tally. Filtering by `status: READY` is the ready set and by `status: BLOCKED` the blocked set; nothing here is a separate derivation.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_issues` |
| HTTP | `GET /api/v1/plan/issues` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, issues |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | Only issues of this milestone. Default: every issue of the plan. |
| `status` | string or null | no | Only issues in this derived status — `READY` for the ready set, `BLOCKED` for the
blocked set. The vocabulary travels with every answer, so a caller never has to know
which statuses exist. |
| `wave` | integer or null | no | Only issues in this execution wave. |

Output: `PlanIssueList`.

## `plan.model` — The whole derived plan

Every milestone and issue with its derived status, wave, rank, both directions of its graph and its counts; the execution waves; both dependency graphs as edges; every validation finding; and the plan's header with the active milestone derived. The one value every other capability of this module answers out of. Derived on every call: a transition writes a lifecycle marker into a record between two calls, and a plan answered from a snapshot would send two workers to one issue.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan` |
| MCP resource | `majordomus://plan` |
| HTTP | `GET /api/v1/plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, graph, introspection |

Input: none.

Output: `Plan`.

## `plan.next` — The one issue to take now

The lowest-wave READY issue of the active milestone, highest priority first, then id. The active milestone can have nothing ready while another one does — one waiting on its own acceptance evidence, for instance — so the search widens to the whole plan rather than answering `none` and sending a worker away from work that is genuinely executable. This is what an agent asks before it starts.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_next` |
| HTTP | `GET /api/v1/plan/next` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, next |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | Restrict the answer to one milestone. Default: the whole plan, and for `next` the
active milestone with the rest of the plan as the fallback. |

Output: `PlanNextIssue`.

## `plan.record` — One milestone or issue, with everything derived about it

A milestone with its issues in full, or an issue with the issues it waits on in full. The record's own prose stays where it has always been — `majordomus://issue/<id>` returns the file — and this answers what the file cannot say about itself: what its status is, where it sits in the graph, and what is between it and being executable.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_record` |
| HTTP | `GET /api/v1/plan/record` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, issues, milestones |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The id of a milestone or an issue, as its file is named (`I0001`, `work-graph-github`). |

Output: `PlanRecord`.

## `plan.roadmap` — The milestones in derived order

The milestone graph laid out by rank, with `order` breaking ties inside a rank only, and the first unblocked unfinished milestone as `now` and the one after it as `next`. Nothing in the sequence is authored: a milestone whose prerequisites are not real cannot be nominated, which is what makes `each step is gated by the previous one being real` an invariant rather than a sentence.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_roadmap` |
| HTTP | `GET /api/v1/plan/roadmap` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, roadmap |

Input: none.

Output: `PlanRoadmap`.

## `plan.status` — Where the plan stands

Every milestone with its derived status and its issues counted by status, the milestone a worker is executing now, the next ready issue in full, and the plan's own totals. The counts are keyed by the declared vocabulary, which travels with the answer, so a status added to the engine appears here without anything being edited.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_status` |
| HTTP | `GET /api/v1/plan/status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, status |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | Restrict the answer to one milestone. Default: the whole plan, and for `next` the
active milestone with the rest of the plan as the fallback. |

Output: `PlanStatusReport`.

## `plan.validate` — What the model refuses

Every finding the derivation produced, in the order it produced them: a dependency on something that is not an issue, a cycle, an issue executing ahead of its dependencies or of its milestone's gate, an issue with no acceptance criteria, evidence missing under a completion date, a milestone whose graph contradicts itself, two issues of one wave sharing a path. A failure means the model is invalid; a warning means it is legal and worth reading.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_validate` |
| HTTP | `GET /api/v1/plan/validate` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, validation |

Input: none.

Output: `PlanValidation`.

## `plan.waves` — What may run at the same time

The topological layering of the issue graph: an issue enters a wave only once every dependency has left it, so its wave is one past the longest path to it. Sharing a wave is a necessary condition for running two issues at once, not a sufficient one — overlapping scope serialises them, and every such overlap is reported beside the waves rather than left for two workers to discover in a merge conflict.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_waves` |
| HTTP | `GET /api/v1/plan/waves` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, graph, waves |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | Restrict the answer to one milestone. Default: the whole plan, and for `next` the
active milestone with the rest of the plan as the fallback. |

Output: `PlanWaveReport`.

