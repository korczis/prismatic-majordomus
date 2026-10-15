# Milestone 30: Canonical Criterion and Intent Satisfaction Engine

## Mission

Implement the deterministic engine that decides whether acceptance criteria are satisfied from canonical evidence. This is the semantic heart of intent-driven completion.

Do not use issue state, milestone closure or hand-edited booleans as proof.

## 1. Audit evidence and criterion state

Inspect:

- evidence ledger types;
- criterion schema;
- existing derived criterion state;
- validation of evidence links;
- test/report/deployment/claim evidence types;
- current intent stage derivation;
- completion policy.

Preserve what already exists and fill only the missing semantics.

## 2. Define evaluation contract

Create/reuse a typed evaluation result capable of expressing more than yes/no. Conceptually it may include:

```text
unsatisfied
partially_satisfied
satisfied
blocked
invalid/stale evidence
```

Use repository-native names.

Each evaluation should explain:

- criterion;
- applicable evidence requirements;
- evidence considered;
- evidence rejected and why;
- resulting state;
- blockers/gaps;
- evaluator policy/version.

## 3. Evidence admissibility

Define deterministic policy for whether evidence can satisfy a criterion.

Consider existing concepts such as:

- evidence type/kind;
- target/revision/commit;
- freshness;
- source/provenance;
- pass/fail status;
- environment;
- reproducibility;
- criterion-specific requirements.

Do not make an LLM the final authority for machine-verifiable facts like test pass, build success, schema validation or deployment checks.

Semantic criteria may use an evaluator, but the resulting claim must still be typed, attributable and policy-validated.

## 4. Stale evidence

Evidence tied to an old revision must not silently satisfy changed work when repository semantics say it is stale.

Reuse existing executable/reference/staleness mechanisms where possible.

## 5. Criterion aggregation

Define intent satisfaction from required criteria and invariants.

At minimum:

- optional criteria do not block unless policy says otherwise;
- required criteria must be satisfied;
- invalidated evidence can regress a criterion;
- a violated invariant blocks intent satisfaction regardless of issue closure;
- derived intent stage reflects actual semantic state.

Avoid circular derivation.

## 6. Reassessment

Provide the canonical operation used after evidence changes to recompute:

- criteria;
- milestones/projected progress if applicable;
- intent stage;
- completion blockers.

If state is purely derived on read, preserve that model; do not materialize redundant caches without reason.

## 7. Explainability

CLI/API/MCP model should be able to expose why a criterion is or is not satisfied, including missing/stale evidence.

## 8. Tests

Cover at least:

- no evidence → unsatisfied;
- admissible evidence → satisfied;
- failing evidence does not satisfy;
- stale evidence invalidates prior satisfaction;
- multiple required evidence items;
- optional criterion behavior;
- invariant violation blocks intent;
- evidence for wrong criterion/work/revision rejected;
- intent with all required criteria satisfied derives completed/satisfied stage;
- closing all issues alone does not satisfy intent;
- deterministic repeatability.

## 9. Migration

If existing fixtures/intents rely on manual satisfaction fields or legacy semantics, migrate them and remove bypasses.

## Acceptance criteria

- criterion state is canonically derived from admissible evidence;
- intent satisfaction is canonically derived from required criteria/invariants;
- stale or invalid evidence cannot keep an intent falsely green;
- issue/milestone status alone cannot mark intent satisfied;
- every result is explainable and tested.
