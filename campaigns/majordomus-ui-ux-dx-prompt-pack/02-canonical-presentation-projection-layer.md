# Milestone 2: Canonical Presentation / Projection Layer

Introduce or formalize a typed, reusable projection layer between Majordomus domain state and presentation surfaces.

## Core invariant

```text
domain truth
  -> canonical projection
  -> CLI/API/MCP/Cockpit/docs
```

The UI must not infer business semantics independently.

## Objectives

Create/reuse typed projections for the concepts the product actually needs, such as:

- overview/project summary
- navigation
- actions
- diagnostics
- status semantics
- detail views
- lists/tables
- timelines
- graphs
- evidence/provenance chains

Do not create abstractions without real consumers.

## Rules

Projection types may contain semantic presentation concepts such as:

```text
severity = warning
kind = intent
state = blocked
importance = primary
capability = mesh
```

They must NOT contain renderer-specific classes such as:

```text
bg-red-500
flowbite-badge-danger
cytoscape-selector
```

## Derivation

Projection state must derive from canonical typed domain data.

Where status is computed today in multiple consumers, move computation to one canonical owner.

Where the same relation is rebuilt by CLI, API and UI independently, centralize it.

## Stable identifiers

Every projected entity/action/relation intended for cross-surface use must have stable canonical identity.

Do not use display labels as IDs.

## Serialization

Ensure projections can be serialized where appropriate for:

- API
- structured CLI
- MCP
- tests

Do not create five incompatible projection schemas.

## Versioning

If the repository versions schemas/contracts, integrate projections into that versioning discipline.

## Testing

Add unit and contract tests proving:

- identical domain input produces identical projection
- projections are deterministic
- projections do not depend on filesystem/hash iteration order
- renderer-independent semantics remain stable
- malformed/incomplete domain data yields actionable diagnostics or explicit unknown states

## Migration

Migrate at least one real existing Cockpit surface and one non-web consumer onto the new projection to prove it is not frontend-only architecture.

## Acceptance criteria

- canonical projection layer exists or is formalized
- at least one duplicated domain semantic is removed
- at least two surfaces consume the same projection semantics
- renderer-specific details remain outside domain/projection types
- tests enforce determinism and cross-surface consistency
