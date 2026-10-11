# 07 — Governance, Gates & Proof

## Milestone outcome
Ensure guarantees are machine-enforced and survive future model/provider/repository changes.

## Prerequisites
Prior phases 01–06 verified where relevant; discover current HEAD before assuming.

## Issue 07.1 — Schema/doctrine integration

**Implementation mandate.** Map invariants to existing rule/doctrine/claim schema; add only missing enforceable requirements. Keep guarantees vs advisory vs planned honest.

**Must deliver:** typed or schema-backed behavior through existing owner subsystem; repository-level enforcement where appropriate; non-duplicated cross-surface wiring; actionable failure diagnostics; focused automated proof of success and at least one explicit failure mode; clear linkage to intent criteria.

**Acceptance:** Given a disposable fixture with relevant canonical inputs, the feature functions automatically; when inputs are missing, stale, contradictory or unauthorized it fails honestly and deterministically. A new entity does not require editing manual consumer inventories. Validate no unrelated behavior regressed and capture command + exit code + proof artifact.

## Issue 07.2 — Preflight and CI gates

**Implementation mandate.** Wire validation to current local checks, plan transitions, relevant hooks, PR/CI checks. Fail closed only for genuinely blocking invariants; keep unavailable external services explicit.

**Must deliver:** typed or schema-backed behavior through existing owner subsystem; repository-level enforcement where appropriate; non-duplicated cross-surface wiring; actionable failure diagnostics; focused automated proof of success and at least one explicit failure mode; clear linkage to intent criteria.

**Acceptance:** Given a disposable fixture with relevant canonical inputs, the feature functions automatically; when inputs are missing, stale, contradictory or unauthorized it fails honestly and deterministically. A new entity does not require editing manual consumer inventories. Validate no unrelated behavior regressed and capture command + exit code + proof artifact.

## Issue 07.3 — Behavioral matrix

**Implementation mandate.** Test incomplete critique; uncovered criterion; orphan work; stale evidence; changed goal; concurrent workers; missing provider; cancellation; injected repo; repeated run and deterministic projections.

**Must deliver:** typed or schema-backed behavior through existing owner subsystem; repository-level enforcement where appropriate; non-duplicated cross-surface wiring; actionable failure diagnostics; focused automated proof of success and at least one explicit failure mode; clear linkage to intent criteria.

**Acceptance:** Given a disposable fixture with relevant canonical inputs, the feature functions automatically; when inputs are missing, stale, contradictory or unauthorized it fails honestly and deterministically. A new entity does not require editing manual consumer inventories. Validate no unrelated behavior regressed and capture command + exit code + proof artifact.

## Issue 07.4 — Benchmarks & docs truthfulness

**Implementation mandate.** Measure added preflight and local-loop cost, document tradeoffs; derive docs/claims from evidence. Never advertise unimplemented capabilities as guaranteed.

**Must deliver:** typed or schema-backed behavior through existing owner subsystem; repository-level enforcement where appropriate; non-duplicated cross-surface wiring; actionable failure diagnostics; focused automated proof of success and at least one explicit failure mode; clear linkage to intent criteria.

**Acceptance:** Given a disposable fixture with relevant canonical inputs, the feature functions automatically; when inputs are missing, stale, contradictory or unauthorized it fails honestly and deterministically. A new entity does not require editing manual consumer inventories. Validate no unrelated behavior regressed and capture command + exit code + proof artifact.


## Non-negotiable execution contract
- Inspect actual checked-out HEAD first. Treat all paths and status descriptions here as hypotheses until verified.
- Read applicable AGENTS.md, .ai context chain, .majordomus, policies, rules, doctrines, ADRs, schemas, claims, tests and commands before changes. Never weaken governance to make this task pass.
- Reuse canonical typed models, registries, derived projections, evidence ledger and lifecycle. Do not create another state store, inventory, source of truth, UI model, API variant, or shell-script orchestration universe.
- Distinguish declared / observed / derived / inferred facts, include provenance and freshness. Derived states must not become mutable shadow status fields.
- Work autonomously: discover, design minimally, implement, run tests, repair regressions, document, enforce, demonstrate. Do not stop at analysis; do not ask for routine architecture choices.
- Never assert a passing check without executing it and recording exact command, exit code, revision and meaningful output. Unknown, stale, unavailable and blocked are not success.
- Prefer narrow incremental commits, existing task/issue/milestone machinery and resumable checkpoints; do not push, merge, deploy, publish or modify remote resources unless repository policy and user authorization permit it.
- Unsafe/irreversible, destructive, billable or external-side-effect actions require explicit authorization. Safe local planning/projections can be automatic.
- When a provider/peer is unavailable, degrade deterministically to single-worker local execution; never simulate real peer conclusions.
- Test both the Majordomus repository itself and an independently initialized/injected consumer fixture. All added features must work on both when relevant.
- Keep existing API/CLI contracts compatible or explicitly migrate/version them with schema and behavioral tests.
- Each issue-sized section below is independently testable, linked to its acceptance evidence and suitable for native .ai/GitHub plan projection.

## Required per-issue protocol
1. Locate current code and behavior, show concrete gap and provenance.
2. Specify input/output types, invariants, failure behavior and consumers.
3. Implement using existing subsystem boundaries; remove duplicate legacy path.
4. Write a positive test and at least one adversarial/negative or stale-evidence test.
5. Run focused tests, canonical checks and relevant cross-surface checks.
6. Update executable claim/ADR/doctrine only where project policy warrants it.
7. Record evidence references and mark the issue complete only when it actually passes.

## Required completion report
Report: audited base SHA; existing machinery reused; code changes; migrations; CLI/API/MCP/UI/docs impact; proof and commands with exit statuses; security/performance/compatibility impacts; unresolved blockers; derived satisfaction of every criterion; no invented successes.
