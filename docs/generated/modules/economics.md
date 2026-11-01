<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `economics` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `economics` — Token economics

What a coding session consumes with Majordomus installed and without it, from recorded runs: matched control and treatment sessions judged by the same hidden acceptance tests, the provider's usage as it reported it, and every number labelled with its measurement class (observed, counted, derived, estimated, counterfactual). One calculator; every surface is a projection of it, and the verdict states only what the methodology's publication rule allows.

Stability: behaviorally_verified. Capabilities: 5.

## `economics.check` — Refuse unsupported economics claims

Scan the repository's hand-written prose and claim sentences for a quantity (a percentage, 'N times fewer') stated next to the economics vocabulary, and check every claim the methodology binds to a metric: a bound claim may be guaranteed only while its metric's evidence meets the binding and is current. `ok: false` names every finding.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_economics_check` |
| HTTP | `GET /api/v1/economics/check` |
| CLI | `majordomus economics check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::economics |
| tags | economics, claims, gate |

Input: none.

Output: `EconomicsCheckReport`.

## `economics.explain` — Explain one economics metric

One metric and everything it rests on: its formula, its measurement class and what that class means, the pairs and runs behind it (including the invalid ones), excluded runs, the suites and revisions, and the commands that reproduce it. A repository that declares no methodology has no metric, and any id is answered with `present: false` and the statement why, not refused.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_economics_explain` |
| HTTP | `GET /api/v1/economics/explain` |
| CLI | `majordomus economics explain` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::economics |
| tags | economics, evidence |

| input | type | required | description |
|---|---|---|---|
| `metric` | string | yes | The metric's id, as `economics.summary` lists it. |

Output: `EconomicsExplanation`.

## `economics.runs` — Recorded benchmark runs

The raw facts every economics metric is computed from: each recorded run with its task, variant, repetition, model, revision, success-gate verdicts and the totals derived from its provider usage, and the path of its record.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_economics_runs` |
| HTTP | `GET /api/v1/economics/runs` |
| CLI | `majordomus economics runs` |
| cache | process, 16 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::economics |
| tags | economics, evidence |

| input | type | required | description |
|---|---|---|---|
| `suite` | string or null | no | Only this suite. |
| `task` | string or null | no | Only this task. |
| `variant` | string or null | no | Only this variant. |

Output: `EconomicsRunList`.

## `economics.summary` — Token economics summary

Every metric of the economics benchmark with its value, measurement class, sample size, distribution and bootstrap interval; every pair of runs with its status (valid, control failed, treatment failed, incomparable, ...); results by segment; the state of each suite's evidence (current, stale, incompatible); and the one statement the publication rule allows. Context reduction and total-token reduction are separate metrics and are never merged.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_economics` |
| MCP resource | `majordomus://economics` |
| HTTP | `GET /api/v1/economics` |
| CLI | `majordomus economics summary` |
| cache | process, 8 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::economics |
| tags | economics, evidence, benchmark |

| input | type | required | description |
|---|---|---|---|
| `suite` | string or null | no | Only this suite. |
| `category` | string or null | no | Only tasks of this category. |
| `task` | string or null | no | Only this task. |
| `model` | string or null | no | Only runs that asked for this model. |

Output: `EconomicsSummary`.

## `economics.tasks` — Each task's delivery economics

One record per task, folded from the ledger of every checkout `git worktree list` reports and the archives a ledger rotation left: its issue and intent, checkout and branch, first start, last finish or refusal, wall seconds, refusal count, and the episodes that carried it with their spans. Interventions, CI reruns, spend and acceptance are present on every record as not_observed, each with the reason and the issue that will supply it. Lists every ledger read with its line and skipped-line counts and every checkout not read; an optional since/until window filters by each task's start.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_economics_tasks` |
| HTTP | `GET /api/v1/economics/tasks` |
| CLI | `majordomus economics tasks` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::economics |
| tags | economics, evidence, delivery |

| input | type | required | description |
|---|---|---|---|
| `since` | string or null | no | Answer only tasks started at or after this time (`YYYY-MM-DDTHH:MM:SSZ`). |
| `until` | string or null | no | Answer only tasks started before this time (`YYYY-MM-DDTHH:MM:SSZ`). |

Output: `TaskEconomicsList`.

