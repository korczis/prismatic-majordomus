# Phase 6 — Cockpit UI/UX, Knowledge Graph, and Developer Experience

Use the master contract and existing RKS interfaces. Build a polished RKS experience inside the existing Majordomus Cockpit without introducing a second frontend architecture.

## First step

Inspect current Cockpit patterns, Flowbite/Alpine/Cytoscape usage, route registration, server-rendered/static assets, component conventions and responsive design rules. Reuse them.

## Product UX principle

The UI is not a document browser. It is a repository knowledge health and explainability interface.

## Information architecture

Add RKS navigation derived from existing capability/route metadata wherever possible.

Target sections:

```text
Overview
Explore
Graph
Changes / Impact
Coverage
Gaps
Conflicts
Sources
Schemas
Diagnostics
```

Avoid manual duplication in top nav/side nav if Majordomus already has dynamic route/menu discovery.

## Overview

Create a high-signal knowledge health view containing:

- current / stale / possibly stale / conflicted / unverified counts
- baseline mode and delta
- coverage summaries only where denominator is well-defined
- recent repository changes affecting knowledge
- actionable next steps

Visual design should be compact, responsive and information-dense without dashboard cosplay.

## Explore

Allow browsing/filtering by:

- kind
- provenance
- freshness
- ownership
- visibility
- extractor/source

Provide search driven by the same backend query model as CLI/API/MCP.

## Node detail

For every node/claim show clearly:

- title/kind
- provenance
- confidence and basis
- freshness
- ownership
- evidence
- relationships
- dependents
- source revisions/fingerprints where useful

Include a prominent “Why?” / evidence explanation interaction.

Do not hide uncertainty behind generic AI sparkle indicators.

## Graph

Use the existing graph visualization library if available, likely Cytoscape based on current project direction.

Graph must support:

- node kinds as semantic categories
- filtering
- focus neighborhood
- search-to-node
- relationship labels/tooltips
- drilldown to detail
- optional overlays for stale/conflicted/affected nodes

Keep graph payloads bounded for large repos. Provide server-side filtering or neighborhood queries instead of rendering the entire universe by default.

## Impact view

Visualize a current worktree or revision-range impact:

```text
changed evidence
  ↓
affected claims
  ↓
affected knowledge
```

Clearly distinguish definite vs possible impact.

Provide links back to code/source references if the Cockpit already has safe file navigation patterns.

## Conflict view

Each conflict should show:

- disputed claim
- supporting evidence
- contradictory evidence
- current status
- recommended resolution actions if backend exposes them

Do not auto-resolve from UI without explicit user action and policy support.

## Brownfield UX

For newly adopted repos, show:

- baseline status
- inherited debt
- new debt delta
- adoption maturity mode: observe/warn/protect/strict

Make it obvious that legacy debt is recognized rather than immediately blocking development.

## Actions

Where safe and supported, Cockpit actions should call the canonical API:

- rescan
- reconcile
- validate
- accept/update baseline if policy permits

Do not implement browser-only business logic.

## Live updates

If Majordomus already has WebSocket/SSE infrastructure, stream state changes for scans/reconciliation/agent work rather than polling aggressively. Reuse existing runtime coordination.

## Responsive and accessibility requirements

- mobile-first layout
- keyboard navigable controls
- semantic labels
- no information conveyed by color alone
- sensible empty/loading/error states
- robust rendering for hundreds/thousands of nodes

## UI generated from schema/metadata

Where useful, use RKS schema/capability metadata to derive labels, filters or diagnostics. Do not hand-maintain another taxonomy in JavaScript.

## Tests

Add the strongest current UI test level supported by repo infrastructure:

- backend route tests
- rendered HTML/component tests
- browser E2E if existing harness exists
- graph/filter state tests where practical

Include a fixture with stale and conflicted nodes so the UI is tested against non-happy paths.

## Documentation

Document Cockpit usage with screenshots only if the current docs pipeline supports deterministic generation; otherwise document routes and behavior without brittle manual screenshot maintenance.

## Acceptance criteria

- RKS feels native to Cockpit.
- Evidence/provenance/freshness are first-class UI concepts.
- Graph is bounded and useful, not decorative.
- Actions delegate to API/backend.
- Navigation and labels do not create duplicate registries.
- Mobile/accessibility basics are respected.
- Non-happy states are tested.
