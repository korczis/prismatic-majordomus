# Wave: Collection, idempotency and provenance

Use after `00-MASTER-CLAUDE-CODE.md`. This is an acceptance contract, not authorization to bypass existing repo procedures.

## Task

Implement typed versioned source-preserving metering via existing event/state storage. OTLP logs/metrics correctness; delta/cumulative aggregation and temporality; per-request dedup, request retry, clock skew, offline replay, crash recovery, secret redaction, backpressure, retention, bounded read and coverage. Tests: duplicates, gaps, reorder, restart, silent emitter, contract upgrades. Source precedence explicit.

## Required deliverables

- Preflight evidence and canonical code ownership.
- Implemented vertical slice or documented external blocker.
- Exact commands/tests and results, source provenance, generated parity.
- Existing issues mapped to actual acceptance and verified outcomes.
- Any new claim is backed by data or marked unknown.
