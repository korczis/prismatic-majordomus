---
id: github-workgraph-phase-06
phase: 6
depends_on: [github-workgraph-phase-05]
goal: governance and machine-enforced invariants
---

# Phase 06 — Rules, Doctrines, Policies, Doctor, Check, and Finish Enforcement

Read shared contract and prior handovers.

## Objective

Encode the architecture as repository governance that is actually executed.

Do not add decorative Markdown proclamations that CI never invokes.

## Determine the correct governance layer

Inspect existing definitions of:
- rule;
- doctrine;
- policy;
- schema;
- workflow;
- validation;
- doctor;
- check;
- finish;
- git hook;
- CI gate.

Place each invariant in the correct canonical layer.

## Required invariants

Implement equivalent enforceable rules:

### Work Traceability
Every managed implementation change must have a machine-traceable path from canonical execution contract to relevant branch/worktree/code/evidence according to the repository lifecycle.

### External Projection
GitHub is an external projection/observation plane. Synchronized fields have explicit authority/provenance. No second hand-maintained copy of canonical semantics.

### No Orphan Managed Work
Managed feature worktrees/branches/PRs must map to canonical work or an explicitly allowed exception.

### Evidence-Based Completion
Remote issue closure/PR merge alone cannot mark canonical work accepted.

### Dependency Integrity
A blocked dependency cannot be bypassed by manually changing a status field. Readiness is derived.

### Reconciliation Integrity
Validation/check mode may observe and diff but must not mutate remote state. Mutation happens only through explicit reconcile/apply lifecycle.

### Projection Drift
Generated projections and schemas must be reproducible from canonical sources; drift fails the appropriate gate.

### Capability Single Source
New GitHub/workgraph operations must follow the canonical capability mechanism and generation path.

## Diagnostics

Every failure should include structured data and a concise fix.

Examples:
- `orphan-worktree`
- `branch-work-item-mismatch`
- `missing-github-mapping`
- `github-projection-drift`
- `premature-remote-close`
- `unsatisfied-acceptance`
- `ambiguous-commit-attribution`
- `dependency-cycle`
- `stale-check-evidence`
- `remote-permission-insufficient`

Use existing diagnostic IDs/style.

## Doctor/check/finish integration

Decide:
- which checks belong in fast local `check`;
- which remote observations belong in explicit GitHub status/check;
- which checks gate `finish`;
- which checks can run offline;
- how unavailable network is distinguished from clean state.

Never make an offline developer unable to run unrelated local checks merely because GitHub is unavailable, unless current project policy explicitly requires network.

`finish` should refuse canonical completion when required evidence/traceability is unsatisfied.

## Hooks

Integrate only lightweight deterministic local checks into pre-commit/pre-push based on current conventions.

Do not make every commit wait on the GitHub API.

## Self-enforcement

Add tests proving:
- a rule is not only declared but reachable from the canonical validator;
- a new invalid fixture fails;
- a valid fixture passes;
- bypass paths do not exist through alternative CLI/API/MCP operations.

## Gate

The architectural invariants must be executable and reachable from existing quality lifecycle. Update generated AGENTS/policy-derived outputs only through their canonical source/generator.
