# M02 — Caching & invalidation

Dependencies: M01. Reconcile canonical intent/milestone/issue identifiers before implementation. All results must be real and evidence-backed.

## M02-I01: Fingerprint dependency closure

1. Inspect existing implementation and canonical declarations relevant to this task, including related PRs. Record file paths and initial behavior.
2. Design the minimal typed change; document ownership, dependencies, permission model, performance budget, failure and invalidation modes.
3. Implement in existing engine/registry/UI; delete superseded duplicate mechanisms only after migrated consumers pass tests.
4. Add focused unit and integration tests including a negative/failure path; test actual data/route where applicable.
5. Prove cross-surface parity, no-JS fallback, security and provenance if affected; capture timings from real execution.
6. Update `.ai/`, schema/gate/ADR (only where warranted), docs and generated artifacts through repository machinery.
7. Format/build/validate, integrate safely, evaluate version bump, release/deploy if authorized and eligible, verify live result; add exact evidence references to canonical issue.

**Exit:** traceable implementation, no new duplicate data authority, tests pass, measured regression status reported, downstream consumers updated, blockers explicit.

## M02-I02: Single-flight cache keyed by scope and revision

1. Inspect existing implementation and canonical declarations relevant to this task, including related PRs. Record file paths and initial behavior.
2. Design the minimal typed change; document ownership, dependencies, permission model, performance budget, failure and invalidation modes.
3. Implement in existing engine/registry/UI; delete superseded duplicate mechanisms only after migrated consumers pass tests.
4. Add focused unit and integration tests including a negative/failure path; test actual data/route where applicable.
5. Prove cross-surface parity, no-JS fallback, security and provenance if affected; capture timings from real execution.
6. Update `.ai/`, schema/gate/ADR (only where warranted), docs and generated artifacts through repository machinery.
7. Format/build/validate, integrate safely, evaluate version bump, release/deploy if authorized and eligible, verify live result; add exact evidence references to canonical issue.

**Exit:** traceable implementation, no new duplicate data authority, tests pass, measured regression status reported, downstream consumers updated, blockers explicit.

## M02-I03: HTTP ETag/conditional response and safe asset headers

1. Inspect existing implementation and canonical declarations relevant to this task, including related PRs. Record file paths and initial behavior.
2. Design the minimal typed change; document ownership, dependencies, permission model, performance budget, failure and invalidation modes.
3. Implement in existing engine/registry/UI; delete superseded duplicate mechanisms only after migrated consumers pass tests.
4. Add focused unit and integration tests including a negative/failure path; test actual data/route where applicable.
5. Prove cross-surface parity, no-JS fallback, security and provenance if affected; capture timings from real execution.
6. Update `.ai/`, schema/gate/ADR (only where warranted), docs and generated artifacts through repository machinery.
7. Format/build/validate, integrate safely, evaluate version bump, release/deploy if authorized and eligible, verify live result; add exact evidence references to canonical issue.

**Exit:** traceable implementation, no new duplicate data authority, tests pass, measured regression status reported, downstream consumers updated, blockers explicit.

## M02-I04: Atomic invalidation and cache diagnostics

1. Inspect existing implementation and canonical declarations relevant to this task, including related PRs. Record file paths and initial behavior.
2. Design the minimal typed change; document ownership, dependencies, permission model, performance budget, failure and invalidation modes.
3. Implement in existing engine/registry/UI; delete superseded duplicate mechanisms only after migrated consumers pass tests.
4. Add focused unit and integration tests including a negative/failure path; test actual data/route where applicable.
5. Prove cross-surface parity, no-JS fallback, security and provenance if affected; capture timings from real execution.
6. Update `.ai/`, schema/gate/ADR (only where warranted), docs and generated artifacts through repository machinery.
7. Format/build/validate, integrate safely, evaluate version bump, release/deploy if authorized and eligible, verify live result; add exact evidence references to canonical issue.

**Exit:** traceable implementation, no new duplicate data authority, tests pass, measured regression status reported, downstream consumers updated, blockers explicit.
