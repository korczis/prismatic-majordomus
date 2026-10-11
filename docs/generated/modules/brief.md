<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `brief` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.21.0 -->
# Module `brief` — The session brief

What surrounds one working session, in one line and in a few: its episode and task, the peers of every board, the mesh, the integration queue and the derive queue, each section asked of its owner once and saying whether it could be read. The answer a session reports on itself from without being asked, and the one a provider mod shows as its status line (ADR 0136). Writes nothing.

Stability: behaviorally_verified. Capabilities: 1.

## `brief.session` — The session brief

One line and a few: this session's episode and task (the one the given provider session owns, else the pointer's), the peers of every board with the boards that could not be read named, the mesh's links and what its journal addresses to this runtime, the integration queue as `majordomus prs brief` says it, and the machine's derive lock. Composed by the server holding this checkout's lease, which holds the board and the mesh: the command line asks it and composes the brief itself only when none answers, saying so; a superseded server says its own board is unreachable by the repository's gather. Carries the reporting interval the policy declares (`session.brief_interval_minutes`) and the keep-awake text a session is told at its start. Writes nothing.

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
| `session` | string or null | no | The provider's own session identity (Claude Code's session id): the brief names the open episode that session owns. Absent, the episode the pointer resolves to. |
| `checkouts` | Checkouts | no | Which checkouts the peers section covers: `repository` (every board, the default) or `this`. |
| `local` | boolean | no | Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again. |

Output: `SessionBrief`.

