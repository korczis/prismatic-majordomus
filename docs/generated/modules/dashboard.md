<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `dashboard` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.12.0 -->
# Module `dashboard` — Dashboards

The Dashboard Suite as a projection of the canonical state: every card is one fact read out of an existing capability's answer, carrying the capability it asked, the input, the JSON pointer its value was read from, the Cockpit page that holds the evidence and the command that acts on it. No dashboard stores, counts or judges anything of its own (ADR 0088).

Stability: behaviorally_verified. Capabilities: 1.

## `dashboard.overview` — The Overview: four questions

Is it healthy, what changed, what is broken, what needs action — each answered by cards read out of `health.report`, `release.version`, `plan.status`, `worktree.status`, `plan.next`, `continuity.state` and `peers.list` (this checkout's board), asked once each through the executor. Every card carries its source capability, input and RFC 6901 pointer, so asking the source the same question yields the same value; a source that cannot answer makes its cards `unknown`, never `ok`. Nothing here rebuilds canonical state or asks anything beyond this process and its checkout.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_dashboard_overview` |
| HTTP | `GET /api/v1/dashboard/overview` |
| CLI | `majordomus dashboard overview` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::dashboard |
| tags | dashboard, overview, introspection |

Input: none.

Output: `DashboardOverview`.

