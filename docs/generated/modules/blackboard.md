<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `blackboard` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.29.0 -->
# Module `blackboard` — The peer blackboard

Who works on what in this repository, as one projection of the records that already know it: every checkout's board, the mesh journal's sessions, claims, reviews and handovers on every linked machine, and the monitor's newest signals, joined once so that no surface joins them again (ADR 0176). A board peer and a mesh session are one worker only when an explicit identifier joins them; a source that could not be read makes the answer incomplete and says why.

Stability: experimental. Capabilities: 1.

## `blackboard.snapshot` — Who works on what, now

Every worker of this repository — attached to a checkout's board or in an active mesh session on any linked machine — with the client it is, what it announced, its checkout, branch, machine and claims (advisory or exclusive); the claims that meet (potential, or confirmed when both are exclusive or the mesh marked one conflicted); the open reviews and untaken handovers; typed relations between workers, verified only when an explicit identifier joins them; the newest signals with the monitor's cursor a follower resumes from; each source with how it was read, and whether the answer is complete. Reads only.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_blackboard` |
| HTTP | `GET /api/v1/blackboard` |
| CLI | `majordomus blackboard` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::blackboard |
| tags | peers, mesh, monitor, coordination |

| input | type | required | description |
|---|---|---|---|
| `checkouts` | Checkouts | no | Which checkouts' boards it covers: `repository` (every one, the default) or `this`. |
| `for` | string or null | no | Comma-separated names of the reader — a worker (`1b`), a role, a login — so that the signals addressed to it are marked `for_you`. An MCP caller is also its own board session. |
| `signals` | integer or null | no | How many of the newest signals of the last hour it carries (default 30, at most 200); 0 reads no signals at all, for a follower that reads them by its own cursor. |
| `sources` | string or null | no | A comma-separated subset of the monitor's sources, `mesh`, `executor` and `github` (default: `mesh,executor`). |

Output: `Blackboard`.

