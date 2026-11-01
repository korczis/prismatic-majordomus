<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `board` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `board` — The delivery board

The documents the delivery board reads, derived from the plan, the roadmap, the intents, the recorded pull request queue and the release currency, with the lanes share/board.yaml declares once. Every open milestone is in a lane or the snapshot refuses, naming it.

Stability: experimental. Capabilities: 1.

## `board.snapshot` — The delivery board's documents

Every document of the delivery board by id: meta (the commit, the lanes, the issue counts, the release), milestones in roadmap order, intents, the pull request queue from the last recorded observation, and the open issues of each lane as issues-<lane>, split into issues-<lane>-2 and onwards so that no document serializes past 250000 bytes. The digest is a SHA-256 over the canonical JSON of the documents with meta.syncedAt and meta.syncedBy left out, so equal state is an equal digest. A milestone that is not DONE and is in no lane of share/board.yaml is refused by name, never filed under other. An unobserved queue is said in prs.unavailable and the diagnostics, never refused. Mesh seats, live work, CI runners and blockers are the sync step's and are not here. Reads only; nothing is cached.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_board_snapshot` |
| HTTP | `GET /api/v1/board/snapshot` |
| CLI | `majordomus board snapshot` |
| cache | — |
| benchmark | waived (repository_declaration) |
| provenance | builtin majordomus_cli::capability::builtin::board |
| tags | board, plan, delivery |

| input | type | required | description |
|---|---|---|---|
| `by` | string or null | no | Who takes the snapshot, as `meta.syncedBy` names it; `board.snapshot` when absent.
Left out of the digest. |

Output: `BoardSnapshot`.

