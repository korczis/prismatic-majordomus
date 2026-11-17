# Milestone 20: Turn Critique Records into an Executed Opposition Pass

## Mission

The repository already has typed gap/critique records and can reject execution that started before a critique. That is necessary but insufficient. Implement the canonical orchestration that actually performs opposition, records its provenance, and gates plan acceptance.

The goal is not "an LLM wrote a critique field." The goal is a lifecycle transition with deterministic eligibility rules and auditable semantic review.

## 1. Audit existing critique/gap model

Determine:

- where gap records are produced;
- how plans are represented;
- current critique schema;
- existing provider/worker orchestration;
- execution waves/peer roles;
- provenance/evidence mechanisms;
- how preflight currently detects absent critique.

Reuse these.

## 2. Define opposition lifecycle

Implement typed states equivalent to the repository's conventions for:

```text
plan draft
→ opposition required
→ opposition running/completed
→ disposition:
    accept
    accept-with-required-changes
    reject/replan
→ executable plan
```

Do not permit a critique record to become equivalent to "opposition passed" merely because a file exists.

## 3. Opposition input contract

The opposition pass must receive canonical inputs sufficient to challenge:

- intent statement;
- invariants;
- acceptance criteria;
- discovered gaps/context;
- proposed milestones/issues/dependencies/waves;
- assumptions;
- safety/compatibility constraints;
- relevant repository doctrines and ADRs.

Context must be bounded and derived through existing context compilation, not ad hoc file concatenation.

## 4. Opposition output contract

Use/extend typed critique data to capture at least:

- challenged assumptions;
- missing criteria/work;
- invariant conflicts;
- dependency/order problems;
- test/evidence gaps;
- scope inflation;
- risk items;
- required plan changes;
- disposition;
- provenance: actor/model/tool/session/time/input revision.

If the existing schema represents this differently, preserve repository style.

## 5. Execution strategy

Use available existing worker/provider orchestration. The system must function with a single capable provider; multi-provider or peer adversarial review is an enhancement, not a dependency.

When multiple providers/peers are available, use existing capability routing rather than hardcoded vendor names.

## 6. Staleness and invalidation

A plan-changing mutation must invalidate prior opposition when the change is material to what was reviewed. Define deterministic invalidation rules.

Examples likely requiring re-opposition:

- criteria added/removed/changed;
- invariants changed;
- issue decomposition materially changed;
- dependencies/waves changed;
- assumptions changed.

Trivial metadata edits should not trigger expensive semantic review unless policy requires it.

## 7. Gate execution

Integrate opposition disposition with session/intent preflight from Milestone 10.

Execution must refuse:

- no completed opposition where required;
- rejected plan;
- accepted-with-required-changes where required changes are unresolved;
- stale opposition against a newer plan revision.

## 8. Explainability

Existing explain/doctor/diagnostic surfaces should be able to answer:

- why opposition is required;
- which revision was reviewed;
- who/what performed it;
- what changes were required;
- why execution is blocked.

## 9. Tests

Cover:

- opposition orchestrator creates canonical critique/disposition;
- accept path;
- reject path;
- required-change path;
- plan mutation invalidates critique;
- irrelevant metadata change does not invalidate if policy says so;
- single-provider fallback;
- optional multi-peer path if existing infrastructure supports it;
- execution gate respects disposition;
- provenance is persisted;
- malformed/free-form critique cannot bypass typed validation.

## 10. Docs/gates

Document opposition as part of intent lifecycle, not as optional advice.

Ensure validation catches manually fabricated/incomplete critique state that lacks required provenance/disposition.

## Acceptance criteria

- Majordomus can run opposition itself through its canonical orchestration path;
- output is typed and auditable;
- plan revisions invalidate stale reviews correctly;
- only accepted valid review state permits intent execution;
- no optional provider is required for the feature to function.
