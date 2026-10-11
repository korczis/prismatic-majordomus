# Master Orchestrator: Majordomus Browser UX as a Canonical Derived Surface

You are working inside the `prismatic-majordomus` repository.

Your mission is to turn the existing Majordomus web/Cockpit experience into a coherent, user-friendly, browser-first UI/UX/DX layer while preserving the project's core architectural doctrine: **one canonical truth, derived everywhere, enforced mechanically**.

Do NOT treat this as a frontend redesign. Treat it as a repository-wide presentation architecture correction and productization effort.

## Required end state

Majordomus must expose the same canonical state through:

- CLI
- structured JSON
- REST/API
- OpenAPI/Swagger
- MCP
- Cockpit/web UI
- generated documentation
- graph/visualization surfaces

The UI stack should use, where appropriate:

- Tailwind CSS
- Flowbite
- Alpine.js
- Cytoscape.js
- p5.js only for genuinely visualization-centric cases

But these libraries are renderers/interactions, not domain model owners.

## Hard invariant

```text
canonical domain state
    -> typed canonical projections
    -> renderer-specific presentation
```

Never:

```text
backend state
    + frontend reimplementation
    + duplicated JS inventories
    + handwritten navigation
    + independent status mappings
```

## Repository-first discipline

Before making changes, deeply inspect:

- `AGENTS.md` and equivalents
- `.ai/**`
- `.majordomus/**`
- rules/doctrines
- ADRs
- schemas
- Cockpit/web implementation
- API/OpenAPI
- MCP
- docs/site generation
- CLI/environment/introspection layers
- current tests and E2E tooling
- route registries
- capability registries
- action/command registries
- existing design or CSS conventions
- deployment and Pages/runtime behavior

Do not assume filenames or frameworks. Discover them.

## Architectural target

The system should converge on a reusable presentation model such as:

```text
Domain / Registry / Snapshot
  -> Projection layer
     - SummaryProjection
     - NavigationProjection
     - ActionProjection
     - DetailProjection
     - GraphProjection
     - DiagnosticProjection
     - TimelineProjection
  -> Surfaces
     - CLI
     - API
     - MCP
     - Cockpit
     - docs
```

Exact names should follow repository conventions.

## Product UX target

The browser experience should primarily answer:

- What are we doing now?
- Why are we doing it?
- What is blocked?
- What is done?
- What evidence proves it?
- What changed?
- What should happen next?
- Which agents/people/sessions are involved?
- Why is the system showing this status?

The browser must not merely expose raw internal entities in a giant admin UI.

## Information architecture target

Prefer a compact top-level model similar to:

```text
Overview
Work
System
Knowledge
Explore
```

But infer the actual categories from canonical capabilities and current product architecture. Do not hardcode this taxonomy if the repository already has better semantics.

## Progressive disclosure

Majordomus should be usable by both:

- developers/operators who need branches, commits, providers, schemas, runtime, MCP, APIs
- non-programmer knowledge workers who need goals, tasks, decisions, evidence, sources, progress, blockers and outcomes

Do not build separate products. Build persona/profile projections over the same canonical model.

## Client-state rule

Alpine.js or equivalent may own:

- modal open state
- active local tab
- local filter text
- temporary selection
- graph viewport
- disclosure state

It MUST NOT own:

- intent status
- issue relationships
- session truth
- completion truth
- durable evidence state
- capability availability
- canonical navigation inventory

## Flowbite policy

If Flowbite provides a suitable primitive, prefer it over inventing a custom component. Custom components require justification when an equivalent canonical primitive already exists.

## Tailwind policy

Use semantic presentation tokens and a coherent design system. Avoid arbitrary class soup and avoid embedding Tailwind classes into backend domain types.

## Graph policy

Cytoscape receives canonical graph projections, not raw lists that frontend JS heuristically joins together.

## p5.js policy

Only use p5.js where dynamic/exploratory visualization adds clear value. Do not use it merely to make dashboards look animated.

## Mechanical enforcement

At completion, the repository must detect or prevent:

- duplicate navigation inventories
- duplicate action inventories
- duplicate status/presentation mappings
- frontend-only domain semantics
- hardcoded lists that should derive from canonical registries
- raw UI state that conflicts with backend projections
- generated artifact drift
- missing projection coverage for new canonical entities where required

## Execution strategy

Execute the prompt pack milestones in order unless current repository architecture proves a different dependency order is necessary.

For every milestone:

1. inspect actual current state
2. identify the architectural source of truth
3. design the smallest canonical extension
4. implement
5. migrate legacy behavior
6. delete superseded duplication
7. test
8. document
9. enforce
10. run repository-level validation

Do not stop at analysis if implementation is possible.

Do not ask me obvious implementation-detail questions. Infer from repository conventions.

## Completion evidence

At the end, report:

- architecture before/after
- canonical sources used
- duplicate state removed
- UI surfaces migrated
- tests/gates added
- exact validation commands and results
- deploy/verification evidence if applicable
- remaining debt only where genuinely blocked
