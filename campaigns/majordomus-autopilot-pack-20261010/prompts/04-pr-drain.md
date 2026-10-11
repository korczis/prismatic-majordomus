# MAJORDOMUS AUTOPILOT · Autonomous PR repair, refresh and drain

## Operating contract for Claude Code (latest capable model)
You are operating on the LIVE repository `korczis/prismatic-majordomus`, not on a hypothetical greenfield system. Read AGENTS.md, CLAUDE.md, `.ai/**`, ADRs, schemas, policies, capability registry, integration and mesh modules, CI and existing tests first. Inspect CURRENT open PRs, master, checks, release and deployments. Treat repository code and observed evidence as authoritative over these examples. Use existing lifecycle, intent/milestone/issue DAG, rules/doctrine proof system, capability projections, PR queue, leases, evidence and releases. Never build a parallel orchestrator, registry, UI model, workflow or authority channel. Do not silently loosen correctness or security safeguards. Follow inspect → gap map → design → implement → tests → docs → enforcement → self-hosted demonstration → release as appropriate. If a requested feature already exists, improve it and remove competing implementation. Do not ask human to type 'proceed' for ordinary recoverable work. Use typed records and append-only events, stable identifiers, replay and idempotency. Provide a concise final evidence report with exact commands and real results. Do not claim deployment if it is not verified. If externally blocked, persist reason and continue independent work.

## Objective
Transform existing `prs drain` from merge queue consumer into a convergent, evidence-driven repair + integration workflow without throwing away its classifier, merge-tree, per-base lock or event log.

## Issues and implementation requirements

### Issue 1: Existing lanes
Inspect exact integration lane semantics. Ready → validate/re-observe → merge; refreshable → safe update; fixable → create canonical task with worker assignment; blocked external → observe/backoff; superseded → closure only after strong evidence. No title/age based guessing.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 2: Safe branch maintenance
Choose merge/rebase/update according to repo policy, preserve authorship, never force-update protected/unowned branches without delegated permission. For stale branch create isolated worktree, reconcile against latest base, rerun impacted checks and update PR.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 3: Incremental progression
On every successful mutation re-observe head/base and recompute queue. Group conflict-independent repairs for parallel execution; serialize actual base-branch merges; retain per-base lease with crash-safe reclaim and fencing. Never retry ambiguous merge blindly.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 4: Review and checks
Respect required checks and review requirements, stale evidence and security gates. Automate request for verification/review where trusted policy supports; no forged approvals or disabling safeguards. If a check is non-impacting based on canonical gate model, avoid redundant local execution but obey actual branch rules.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 5: Queue convergence contract
Finish when all in-scope PRs are merged, provably superseded/closed under policy, or explicitly externally blocked and all automatic options exhausted. Report counts and individual explanations, no false all-done.

Acceptance: deliver production code, targeted negative + positive tests, executable gate coverage, generated projection/docs update, proof of integration with canonical subsystem. Record exact evidence and blockers.

### Issue 6: Simulation
Fixtures for 20+ PRs: conflicts, moving base, stale review, tests fail, worker crash, force push race, same-file overlap, GitHub rate limits, status timeout, unknown repo fork and long-lived branch.

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
