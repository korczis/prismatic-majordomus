# Architecture Principles

## Invariants first

Every major abstraction should answer:

- What invariant does it enforce?
- What failure mode does it eliminate?
- How is the behavior observed?
- How is it tested?
- What depends on it?
- Can it be replaced without rewriting unrelated components?

## Policy versus execution

Keep policy definitions separate from mechanisms that execute or enforce them.

Runtime adapters may enforce policy but must not silently redefine it.

## Provider-neutral core

Core Majordomus concepts must not be shaped around OpenAI, Anthropic, Claude Code, Codex, Gemini, Antigravity, GitHub, or any single agent framework.

Provider-specific functionality belongs behind explicit adapters.

If telemetry, limits, runtime control, or execution metadata are unavailable from a provider, represent them as unavailable. Estimates must be labeled explicitly.

## Canonical state versus projections

Clearly distinguish canonical durable state from derived projections.

Derived state should be reproducible or explicitly reconcilable from canonical truth. Stale projections must be detectable.

## Reconciliation

Assume these may diverge:

- persisted policy,
- local configuration,
- runtime state,
- provider configuration,
- generated projections,
- external integration state.

Reconciliation should report expected state, observed state, and unresolved divergence.

## Dependency discipline

Use mature dependencies for commodity problems.

Prefer local implementations for small domain-specific logic, critical guarantees, policy semantics, state models, and control-plane behavior whose semantics Majordomus must own.
