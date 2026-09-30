<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `integration` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.11.0 -->
# Module `integration` — Pull-request integration

Every open pull request classified against the current master — ready, needs refresh, waiting for checks, review or a dependency, draft, needs repair, conflicting, blocked, superseded, possibly redundant, other base or unknown — each with the master and head it was decided against, its reasons, its evidence, its risk and its overlaps, ranked deterministically; and the audit trail of the executor that merges the next provably safe one, one at a time. The relation to master is decided by git with this repository's own merge drivers, because the forge cannot run the derived-file driver. Read from the last recorded forge observation: nothing here reaches the network.

Stability: experimental. Capabilities: 3.

## `integration.events` — The integration audit trail

Every action this checkout's executor recorded, oldest first: selections, stale decisions, merge attempts, merges with the master before and after, refusals, refreshes, verification failures and closures, each with its actor, the pull request, the head and the decision's reasons.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_integration_events` |
| HTTP | `GET /api/v1/pull-requests/events` |
| CLI | `majordomus prs events` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::integration |
| tags | integration, pull-requests, audit |

Input: none.

Output: `IntegrationEvents`.

## `integration.explain` — Why one pull request is where it is

One pull request's assessment — disposition, lane, reasons, next action, the master and head it was decided against, required checks, review, relation to master, dependencies, overlaps, risk with its factors, and every piece of evidence — with its rank in the queue. `found: false` with the reason when it is not open or nothing is observed.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_pull_request_explain` |
| HTTP | `GET /api/v1/pull-requests/explain` |
| CLI | `majordomus prs explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::integration |
| tags | integration, pull-requests, explain |

| input | type | required | description |
|---|---|---|---|
| `number` | integer | yes | The pull request number. |

Output: `IntegrationExplanation`.

## `integration.queue` — The integration queue

The ranked queue: the repository, the base and the master commit every assessment was decided against, when the forge was observed, the policy in force (the branch protection's required checks and reviews, the blocking labels, the merge method), every open pull request's assessment in rank order, the next merge, the pull requests that need master brought in, the tallies by disposition and lane, and the diagnostics — a stale observation first. `observed: false` with the reason when this checkout has recorded no observation.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_pull_requests` |
| HTTP | `GET /api/v1/pull-requests` |
| CLI | `majordomus prs status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::integration |
| tags | integration, pull-requests, git, queue |

Input: none.

Output: `IntegrationStatus`.

