# MAJORDOMUS AUTOPILOT · Policy evolution, scoped authority and evolving intent

## Operating contract for Claude Code (latest capable model)
You are operating on the LIVE repository `korczis/prismatic-majordomus`, not on a hypothetical greenfield system. Read AGENTS.md, CLAUDE.md, `.ai/**`, ADRs, schemas, policies, capability registry, integration and mesh modules, CI and existing tests first. Inspect CURRENT open PRs, master, checks, release and deployments. Treat repository code and observed evidence as authoritative over these examples. Use existing lifecycle, intent/milestone/issue DAG, rules/doctrine proof system, capability projections, PR queue, leases, evidence and releases. Never build a parallel orchestrator, registry, UI model, workflow or authority channel. Do not silently loosen correctness or security safeguards. Follow inspect → gap map → design → implement → tests → docs → enforcement → self-hosted demonstration → release as appropriate. If a requested feature already exists, improve it and remove competing implementation. Do not ask human to type 'proceed' for ordinary recoverable work. Use typed records and append-only events, stable identifiers, replay and idempotency. Provide a concise final evidence report with exact commands and real results. Do not claim deployment if it is not verified. If externally blocked, persist reason and continue independent work.

## Objective
Make autonomous completion the default for explicitly authorized runs, without opening a hole through validation, provenance, merge or deployment safeguards.

## Issues and implementation requirements

### Issue 1: Policy taxonomy
Identify hard invariants (trust, immutable evidence, no data loss, validated merge, secret boundaries), flexible process constraints (scope, review, handover cadence), heuristics (model/provider preferences), and explicit per-run authorization. Add typed severity/override semantics by deriving from existing policy model.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 2: Evolving scope protocol
When discovered work expands issue scope, write an amendable scope change proposal tied to original intent/criterion and impact evidence. Automatically approve within delegated envelope; recompute DAG/scope conflicts/gates. Escalate only scope crossing trust, repo, environment, destructive data or cost boundary. Do not silently edit another issue or rewrite historical approvals.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 3: Milestone intent reconciliation
Every PR repair must resolve to a canonical intent criterion, milestone and issue, or autonomously create/link them via existing plan transition mechanisms. Preserve objective and acceptance criteria while refreshing the plan from observed repository conditions.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 4: Authority profile
Define repo-scoped/autonomous-delivery authorization: allowable worktrees, commit/push, update/rebase, create PR, merge, publish, deployment environments, cost/time boundaries, concurrency. Trusted mesh identity != authorization; dispatch requires explicit capability token/delegation, short expiry and audit. Never store secrets in logs.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 5: Rules and migration
Modify existing project rules, policy, x-majordomus validator/test bindings, docs/CLAIMS and ADRs as needed. Convert needless manual-ack gates to deterministic machine proof, preserving branch protection and release integrity. Update legacy repositories through established update mechanism.

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
