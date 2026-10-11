# MAJORDOMUS AUTOPILOT · Verified autonomous release, deploy and self-hosted proof

## Operating contract for Claude Code (latest capable model)
You are operating on the LIVE repository `korczis/prismatic-majordomus`, not on a hypothetical greenfield system. Read AGENTS.md, CLAUDE.md, `.ai/**`, ADRs, schemas, policies, capability registry, integration and mesh modules, CI and existing tests first. Inspect CURRENT open PRs, master, checks, release and deployments. Treat repository code and observed evidence as authoritative over these examples. Use existing lifecycle, intent/milestone/issue DAG, rules/doctrine proof system, capability projections, PR queue, leases, evidence and releases. Never build a parallel orchestrator, registry, UI model, workflow or authority channel. Do not silently loosen correctness or security safeguards. Follow inspect → gap map → design → implement → tests → docs → enforcement → self-hosted demonstration → release as appropriate. If a requested feature already exists, improve it and remove competing implementation. Do not ask human to type 'proceed' for ordinary recoverable work. Use typed records and append-only events, stable identifiers, replay and idempotency. Provide a concise final evidence report with exact commands and real results. Do not claim deployment if it is not verified. If externally blocked, persist reason and continue independent work.

## Objective
Take an intent-driven campaign to a real, verified, bounded conclusion, document it and enforce ongoing architecture.

## Issues and implementation requirements

### Issue 1: End-to-end execution
From existing intent and PR queue, autonomously reconcile plan, assign peer, update branch, fix, test, merge, bump version according to current semantics, release artifact, verify provenance, deploy to pre-authorized target and probe runtime.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 2: Completion policy
Translate actual acceptance criteria and release evidence to finish decision. Release/deploy gated by independently verified observations; missing permissions/environment/secret prompts become explicit blockers not fabricated success.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 3: Self-host and consumer
Run on Majordomus itself and clean injected repository. Demonstrate normal success, a fixable conflict, crash/restart, unavailable peer fallback, no progress and budget exhaustion. Evidence and timings attached.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 4: Docs/projections
Update operator guide: `prs drain --until-settled`, background service status, resume, permissions, troubleshooting; generated OpenAPI, MCP, UI and Pages from capabilities; docs/CLAIMS backed by tests.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 5: Rollout and compatibility
Feature-gate risky mutations during migration, preserve existing `prs drain` semantics absent explicit autonomous profile, provide migration and rollback, publish version only once checks prove it, verify deployed version and health.

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
