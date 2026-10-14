<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `observatory` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.24.1 -->
# Module `observatory` — Observatory

What the Observatory may carry to a place where no mesh runs: the mesh scenes this process has live, pseudonymized for publication, and the recording the repository carries, which the published Observatory replays and labels as a replay.

Stability: behaviorally_verified. Capabilities: 2.

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

