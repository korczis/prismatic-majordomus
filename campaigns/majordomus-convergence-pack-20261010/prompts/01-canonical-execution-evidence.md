# Milestone 01: Canonical execution and evidence convergence

First read and honor `00` report and current live master + PRs. Inspect existing evidence ledger, intent criteria, task/session completion, capability registry, execution/command graph, diagnostics and Git identities. Do not invent a parallel task database, tracing service, orchestration layer or manually maintained data catalog.

## Issue 01.1: Canonical execution identity

Reuse existing typed models wherever possible. Define or refine operation identity; repository HEAD, worktree, intent/issue/session references, exact inputs and hashes, generator/capability version, dependency keys, environment identity, output digests and timestamps. Clarify which fields are computed/immutable, optional, redactable or locally scoped. Ensure schema versioning/migrations and deterministic serialization. No secrets, raw provider credentials or private transcripts in persistent evidence.

## Issue 01.2: Integrate telemetry into evidence ledger

Record start/end, measured wall and CPU durations if available, statuses, retries, cache decisions, causal dependencies and diagnostic references without replacing existing outcome evidence. Failures must remain failures. Avoid pretending that a cache hit is a verified deployment, or that local tests settle production health. Add provenance linking to commit/PR, environment and artifact version.

## Issue 01.3: Real lifecycle and projection reuse

Thread canonical records through existing `check`, `finish`, issue/milestone/intent lifecycle, CLI status, REST/OpenAPI, MCP and UI consumers where such consumers exist. Implement using the existing registry/projection pipeline. If UI or endpoint is currently missing, document gap and do not add speculative parallel interfaces in this milestone. Ensure idempotent reads, append-only durable evidence where required, stale-state detection and minimal sufficient context.

## Issue 01.4: Adversarial verification

Unit and integration tests for duplicate execution events, retry, clock skew, changed HEAD, ambiguous refs, reordered independent events, concurrent writers, crash/recovery, secret redaction, corrupted payload, incompatible schema and stalled worker. Failure must be actionable. Add schema, doctrine/rule and CI wiring only through existing governance model, including a negative test that proves the gate actually fails.

## Issue 01.5: Migration and observability

Migrate legacy evidence with explicit lossless handling where possible; never fabricate historic measurements. Delete deprecated redundant state readers after migration. Generate docs/claim traceability from canonical model; report actual commands and proofs.

Acceptance: one query can explain a finished task from intent to commit, tests and deploy (where present), using only canonical evidence and honestly marking unknown steps. No duplicate manual registration per surface.
