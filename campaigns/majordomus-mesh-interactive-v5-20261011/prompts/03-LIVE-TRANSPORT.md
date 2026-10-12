# 03-LIVE-TRANSPORT: Live node monitoring and streaming

## Shared execution contract
For each issue: discover existing implementation, implement minimal compatible change, prove with a positive and negative test, update canonical documentation and .ai enforcement where applicable, verify UI/CLI/API/MCP parity, and record actual evidence. No fake statuses, metrics, or unreviewed remote-execution shortcuts.

## M03-I01: Audit existing execution GET /events WebSocket; reuse framing/auth/heartbeats, extend using typed mesh events only if required
**Objective:** Audit existing execution GET /events WebSocket; reuse framing/auth/heartbeats, extend using typed mesh events only if required.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.

## M03-I02: Define snapshot-then-subscribe with monotonic cursor, sequence, deduplication, versioned envelope and replay policy
**Objective:** Define snapshot-then-subscribe with monotonic cursor, sequence, deduplication, versioned envelope and replay policy.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.

## M03-I03: Stream real mesh link transitions, heartbeats, claims, conflicts, handovers, reviews, task and execution progress where actually emitted
**Objective:** Stream real mesh link transitions, heartbeats, claims, conflicts, handovers, reviews, task and execution progress where actually emitted.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.

## M03-I04: Implement per-node subscription filtering, bounded backpressure, bounded buffers, masking of secrets and explicit retention limits
**Objective:** Implement per-node subscription filtering, bounded backpressure, bounded buffers, masking of secrets and explicit retention limits.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.

## M03-I05: Reconnect with exponential backoff, gap detection, catch-up snapshot, stale/offline badges and no fabricated activity
**Objective:** Reconnect with exponential backoff, gap detection, catch-up snapshot, stale/offline badges and no fabricated activity.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.

## M03-I06: Test socket churn, event reorder/duplicate/gap, browser sleep, tab close, server restart and mesh partition
**Objective:** Test socket churn, event reorder/duplicate/gap, browser sleep, tab close, server restart and mesh partition.
**Deliverables:** repository code and/or canonical declarations, route/UI projection, relevant test fixtures, observable diagnostics, documentation and tracker references.
**Acceptance:** verified on a real local server, traceable to canonical IDs and authorization, deterministic and correct under negative conditions; provide test command, result and SHA.
