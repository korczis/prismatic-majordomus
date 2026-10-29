# MAJORDOMUS AUTOPILOT · Incremental execution and bounded pipeline latency

## Operating contract for Claude Code (latest capable model)
You are operating on the LIVE repository `korczis/prismatic-majordomus`, not on a hypothetical greenfield system. Read AGENTS.md, CLAUDE.md, `.ai/**`, ADRs, schemas, policies, capability registry, integration and mesh modules, CI and existing tests first. Inspect CURRENT open PRs, master, checks, release and deployments. Treat repository code and observed evidence as authoritative over these examples. Use existing lifecycle, intent/milestone/issue DAG, rules/doctrine proof system, capability projections, PR queue, leases, evidence and releases. Never build a parallel orchestrator, registry, UI model, workflow or authority channel. Do not silently loosen correctness or security safeguards. Follow inspect → gap map → design → implement → tests → docs → enforcement → self-hosted demonstration → release as appropriate. If a requested feature already exists, improve it and remove competing implementation. Do not ask human to type 'proceed' for ordinary recoverable work. Use typed records and append-only events, stable identifiers, replay and idempotency. Provide a concise final evidence report with exact commands and real results. Do not claim deployment if it is not verified. If externally blocked, persist reason and continue independent work.

## Objective
Remove redundant work from derive/build/test/deploy while preserving deterministic output and evidence freshness.

## Issues and implementation requirements

### Issue 1: Profile full critical path
Instrument meaningful stages and record measurement provenance; distinguish cold/warm builds, Mac/Linux runner, local and CI, queue/retry overhead and artifact publication. Baseline from repo, not assumed targets.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 2: Derivation DAG
Reuse existing derive and dependency model; declare canonical inputs/tool versions/output hashes, derive transitive invalidation. Single-flight per independent operation key; atomic staging and publish. Global lock only when shared destination truly conflicts.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 3: Reusable CI selection
Integrate path-class gates and implications with affected closure, cache key, proof of skipped irrelevant work, remote vs local cache integrity, concurrency and precise invalidation. Stale verification never reused.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 4: Release pipeline
Release once per valid policy-defined delivery boundary; avoid meaningless version bumps and full site rebuild on no-op. Preserve attestation and integrity checks. Add reproducible caches and inspect true bottleneck rather than promise arbitrary 60s for full pipeline.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 5: Budget gate
Define per-scenario observed performance budgets and ratchets in existing CI policy. On regression provide stage-level explanation and reproducible benchmark fixture.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

## Universal mandatory checks
- Every newly introduced canonical record has defined ownership, schema version, invariants, provenance, migrations and deterministic serialization where applicable.
- Derive CLI, JSON, API/OpenAPI, MCP, Cockpit and docs from existing registries/projections. Avoid separately maintained tables; only expose where meaningful.
- Cover unit, integration, process restart, crash recovery, concurrent contender, negative-security, CI/drift and end-to-end tests. Use existing test mechanisms; add fixtures only when needed.
- Changes to policies or rules must include executable proof and a migration for existing repositories (`majordomus update` or current mechanism); do not make docs-only rules.
- CI must run appropriate impact-selected gates; gate results are bound to exact relevant inputs and stale results never become successful evidence.
- All mutations use explicit permission checks, bounded scopes, idempotency keys or reconciliation, and an audit trail. No untrusted peer may inherit authorization from discovery.
- Validate on both self repository and a clean injected consumer fixture. Keep backward compatibility where possible, version genuine breaks.
- Maintain explicit stop conditions for unrecoverable failure, budget exhaustion, trust/auth mismatch, required approval, and lack of progress; report BLOCKED with evidence, never fake COMPLETED.

## Required completion report
Current revision and PR snapshot; what exists vs was missing; architectural changes; updated invariants; migrated legacy consumers; new/changed tests and real pass/fail output; performance before/after when relevant; produced docs/projections; merged PRs; version/release/deploy verification; remaining blockers with concrete reason. No simulated success.
