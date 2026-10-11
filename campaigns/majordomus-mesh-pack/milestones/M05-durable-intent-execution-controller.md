# M05 — Durable intent execution controller

> Milestone contract | Parent: MASTER.md | Order: dependency-aware, not blindly sequential.

## Outcome
Make autonomous continuation runtime-owned, never dependent on a long chat.

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
## Issue 05.01: Define typed legal transitions, terminal states and blocker taxonomy

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.02: Persist state transitions and checkpoint before external side effects

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.03: Establish operation IDs, outbox/inbox and idempotent reconciliation

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.04: Implement resume after process, machine, network and provider failures

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.05: Compute next eligible work from dependency and policy constraints

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.06: Support safe cancellation, bounded retries, backoff and circuit breakers

**Implementation contract**
- Inspect the closest existing typed implementation and associated tests; write a path-level before/after finding.
- Specify the canonical ownership boundary, invariant, public consumer(s), schema or migration impact, and rollback/recovery behavior.
- Implement the smallest reusable vertical increment; do not create a parallel inventory or a consumer-specific source of truth.
- Add a positive case, a negative/failure case, and a restart/concurrency/compatibility case where applicable.
- Expose diagnostics and evidence with stable identifiers. Integrate through the actual repository check and PR gates.

**Acceptance evidence**: changed file references; tests and exact outputs; canonical graph relationships; impacted projections; merge/release evidence when applicable.
**Anti-pattern rejection**: manually synchronized lists, inferred completion from activity, swallowed validation failures, simulated claims presented as deployment proof.
## Issue 05.07: Test replay, crash points, duplicate callbacks and stuck states

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
