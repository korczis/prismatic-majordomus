# 05 — MCP, API, OpenAPI and Cockpit as Derived Surfaces

Now wire surfaces to the canonical domain model and delete surface-specific truth.

## MCP

Audit current server lifecycle and capability registration.

Ensure:

- MCP is started/discovered through control-plane desired state,
- tools/resources/prompts derive from canonical registries,
- runtime/session/peer/claim/handover/diagnostic state can be inspected appropriately,
- safe mutations reuse domain services used by CLI/API,
- MCP transport health participates in control-plane readiness/diagnostics.

## REST API

Expose/reconcile canonical operations and snapshots through existing API patterns.

Do not create duplicate domain implementations in handlers.

## OpenAPI/Swagger

OpenAPI must derive from actual routes/types. Swagger must not carry a hand-maintained copy of behavior.

Validate generated docs/schema drift.

## Cockpit

Convert Cockpit into a live consumer of canonical state.

At minimum provide useful views for:

- services + health,
- endpoints,
- sessions,
- agents/providers,
- peers/presence,
- claims/work ownership,
- handovers/context references,
- diagnostics,
- MCP capability surface where useful.

Live updates should use the canonical event model.

No frontend hardcoded service/capability arrays.

## Cross-surface consistency

Add contract tests proving representative state is the same through:

```text
domain snapshot
CLI structured output
REST
MCP
Cockpit backend payload
```

OpenAPI should describe the REST schema rather than becoming another data source.

## Zero-registration proof

Add one representative discoverable capability/entity and prove it propagates to all applicable surfaces without manually adding it to each consumer.
