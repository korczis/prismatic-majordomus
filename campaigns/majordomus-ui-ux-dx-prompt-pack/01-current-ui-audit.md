# Milestone 1: Audit Existing UI, Data Ownership, and Parallel Truth

Perform a repository-wide audit of the current Cockpit/web UI and all adjacent presentation layers.

This is not a visual review. It is a **data ownership and architectural drift audit**.

## Goal

Produce a concrete map of what the UI currently renders, where each value originates, what is canonical, what is duplicated, and which client-side structures accidentally became independent sources of truth.

## Inspect

Audit at minimum:

- Cockpit routes/pages
- page templates
- frontend JS
- CSS/design system
- navigation
- breadcrumbs
- status badges
- tables
- cards
- dashboards
- graphs
- API calls
- server-side rendered data
- websocket/SSE/live-update paths if present
- command/action affordances
- filters/sorts
- docs/site overlap
- mobile behavior
- accessibility
- dark/light modes
- generated UI artifacts

Also inspect corresponding backend sources:

- domain models
- registries
- schemas
- API serializers
- MCP models
- CLI outputs
- diagnostics
- environment snapshots
- capability discovery
- intent/session/evidence/mesh structures

## Build an internal audit matrix

For every meaningful UI concept capture:

```text
concept
surface
current source
canonical source if different
frontend transformation
manual inventory?
duplicated elsewhere?
derivable?
status
migration recommendation
```

Examples:

- navigation item
- rule count
- intent status
- provider status
- active session
- completion state
- doctrine listing
- mesh peer
- graph edge
- issue relation
- evidence relation
- action/button availability

## Detect anti-patterns

Search for:

- hardcoded arrays of entities
- repeated labels/status mappings
- repeated Tailwind color mappings
- repeated route catalogs
- repeated icons by domain type
- frontend joins across independent API lists
- duplicated constants matching backend enums
- UI-only interpretations of domain status
- hidden assumptions about developer-only projects
- mobile/desktop duplicate templates
- raw API structures tightly coupled to pages
- ad-hoc fetches bypassing canonical API abstractions

## Required deliverable

Create or update an architecture/audit document in the repository according to existing documentation conventions.

The audit must distinguish:

1. canonical and healthy
2. legacy but migratable
3. architectural duplication
4. missing canonical projection
5. purely visual debt
6. inaccessible or non-responsive behavior

## No premature redesign

Do not start rearranging the UI until the ownership graph is understood.

You may fix obviously broken tests discovered during audit only if needed to make the audit machinery work; otherwise keep this milestone focused.

## Acceptance criteria

- every top-level UI area has a traced data source
- every manually duplicated inventory is identified
- every frontend-owned domain semantic is identified
- current stack and build pipeline are documented
- migration order is explicit
- follow-up milestones can be implemented without rediscovering the whole architecture
