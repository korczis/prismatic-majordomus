<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `continuity` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.0 -->
# Module `continuity` — Continuity

What this checkout's lifecycle is holding: the open episode, the record the next worker would resume from with the label that says how far to trust it, the newest progress note, and what is blocking acceptance. Read from the local half of the layer, which this process serves to the worker in front of it and never publishes.

Stability: behaviorally_verified. Capabilities: 1.

## `continuity.state` — What the lifecycle is holding

The open episode, the active task, the handover and checkpoint that resolve for this worktree and branch, each with its divergence label, the unresolved questions that refuse completion, and the record tallies. Selection is two-tiered and never repository-wide: a record from an unrelated worktree or branch is not offered, because a briefing that is quietly about somebody else is worse than none. Absence is reported as absence.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_continuity` |
| MCP resource | `majordomus://continuity` |
| HTTP | `GET /api/v1/continuity` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, session, handover |

Input: none.

Output: `Continuity`.

