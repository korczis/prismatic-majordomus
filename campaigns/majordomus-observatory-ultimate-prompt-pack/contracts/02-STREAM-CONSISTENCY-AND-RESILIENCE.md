# Stream semantics, distributed consistency and unattended recovery

## Design audit required
Identify actual source semantics (ordered? per-source cursor? retention? historical query? mesh identity?). Do not declare global exactly-once or global total ordering without proof. Prefer at-least-once delivery with idempotent projection + bounded resync, and explain possible gaps.

## Envelope
Use existing schemas if present; otherwise introduce a narrowly typed event envelope with `source_id`, `source_epoch`, `event_id`, `source_sequence?`, `occurred_at?`, `observed_at`, `cursor?`, `schema_version`, `redaction_class`, `payload_type`. A cursor is opaque and scoped to its source/stream; don't use wall time as cursor unless backend guarantees semantics.

## Recovery state machine
`DISCONNECTED → CONNECTING → SNAPSHOT_SYNC → STREAMING → STALE → RESYNC → STREAMING` with `UNAVAILABLE` distinct from idle. Each transition includes reason, timestamp, last-known cursor and counters. Backoff has jitter and cap. On expired cursor or integrity gap take a fresh authorized snapshot and resume from supported watermark. Deduplicate with bounded ID window. For reordering, only reorder within an explicit bounded/source-specific policy; otherwise annotate uncertainty.

## Storm control
Separate domain event ingestion from 30/60fps drawing. Process capped chunks each frame; batch visual effects without discarding semantic counters. Coalesce only with mathematically correct counts and an explicit aggregation window. Queue limits are configurable, measured and tested. On overflow, never silently resume as though complete: mark `GAP`, resync if possible.

## Hard failure drills
- Server kill/restart while projector runs.
- Wi-Fi/mesh partition and reconnection.
- Source clock skew, late arrivals, identical timestamps, duplicate IDs.
- Truncated cursor retention, schema changes and corrupted delta.
- Extremely chatty source and long quiet periods.
- Public policy change during stream; terminate/re-authorize, never retain older privileged stream.
- Renderer context loss, hidden tab, DPR change, full-screen exit.

## Identity and causality
Canonical node identity comes from verified Majordomus identity source, not labels, IP or topology guesswork. Cooperation arrows require an explicit source-backed interaction/correlation. Causality is not inferred from temporal proximity.
