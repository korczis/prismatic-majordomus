<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `plan` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.10 -->
# Module `plan` — The plan and its derivations

The milestone and issue model of this repository, and everything derived from it that nobody authored: the status of each record, the dependency graphs above and below the milestone boundary, the topological execution waves, the roadmap order, the milestone being executed and the one issue to take next. Status is never stored — a record says what happened to it and the status follows from that and from the state of its dependencies — so no file can contradict the graph. This capability reads; writing a lifecycle marker into a record is `plan.transition`, which declares that it writes the repository.

Stability: behaviorally_verified. Capabilities: 13.

## `plan.allocate` — Reserve the next issue, case or ADR number

One number of the kind asked (`issue`, `case`, `adr`), reserved for one mesh session by one exclusive claim on the identifier's own key, `id:<kind>/<number>`. The candidate is one past the highest number this tree, the sibling worktrees, every ref and every reservation the mesh journal remembers hold, or the caller's `at_least` when that is higher; a number another session's claim already holds is skipped and its holder named, and a source that could not be read refuses the allocation (`source_unread`). On one runtime the claim is admitted under the admission lock, so two allocations at once answer two numbers; across linked runtimes two claims made before they sync are both admitted and the fold names one winner, whose loser allocates again (fencing is mesh-journal-converges' work). Writes this runtime's mesh journal only, never the repository; the reservation ends with its session or `mesh release`.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_allocate` |
| HTTP | `POST /api/v1/plan/allocate` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, mesh, identity, allocation |

| input | type | required | description |
|---|---|---|---|
| `kind` | IdKind | yes | `issue`, `case` or `adr`. |
| `session` | string or null | no | The mesh session the number is reserved for; the calling MCP session's own when omitted. |
| `at_least` | integer or null | no | The lowest number the caller will take: a survey it ran itself, or a block it was given. The candidate is the higher of this and one past every source. |

Output: `Allocation`.

## `plan.delivery` — How far each issue's work has travelled

For every issue (or one), the furthest delivery stage proved — none, branch, pr_open, merged, released, deployed — each with the object that proved it and when it was observed: the branch that names the issue (by id, the I1405 rule, or by its slug), the open pull request in the recorded forge observation, the newest commit in the trunk's history that names it (a merge by its pull-request title or branch, any other commit by its subject, never by its body), the first release record (`.ai/repo/releases/v*.yaml`) whose commit contains that merge, and the published `latest.json` that names that release or a later one. Beside it, the issue's declared status, so a contradiction between the stamps and the work is one comparison away. Live by default: the trunk this clone knows, every branch, the forge observation `majordomus prs refresh` recorded, the release records, and the `latest.json` of the commit the last Pages deployment published. With `at`, the merges of that commit's history and the tree's release records and nothing live — every branch, forge and deployment source is reported unread — so only merged and released can be proved and the answer is a function of the commit alone. A source that was not read is reported unread with its reason, never as none: an issue it could have moved further is not settled and names it. Inferred on every call and stored nowhere; it never writes a stamp.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_plan_delivery` |
| HTTP | `GET /api/v1/plan/delivery` |
| CLI | `majordomus plan delivery` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, delivery, git, provenance |

| input | type | required | description |
|---|---|---|---|
| `issue` | string or null | no | One issue id (`I2740`). Default: every issue of the plan. An id the plan does not
declare is not found, never answered as `none`. |
| `at` | string or null | no | Observe as of this commit (`HEAD`, a sha, a ref): its history and the tree's release
records, and nothing live — no branch, forge, deployment or clock. Default: live. |

Output: `IssueDeliveryReport`.

## `plan.drift` — Where the plan's stamps contradict the delivered work

Every contradiction between an issue's declared status and its delivery (plan.delivery), each with the objects that prove it: `merged_not_recorded` — READY, BLOCKED or ACTIVE, with a merge on the trunk naming it (pull request, merge commit, release) and no open pull request naming it; `active_without_work` — ACTIVE since before the window, with no branch, pull request, merge or live mesh claim naming it; `done_without_trace` — DONE with no merge naming it, advisory because record-only issues exist. Each finding carries its age, whether it is past the window `.ai/repo/ci/plan.yaml` declares (`drift.window`, seconds), the sources that were not read and could have dissolved it, and the command that settles it (`majordomus plan verify|done <id>`). With `gate`, judged as a gate: on a `head` every merged_not_recorded is pending; on the `trunk` one merged longer than the window ago is refused and a younger one pending; the other classes never refuse; a missing or unreadable window refuses (exit 12); refusals fail the gate (exit 10) only under `drift.enforcement: blocking`, and are reported with exit 0 under `advisory`. Observed live, or `at` a commit as plan.delivery is. Inferred on every call; it never writes a stamp.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_plan_drift` |
| HTTP | `GET /api/v1/plan/drift` |
| CLI | `majordomus plan drift` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, delivery, drift, gate |

