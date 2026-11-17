# MAJORDOMUS AUTOPILOT · Durable reconciling Autopilot supervisor

## Operating contract for Claude Code (latest capable model)
You are operating on the LIVE repository `korczis/prismatic-majordomus`, not on a hypothetical greenfield system. Read AGENTS.md, CLAUDE.md, `.ai/**`, ADRs, schemas, policies, capability registry, integration and mesh modules, CI and existing tests first. Inspect CURRENT open PRs, master, checks, release and deployments. Treat repository code and observed evidence as authoritative over these examples. Use existing lifecycle, intent/milestone/issue DAG, rules/doctrine proof system, capability projections, PR queue, leases, evidence and releases. Never build a parallel orchestrator, registry, UI model, workflow or authority channel. Do not silently loosen correctness or security safeguards. Follow inspect → gap map → design → implement → tests → docs → enforcement → self-hosted demonstration → release as appropriate. If a requested feature already exists, improve it and remove competing implementation. Do not ask human to type 'proceed' for ordinary recoverable work. Use typed records and append-only events, stable identifiers, replay and idempotency. Provide a concise final evidence report with exact commands and real results. Do not claim deployment if it is not verified. If externally blocked, persist reason and continue independent work.

## Objective
Make `majordomus prs drain` capable of finishing an explicitly authorized campaign unattended, through long-running supervised workers rather than one conversational Claude session.

## Issues and implementation requirements

### Issue 1: Typed run model
Extend current session/issue/integration events with RunId, desired outcome, authority policy ref, repo/base, active milestone/intent, phase, cursor, attempt, lease, deadline, progress watermark, blockers and evidence. Use existing storage ownership; durable replay and versioned migration.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 2: Supervisor lifecycle
Design idempotent ensure/start/status/stop/resume and graceful shutdown with one leader per scoped campaign; persist before side effects and reconcile outcome after ambiguous response (especially merge). Detect dead owner via lease heartbeat/fencing token, not time alone. Do not spawn duplicate workers on restart.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 3: No-progress and budget controls
Distinguish blocked-with-actionable-fix from blocked-with-external-dependency. Enforce bounded backoff, max equivalent failure attempts, per-task spend/time tokens, global run budget, circuit breaker and safe resume. Continue other independent DAG nodes; end only SATISFIED, BLOCKED, BUDGET_EXHAUSTED, CANCELLED or FATAL.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 4: Resident execution
Detach from terminal using existing shared server lifecycle or a justified supervised service, with shutdown/recovery tests. `--continuous` keeps observing; `--until-settled` attempts remediation. Do not rely on user leaving Claude Code window open. CLI follow/watch streams current run, no new authoritative state.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 5: UI/API projection
Expose run, checkpoints, unresolved blocker and next action using capability registry; infer high-level overview and mobile-usable cockpit controls from same state.

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
