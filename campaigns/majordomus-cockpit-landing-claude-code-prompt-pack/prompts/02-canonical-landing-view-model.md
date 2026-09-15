# PROMPT 02 — Canonical Landing View Model

## Mission

Create or reuse a typed canonical projection for the Cockpit landing page so LiveView does not independently assemble project reality.

Adapt naming to repository conventions.

Conceptually the projection may contain equivalents of:

```text
RepositorySummary
ResumeTarget[]
AttentionItem[]
MilestoneSummary[]
ReadyTask[]
ActiveSession[]
ActivePeer[]
RuntimeHealth[]
RecentEvent[]
CompletionGateSummary[]
SuggestedAction[]
```

Do not blindly create these exact structs if suitable types already exist.

## Requirements

The landing projection must:

- compose canonical domain/runtime data
- preserve stable IDs
- preserve provenance/reference to canonical objects
- define deterministic ordering
- avoid frontend-only joins
- avoid repeated expensive queries
- be serializable if useful cross-surface
- be testable independently of rendering

## Derived, not duplicated

Do not introduce another canonical database/table merely for landing.

The landing projection should be a view/projection over existing sources.

## Deterministic ordering

Define explicit canonical ordering for:

- attention items
- resume targets
- milestones
- ready tasks
- peers
- runtime checks
- events

Never rely on filesystem/map/query incidental order.

## Empty-state behavior

The projection must distinguish:

- no data because nothing exists
- unavailable source
- source failed
- source disabled
- zero relevant items

Cockpit should not display fabricated zeros when state is unknown.

## Caching

If needed, use repository-standard cache/projection infrastructure.

No network access should be introduced merely to render the local landing page unless current architecture explicitly supports asynchronous external status enrichment.

## Cross-surface readiness

Where useful, make the projection or underlying canonical submodels available through existing JSON/API mechanisms rather than Cockpit-specific private structures.

## Tests

Test:

- empty repository
- normal active repository
- missing optional subsystem
- deterministic order
- failures/degraded sources
- large datasets
- stale data markers if supported

## Acceptance

Landing LiveView can render useful content from one coherent typed projection rather than orchestrating domain logic itself.
