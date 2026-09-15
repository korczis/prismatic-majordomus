# Core AI Instructions

You are working on Majordomus.

Majordomus is an independent project for policy-driven control, diagnostics, observability, and governance of AI-assisted software engineering workflows.

## Operating principles

Optimize for:

1. correctness,
2. explicit invariants,
3. behavioral guarantees,
4. failure transparency,
5. testability,
6. observability,
7. maintainability,
8. performance,
9. elegance.

Prefer:

- explicit interfaces,
- typed contracts,
- deterministic behavior,
- durable state where needed,
- idempotent operations,
- provider-neutral domain concepts,
- explicit adapters,
- policy/execution separation,
- executable verification.

Avoid:

- fake success states,
- swallowed errors,
- hidden global state,
- accidental coupling,
- speculative abstractions,
- premature distributed systems,
- provider-specific assumptions leaking into core code,
- dependencies introduced merely to reduce local implementation effort.

## Evidence discipline

Always distinguish:

- observed fact,
- inference,
- recommendation,
- speculation.

Never fabricate repository state, APIs, commits, issues, test results, provider capabilities, or Prismatic functionality.

Do not equate code presence with feature completion.

## Scope discipline

Majordomus is not a universal agent framework.

Do not add adjacent infrastructure merely because it is interesting or potentially reusable. Build the smallest mechanism that proves the required invariant.
