# Prompt 04: Provider Routing, Fallback and Governance

Use MAXIMUM AVAILABLE CONTEXT and current hydrated session/handover state.

Implement/reconcile the canonical provider/model routing engine.

Routing must consume typed task requirements derived after governance/context hydration. It must be deterministic, explainable and policy-driven.

Support the actual scopes in the repository and define/test explicit precedence among relevant levels such as:

- explicit invocation override
- task
- peer assignment
- workflow
- session
- profile
- repository
- system/default
- automatic routing

Do not blindly copy this order. Infer canonical semantics from existing architecture, then document and enforce it.

Routing should reason about actual modeled capabilities/constraints, such as:

- tools
- structured output
- context needs
- reasoning/task class
- local-only/privacy
- online/offline
- availability
- latency/cost preference if canonical data exists
- workflow role
- peer role

Avoid brand-based conditionals when capability semantics suffice.

Implement typed failure classification and distinguish retry from fallback.

Fallback must never silently violate:

- provider pin
- model pin
- local-only/privacy constraints
- required capability
- governance allow/deny policies
- cost/other explicit policy ceilings

Detect policy conflicts before invocation.

Implement explainability: for every resolved selection expose selected provider/model, source/override chain, capabilities/constraints, eligible candidates, fallback chain and final reason, using existing diagnostics/explain infrastructure.

Integrate provider diversity for review/opposition only if existing Majordomus workflow/governance concepts support it; make it configurable and testable, not hardcoded Claude-vs-Codex folklore.

Add deterministic tests that randomize registry/discovery/insertion order and concurrent probe completion.

Add failure matrix tests for missing credentials, unavailable provider, model disappearance, unsupported capability, rate limit, timeout, offline/local-only, all providers unavailable, explicit pin conflicts and fallback cycles.

Ensure all routing events update canonical session provenance without leaking secrets.
