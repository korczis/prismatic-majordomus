# Realtime protocol & durable semantics

The observed `docs/COCKPIT.md` says existing execution pages follow a WebSocket event channel at `GET /events` with `executions.get/events`, and issue transitions via `plan.transition` currently use bounded polling. Read `docs/EXECUTIONS.md`, ADR 0033 and implementation before changing protocol.

## Event envelope

Require typed protocol version, monotonic sequence *within a documented scope*, unique event ID, scope/parent execution/child execution IDs, entity refs, event type, event time vs observed time, actor, payload schema and privacy classification. Reuse existing envelope when possible. Events are hints to refresh canonical read models, not independent state authority. Gate consumers against unknown versions.

## Connect

1. Authorize request for exact repo + target scope.
2. Atomically read snapshot with cursor or record snapshot watermark.
3. Subscribe/replay from cursor (server either replays gap or forces explicit resync).
4. Apply events only in cursor order, dedupe same IDs, recover out-of-order/missed messages.
5. Refresh canonical snapshot after terminal transition or version mismatch.

Reconnect with exponential backoff + jitter, bound connection attempts, expose state. Avoid background polling storms. Handle browser sleep/reopen and multiple tabs. Maintain stable exec deep links. Protect against buffer overflow by bounded queues/coalesced progress updates while preserving terminal status and durable evidence.

## Security

Enforce origin/session auth, event-level permissions, no cross-repository leak, no arbitrary subscription path traversal, redaction of tokens, provider prompts, environment secrets and internal unpublished docs. Never trust peer message to authorize a mutation. Rate limit and account for resource use.

## Negative tests

Gap, duplicate, reorder, disconnect, logout while connected, token expiry, server reboot, worker crash, lease takeover, stale plan, malformed payload, cross-project access, slow receiver and secret-bearing log. Each either recovers a truthful latest state or fails visibly.