| input | type | required | description |
|---|---|---|---|
| `issue` | string or null | no | One issue id. Default: every issue of the plan. |
| `at` | string or null | no | Observe as of this commit, as `plan.delivery` does. Default: live. |
| `gate` | object | no | Judge as a gate on a `head` or on the `trunk`. Default: report only, exit 0. |

Output: `DriftReport`.

## `plan.flight` — Where the work is now

The milestones with work in flight, the one the graph allows next, and the latest release as the releases themselves say it, read live on every call (ADR 0166, decision 4). Now is every open milestone holding an open issue with an open pull request that names it (`plan.delivery`'s `pr_open`), a live mesh claim made for it, or a declared ACTIVE status, each with those issues, why they are in flight, their declared status beside their inferred stage, and the objects that prove it (branch, pull request, merge commit, release tag, published `latest.json`, claim), linked on the forge where the forge has them. Next is the first open milestone in the plan's order that is not now and whose unfinished prerequisites are all now. The latest release is the highest stable version among the release records, the `v*` tags of this clone and the `latest.json` the last successful Pages deployment published, so a release whose record merged after its tag is not shown older than it is; `executable_tagged` says whether the answering executable's own version is tagged. `milestone` adds that milestone with every one of its issues. A source that was not read (the forge, the publication, the mesh outside a server) is named with its reason, never treated as empty.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_plan_flight` |
| HTTP | `GET /api/v1/plan/flight` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, delivery, release, mesh |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | A milestone id: the answer then carries it, with every one of its issues, as
`inspected`. An id the plan does not declare is not found. |

Output: `PlanFlight`.

## `plan.issues` — The issues, filtered by what the graph derived

One record per issue with its derived status, its wave, the dependencies it declares, the ones that are not DONE (plus `milestone:<id>` when the gate holds the whole outcome back), the issues that depend on it, the paths it touches and its evidence tally. Filtering by `status: READY` is the ready set and by `status: BLOCKED` the blocked set; nothing here is a separate derivation.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_issues` |
| HTTP | `GET /api/v1/plan/issues` |
| cache | process, 16 entries, per generation |
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
| cache | process, 16 entries, per generation |
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
| cache | process, 4 entries, per generation |
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
| cache | process, 16 entries, per generation |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, status |

| input | type | required | description |
|---|---|---|---|
| `milestone` | string or null | no | Restrict the answer to one milestone. Default: the whole plan, and for `next` the
active milestone with the rest of the plan as the fallback. |

Output: `PlanStatusReport`.

## `plan.transition` — Move one issue through the lifecycle

Record that execution of an issue began (`start`), that implementation is complete with evidence outstanding (`verify`), or that it is finished (`done`). One field of the issue's own record is stamped, the seal of its event is written beside it, and the event is appended to the ledger; a stamp without that seal moves no status. The move is refused when the model says it is illegal — an issue that is not READY cannot start, one that was never ACTIVE cannot be verified, and one that was never started, whose dependencies are unfinished or whose required evidence is absent cannot be done — and the refusal names what is in the way. The status that comes back is derived from the record afterwards, not announced by the move: a `done` whose evidence is missing leaves the issue in VERIFY and says so.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_plan_transition` |
| HTTP | `POST /api/v1/plan/transition` |
| cache | — |
| benchmark | required when plan_holds_an_issue |
| provenance | builtin majordomus_cli::capability::builtin::plan |
| tags | plan, project, issues, lifecycle |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue to move, by its id — which is also its file name. |
| `transition` | PlanTransition | yes | Which move: `start`, `verify` or `done`. |

Output: `PlanTransitionResult`.

## `plan.validate` — What the model refuses

Every finding the derivation produced, in the order it produced them: a dependency on something that is not an issue, a cycle, an issue executing ahead of its dependencies or of its milestone's gate, an issue with no acceptance criteria, evidence missing under a completion date, a lifecycle stamp its transition did not write (no seal of its plan_* event beside it), a milestone whose graph contradicts itself, two issues of one wave sharing a path. A failure means the model is invalid; a warning means it is legal and worth reading.

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

