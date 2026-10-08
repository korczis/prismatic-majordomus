<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `integration` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.17.0 -->
# Module `integration` — Pull-request integration

Every open pull request classified against the current master — ready, needs refresh, waiting for checks, review or a dependency, draft, needs repair, conflicting, blocked, unsafe (auto-merge armed), redundant (its work is on master already), superseded (by a declared successor that landed, named in superseded_by), possibly redundant, other base or unknown — each with the master and head it was decided against, its reasons, its evidence, its risk and its overlaps, ranked deterministically; and the audit trail of the executor that merges the next provably safe one, one at a time. The relation to master is decided by git with this repository's own merge drivers, because the forge cannot run the derived-file driver. Read from the last recorded forge observation; the one exception is the dry-run proof, which observes the forge itself because the observation is part of what it proves moves nothing.

Stability: experimental. Capabilities: 5.

## `integration.cleanup` — What cleanup would do

The cleanup plan, decided offline from the recorded observation: every open pull request cleanup would close — redundant (its work is on master already) or superseded (by a declared successor that landed) — and every one it leaves for a person — possibly redundant (weak evidence) or obsolete (a person marked it, owner decision D3) — each with its disposition and the reasons that decided it, in rank order; beside it, the branches merged pull requests left on origin as `majordomus prs cleanup` last read them, with when and how long ago. A read: it closes, deletes and asks the forge for nothing — `majordomus prs cleanup` reads the forge and `--apply` closes. `observed: false` with the reason when this checkout has recorded no observation.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_pull_requests_cleanup` |
| HTTP | `GET /api/v1/pull-requests/cleanup` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::integration |
| tags | integration, pull-requests, cleanup |

Input: none.

Output: `IntegrationCleanup`.

## `integration.events` — The integration audit trail

Every action the repository's executors recorded, from any of its worktrees, oldest first: the lease taken and given back, observations, selections, stale decisions, each act's attempt before it and its outcome after — merges with the master before and after, refusals, refreshes, verification failures and closures — each with a typed action, its actor, the pull request, the head, the decision's reasons and, on an act, the evidence it was decided on. The trail is one file under the common git directory, so every worktree reads the same one.

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

One pull request's assessment — disposition, lane, reasons, every gate of the policy with whether it passed, next action, the master and head it was decided against and when the forge was observed, required checks, review, relation to master, dependencies, overlaps, risk with its factors, and every piece of evidence — with its rank in the queue. `found: false` with the reason when it is not open or nothing is observed.

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

## `integration.prove_dry_run` — Proof that a dry run moves nothing

Runs the executor's non-mutating cycle — refresh, plan, drain --dry-run and cleanup without --apply — between two snapshots of everything it could move if it were wrong: every ref origin serves, every open pull request's number, head, state and labels, the integration audit trail, the executor's lease, and every local ref outside the two namespaces the refresh mirrors. `ok` is true exactly when the snapshots are equal and refs/remotes/origin/<base> and every refs/majordomus/prs/<n> equal what origin serves. A read that reaches the network: the refresh asks the forge through the GitHub CLI and fetches the base and the pull-request heads, and like every read it rewrites the observation, relation and summary caches. It merges, closes and pushes nothing, and takes no input that could make it.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_pull_requests_prove_dry_run` |
| HTTP | `GET /api/v1/pull-requests/prove-dry-run` |
| CLI | `majordomus prs prove-dry-run` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::integration |
| tags | integration, pull-requests, proof, live |

Input: none.

Output: `DryRunProof`.

## `integration.queue` — The integration queue

The ranked queue: the repository, the base and the master commit every assessment was decided against, when the forge was observed, the policy in force (the branch protection's required checks and reviews, the label policy, the merge method: a merge commit, or none when the repository allows none), every open pull request's assessment in rank order — an actionable one with how long it has waited for the executor and how often another was chosen instead — the next merge, the pull requests that need master brought in, the starving ones, the tallies by disposition and lane, and the diagnostics, a stale observation first; beside it, who holds the base branch's integration lease and the last merge the executor recorded. `observed: false` with the reason when this checkout has recorded no observation.

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

