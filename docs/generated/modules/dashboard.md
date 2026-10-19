<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `dashboard` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.30.2 -->
# Module `dashboard` — Dashboards

The Dashboard Suite as a projection of the canonical state: every card is one fact read out of an existing capability's answer, carrying the capability it asked, the input, the JSON pointer its value was read from, the Cockpit page that holds the evidence and the command that acts on it. No dashboard stores, counts or judges anything of its own (ADR 0088).

Stability: behaviorally_verified. Capabilities: 2.

## `dashboard.cluster` — The cluster: machines, sessions, claims and plan progress

Who works where and how far each milestone is, as one reading and one rendering: every machine of the mesh with its runtimes, active sessions, claims held and what its sessions say they are doing (`mesh.peers`), the progress of every open milestone (`plan.status`), and the one-line brief with the integration queue, the derive lock and the mesh's links (`brief.session`). It holds no data of its own and names, in `notes`, each source that did not answer. `lines` is the page as console text, cut to the width asked, and is what the command line prints and the /ui panel draws. Composed by the server holding this checkout's lease; the command line asks it. Writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_dashboard_cluster` |
| HTTP | `GET /api/v1/dashboard/cluster` |
| CLI | `majordomus dashboard cluster` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::dashboard |
| tags | dashboard, cluster, peers, plan, introspection |

| input | type | required | description |
|---|---|---|---|
| `local` | boolean | no | Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again. |
| `width` | integer or null | no | The console width `lines` is cut to, in characters (40 to 200; 78 when absent). |

Output: `DashboardCluster`.

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

