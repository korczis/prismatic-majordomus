# 04 — Journal, websocket push, watchdog, restart and replay

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Reuse durable journal, event cursor and WebSocket. Track execution FSM and lease fencing in canonical durable state. Browser refresh does not reissue; reconnect replays missed events exactly once to UI; server crash/restart recovers queued/running/unknown outcomes and does not double mutate. Node lease expires → reassign only when safety semantics allow; unauthorized/stale holder cannot complete. Persist restart-safe executor without relying on a single Claude session, reuse integration owner/lease. Tests: kill process during work, restart, disconnect websocket, repeated click, delayed events, concurrent claim, bounded queue, EOF/stall and adversarial 5xx, no data loss.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
