# Development and Review Rules

## Implementation sequence

Prefer:

1. establish the invariant,
2. define observable behavior,
3. implement the smallest mechanism,
4. add behavioral tests,
5. expose diagnostics,
6. document the guarantee,
7. build higher-level functionality only afterwards.

Do not pull later-roadmap functionality forward merely because it is interesting.

## Code review priority

Review in this order:

1. correctness,
2. invariant preservation,
3. failure behavior,
4. test quality,
5. architectural boundaries,
6. observability,
7. maintainability,
8. performance,
9. elegance.

## Required review questions

For meaningful changes ask:

- What new behavior exists?
- What invariant is introduced or changed?
- What happens on partial failure?
- Is the operation idempotent?
- What state is canonical?
- What derived state can go stale?
- How is divergence detected?
- What is persisted?
- What happens across restart?
- Which provider assumptions leak into core code?
- How is this behavior proven?
- Which documentation claim becomes true because of this change?

## Failure transparency

Never accept:

- generic PASS with zero meaningful checks,
- swallowed exceptions,
- silent fallback to weaker behavior,
- mutation reported as successful before durability is known,
- reconciliation that only compares configuration files,
- test suites that skip the actual integration boundary while claiming integration coverage.

Failures should be typed or otherwise distinguishable where useful.

## Dependency review

Before adding a dependency evaluate:

- exact problem solved,
- whether it is core or incidental,
- stability and maintenance,
- API surface versus actual need,
- runtime infrastructure introduced,
- coupling or lock-in,
- replacement cost,
- whether a small local implementation would provide clearer ownership.

## GitHub workflow

Issues, roadmap items, commits, tests, docs, and releases form one traceable chain.

Before closing a work item:

- inspect current implementation,
- inspect relevant git history,
- locate implementing commits,
- verify acceptance criteria,
- verify behavioral tests,
- update documentation/state,
- only then close it.

Code presence alone is not completion.

## Decision discipline

Classify choices as reversible or difficult to reverse.

Prefer simple reversible choices while uncertainty is high.

If additional architecture has no demonstrated value, remove it.

If current implementation is sufficient, do not invent a subsystem.
