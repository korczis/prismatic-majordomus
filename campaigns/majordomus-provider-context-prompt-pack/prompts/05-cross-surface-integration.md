# Prompt 05: Full Cross-Surface Integration

Use MAXIMUM AVAILABLE CONTEXT. Verify all prior architectural work against the current repository before integrating surfaces.

Integrate the canonical provider/session/handover/context architecture everywhere. No consumer-specific shadow state.

## CLI

Use existing command conventions to expose provider/model/session/handover operations including list/show/current/select/reset/probe/doctor/explain/models and session/handover status where appropriate. Structured JSON must derive from canonical types.

Explicit switching must have clear persistence scope. One-off invocation override must not accidentally persist.

## API

Expose canonical provider/model/effective-selection/routing-explain/session/handover/context-freshness data and safe mutations using existing API service patterns.

## OpenAPI/Swagger

Generate descriptions and schemas from canonical API/types. No handwritten duplicate provider/session models.

## MCP

Expose provider/session/handover/context capabilities through existing MCP conventions. MCP-triggered repository work must use the same canonical execution/session pipeline, not a separate router.

## Cockpit

Make provider management and continuity first-class, routable, backend-backed features.

The UI should inspect/manage, as appropriate:

- providers and provider instances
- models
- effective selection + scope
- routing explanation
- fallback state
- health/credentials-safe state
- current session/task/issue/milestone
- session lineage
- handover state
- context freshness and sources
- provider switch history
- peer transfer state
- validation diagnostics

A provider/model switch UI must clearly state whether it affects invocation/task/session/profile/repository according to supported scopes.

No frontend hardcoded provider/model lists. No frontend routing logic. No Markdown handover parsing in browser code.

## RepositoryEnvironment/banner

Expose concise safe provider/session status from cheap cached canonical state. No network calls during direnv/repo entry.

## Completion

Derive provider/model/session identifiers from canonical registries for shell/runtime completion where supported.

## Workflows/profiles/skills/commands

Make them consume canonical capability requirements and routing policies. Avoid unnecessary vendor pins.

## Peers/mesh

Expose execution capabilities/location sufficiently for task assignment without conflating provider identity and peer identity.

Add cross-surface contract tests proving semantic parity among domain registry, CLI JSON, API, MCP, Cockpit payload and RepositoryEnvironment.

Remove obsolete surface-specific lists/state after migration.
