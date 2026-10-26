<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `observatory` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `observatory` — Observatory

What the Observatory may carry to a place where no mesh runs: the mesh scenes this process has live, pseudonymized for publication, and the recording the repository carries, which the published Observatory replays and labels as a replay.

Stability: behaviorally_verified. Capabilities: 3.

## `observatory.capture` — Observatory capture

The mesh scenes of the Observatory as this process has them live, fit for publication: machines, runtimes, sessions and events by ordinal, an event by its kind, logical clock and time only. Writes nothing; committing the answer to .ai/repo/observatory/recording.json is what publishes it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_observatory_capture` |
| HTTP | `GET /api/v1/observatory/capture` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::observatory |
| tags | observatory, mesh, ui |

Input: none.

Output: `ObservatoryRecording`.

## `observatory.events` — Observatory events

The events of the mesh journal above a cursor, as the Observatory's activity scene draws them: identity, kind, node, logical clock and time, newest first, at most a page. The answer's cursor is the next request's `after`; a cursor this journal never reached is expired, and the reader takes a fresh projection. Without a cursor, the newest page.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_observatory_events` |
| HTTP | `GET /api/v1/observatory/events` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::observatory |
| tags | observatory, mesh, ui |

| input | type | required | description |
|---|---|---|---|
| `after` | integer or null | no | The cursor: only events whose logical clock is above it. Absent is the newest page. |
| `limit` | integer or null | no | At most this many (default 200, at most 1000). |

Output: `ObservatoryEvents`.

## `observatory.replay` — Observatory replay

The recording of the mesh scenes the repository carries at .ai/repo/observatory/recording.json, which the Observatory draws, labelled as a replay, where no mesh runs. Absent is present: false; a file that cannot be read is an error.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_observatory_replay` |
| HTTP | `GET /api/v1/observatory/replay` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::observatory |
| tags | observatory, mesh, ui |

Input: none.

Output: `ObservatoryReplay`.

