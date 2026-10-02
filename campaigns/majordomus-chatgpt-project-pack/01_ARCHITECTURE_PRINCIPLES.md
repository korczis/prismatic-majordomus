# Majordomus Architecture Principles

## Mission

Majordomus is a standalone control and governance layer for AI-assisted software engineering workflows.

Its architectural value comes from making the following explicit and testable:

- policy,
- profiles and limits,
- execution constraints,
- durable state,
- projections,
- diagnostics,
- provider-specific capabilities,
- reconciliation,
- behavioral guarantees.

It should not become a universal agent framework or a second Prismatic.

## Core design rules

### Explicit invariants

Every major abstraction should answer:

- What invariant does this enforce?
- Which failure mode does it eliminate?
- How is the behavior observed?
- How is it tested?
- What depends on it?
- Can it be replaced without rewriting unrelated parts of the system?

### Policy versus execution

Keep policy definitions separate from mechanisms that execute or enforce them.

Policy should remain inspectable and testable without requiring a live provider whenever practical.

Runtime adapters may enforce policy, but they must not redefine it implicitly.

### Provider-neutral core

The domain model should use Majordomus-native concepts.

Provider-specific concepts belong in adapters.

Provider capability differences must be represented honestly. Do not simulate telemetry or runtime controls a provider does not expose.

### Durable truth versus projections

Clearly distinguish canonical durable state from derived projections.

Derived state should be reproducible or reconcilable from canonical truth.

Stale projections must be detectable.

### Reconciliation

Assume these may diverge:

- persisted policy,
- local configuration,
- runtime state,
- provider configuration,
- generated projections,
- external integration state.

Reconciliation should report both expected and observed state and make unresolved divergence visible.

### Operational simplicity

Prefer the smallest architecture that proves the invariant.

Avoid distributed components, queues, databases, agents, or services until their necessity is demonstrated by concrete constraints.

### Dependency discipline

Use mature dependencies for commodity problems.

Prefer local implementations for small domain-specific logic, policy semantics, critical guarantees, state models, and control-plane behavior whose semantics Majordomus must own completely.

## CLI principles

CLI commands should be:

- deterministic,
- composable,
- script-friendly,
- explicit about mutations,
- useful in CI,
- idempotent where the domain allows it.

Mutations should report what changed, what did not, why, resulting state, and unresolved divergence.

Machine-readable output should be available where automation benefits from it.

## Documentation

Technical documentation should be falsifiable.

Prefer:

- invariants,
- contracts,
- examples,
- state diagrams,
- failure modes,
- acceptance criteria,
- ADRs,
- guarantees matrices.

Mark functionality explicitly as implemented, experimental, planned, or unsupported.
