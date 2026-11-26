# Phase 04 — Persistent outbox/inbox and crash safety

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Reuse the existing durable event/state store; build transactional outbox/inbox or equivalent. Preserve causal links while preventing duplicate external effects. Define idempotency record lifetime, per-topic/per-peer ordering bounds, failure classification, retry/backoff with jitter, TTL expiry, dead-letter and operator recovery. Persist intended recipient/permissions as needed without treating stale authorization as current permission. Crash-injection at every transition: sender before commit, sender after commit before send, receiver before ACK, receiver after persistence before processing, receiver during side effect, server restart. Never claim exactly once. Add deterministic state-machine tests, load and disk-full behavior, protocol version mismatch and offline recipient replay. Evaluate whether durable payload data belongs in existing content-addressed store; avoid stuffing raw conversations in Git/.ai.

## Hard acceptance / evidence
Evidence: fault matrix with passing repeated runs, restart recovery, stable DB migrations, replay trace, disk/queue limits and no lost accepted durable messages under specified fault model.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
