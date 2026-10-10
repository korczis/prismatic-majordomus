<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `monitor` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.21.0 -->
# Module `monitor` — The monitor

Everything that happens around this repository, followed in one stream: the mesh journal (sessions on every linked machine, this checkout's board among them, their claims, handovers and reviews), the executor's trail (merges, refreshes, repairs, closures, failures) and GitHub (pull requests opened, merged and closed, finished workflow runs), folded so that a session re-announcing itself unchanged and a step that decided nothing are counted rather than shown, with what is addressed to the reader marked (ADR 0133).

Stability: experimental. Capabilities: 1.

## `monitor.stream` — Follow everything in one stream

A page of signals after a cursor, oldest first, or without one the newest of the last `since` seconds: each with its time, source, kind, severity, machine, session, actor, pull request, branch, issue, paths, a one-line summary and whether it is for the reader. Re-projections and undecided steps are counted as `suppressed`; a source that cannot be read is listed with its reason, never answered as empty. The page's `cursor` is the next page's `after`, and nothing on this page comes back. Reads only.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_monitor` |
| HTTP | `GET /api/v1/monitor` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::monitor |
| tags | monitor, mesh, coordination, observability |

| input | type | required | description |
|---|---|---|---|
| `after` | string or null | no | The `cursor` of the previous page; without it, the page is the newest signals of the
last `since` seconds. |
| `limit` | integer or null | no | At most this many signals (default 50, at most 500). |
| `sources` | string or null | no | A comma-separated subset of `mesh`, `executor` and `github` (default: all three). |
| `for` | string or null | no | Comma-separated names of the reader — a worker (`1b`), a role, a login — so that what
is addressed to it is marked `for_you`. An MCP caller is also its own board session. |
| `since` | integer or null | no | How far back a first page reaches, in seconds (default 3600). |

Output: `MonitorPage`.

