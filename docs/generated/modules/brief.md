<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `brief` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `brief` — The session brief

What surrounds one working session, in one line and in a few: its episode and task, its own work (claims, branches, worktrees, pull requests, trains, runs, background processes), the peers of every board, the mesh, the integration queue, the CI pool, the machine, the release and the derive queue, each section asked of its owner once under its own budget and saying what it observed, from where, how old it is against its bound, a verdict word and the command that refreshes it; the conditions that need attention, derived from those sections; and the report the session owes, composed from the report contract share/brief/report.yaml (ADR 0136). Writes nothing and builds nothing.

Stability: behaviorally_verified. Capabilities: 1.

## `brief.session` — The session brief

One line and a few, in the order session, work, peers, mesh, integration, ci, machine, release, derive, led by the attention entries the sections measured. The session is the caller's (the provider session given, else the calling board peer's, else the checkout's pointer, said as such); work is this session's own claims, branches and their canonical worktrees, pull requests, trains, latest runs with their jobs and background processes; integration says how old the queue observation is and whether origin/master moved past it; ci and the forge parts of work are read by the leaseholder alone through the monitor's shared cache; machine is free disk, swap and builders against machine.heavy_work. Every section carries its observation (source, age, bound, verdict, remedy, cost) and runs under its own budget, a section past it reading unread. Composed by the server holding this checkout's lease: the command line asks it and composes the brief itself only when none answers, saying so. Carries the reporting interval (`session.brief_interval_minutes`), per-section fingerprints of its clock-free facts, and the three texts composed from the report contract share/brief/report.yaml: keep_awake, report.prompt and report.reminder. Writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_brief` |
| HTTP | `GET /api/v1/brief` |
| CLI | `majordomus brief` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::brief |
| tags | session, peers, coordination, brief |

| input | type | required | description |
|---|---|---|---|
| `session` | string or null | no | The provider's own session identity (Claude Code's session id): the brief names the open episode that session owns and the work it holds. Absent, the calling board peer's, then the episode the pointer resolves to, which the answer says is not necessarily this session. |
| `checkouts` | Checkouts | no | Which checkouts the peers section covers: `repository` (every board, the default) or `this`. |
| `local` | boolean | no | Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again. |
| `since` | string or null | no | The previous report's instant, RFC 3339: the work section then says what was delivered since. |
| `background` | array | no | The provider's background tasks, when its engine exposes them; absent, the brief says the provider's background is not exposed. |

Output: `SessionBrief`.

