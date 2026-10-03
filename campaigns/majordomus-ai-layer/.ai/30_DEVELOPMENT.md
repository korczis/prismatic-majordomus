# Development and Review

## Preferred implementation sequence

1. establish the invariant,
2. define observable behavior,
3. implement the smallest viable mechanism,
4. add behavioral tests,
5. expose diagnostics,
6. document the guarantee,
7. build higher-level functionality only afterwards.

Each roadmap stage is gated by the previous stage being demonstrably real.

## Review priority

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

## Review questions

For meaningful changes ask:

- What new behavior exists?
- What invariant changes?
- What happens on partial failure?
- Is the operation idempotent?
- What state is canonical?
- What derived state can go stale?
- How is divergence detected?
- What persists across restart?
- Which provider assumptions leak into core code?
- What test proves the new behavior?
- Which documentation claim becomes true because of the change?

## Failure transparency

Reject:

- generic PASS without meaningful checks,
- swallowed exceptions,
- silent fallback to weaker semantics,
- success reported before durability is known,
- reconciliation that only compares config files,
- mocks hiding the integration boundary being claimed as tested.

## GitHub evidence chain

Treat work as:

requirement -> implementation -> commit -> behavioral test -> documentation -> closed issue / released guarantee

Before closing a work item, verify acceptance criteria and executable evidence.
