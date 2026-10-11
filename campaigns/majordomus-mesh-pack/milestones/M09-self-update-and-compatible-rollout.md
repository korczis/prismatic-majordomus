# M09 — Self-update and compatible rollout

> Milestone contract | Parent: MASTER.md | Order: dependency-aware, not blindly sequential.

## Outcome
Upgrade Majordomus and injected projects without tearing down its controller.

## Preconditions
- Execute M00 first; inspect repository at current HEAD.
- Resolve current policy, scope, active intents, issues and worktrees.
- Reuse existing Majordomus state models and command conventions.
- Link this milestone to the canonical intent and source segment after ingestion.

## Acceptance gates
1. Domain model and source of truth are explicitly identified and tested.
2. At least one real consumer uses the implementation; no UI-only prototype.
3. Required checks, negative tests and schema/projection drift checks pass.
4. Work is committed and integrated via existing protected-branch policy.
5. Durable evidence includes test IDs, commits/PRs, status and links to source requirements.
6. Where release/deploy applies, actual deployed artifacts are verified.

## Issues (each subsection is one bounded executable issue)
## Issue 09.01: Inspect existing updater, installer, .ai vendoring and version policy

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.02: Define component capability/schema compatibility and migration requirements

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.03: Stage update alongside running version with checkpoint and ownership handoff

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.04: Run rollbackable migrations, health checks and atomic activation

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.05: Preserve local customization and explain generated drift

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.06: Synchronize peers and read models after upgrade

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 09.07: Test interrupted upgrade, downgrade incompatibility and rollback

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.

## Milestone completion rule
Mark completed only if *every* issue acceptance criterion is supported by accepted evidence. Distinguish implemented, verified, integrated, released and deployed. If blocked, record exact failed gate and continue unrelated eligible work.
