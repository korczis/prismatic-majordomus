# Phase 2 — Canonical Domain Model, Schemas, and Registry Integration

Use `00_MASTER_CONTRACT.md` and the Phase 1 architecture/ADR as binding context.

Implement the canonical RKS domain model and schema/versioning layer in the repository’s established style.

## Primary goal

Create one typed, versioned, machine-readable model from which all later RKS behavior and projections can be derived.

Do not build UI or LLM enrichment yet. This phase is about correctness, semantics, invariants, schemas, serialization and discoverability.

## Required model capabilities

Represent equivalents of:

### Repository identity / revision

- stable repository identity
- git revision/branch/worktree identity where applicable
- schema version
- generation/reconciliation metadata

### Knowledge node

A node must support:

- stable ID
- kind
- title/name
- claims
- evidence refs
- relations
- provenance
- confidence
- freshness
- ownership
- visibility/security metadata if consistent with repo patterns
- timestamps/revision metadata only where semantically meaningful

### Claims

A claim should represent a specific proposition, not merely an unstructured paragraph. Support:

- stable identity where useful
- typed key/category or extensible subject/predicate/value representation
- provenance
- evidence refs
- confidence
- state/conflict metadata

Avoid designing a philosophical RDF clone unless the repository actually benefits. Keep the model practical for repository reasoning and serialization.

### Evidence

Support typed evidence and fingerprints. Core types should accommodate at least:

- file
- structured document/frontmatter
- symbol or code location if available
- manifest/config entry
- schema/API entry
- git commit/change
- existing documentation
- deployment/CI metadata
- runtime metadata if later needed

Each evidence record should support:

- identity
- source locator
- fingerprint
- extractor/source metadata
- visibility / remote-processing policy if needed

### Relations

Support typed/extensible relationships such as:

- depends_on
- implements
- exposes
- consumes
- persists_to
- described_by
- decided_by
- related_to
- supersedes
- contradicts
- derived_from

Do not hardcode every future relation into UI-specific logic.

### Provenance

Required classes:

- observed
- declared
- derived
- curated

Make the type machine-readable and ergonomic in serialization.

### Confidence

Implement confidence as an explicit semantic type. Avoid arbitrary unexplained model-generated numbers.

Support structured basis/reason metadata if appropriate, with validation constraints.

### Freshness

Support states equivalent to:

- current
- possibly_stale
- stale
- conflicted
- unverified

Define transition semantics in code/docs/tests where possible.

### Ownership

Support:

- external
- majordomus
- hybrid

Ownership must later control whether reconciliation may auto-update, propose, or only report.

### Knowledge kinds

Provide a typed and extensible approach for core kinds. Ensure plugin-defined kinds can be admitted under schema governance without multiplying switch statements across CLI/API/UI.

### Conflict / impact / gap / coverage / baseline

Define the domain structures needed later for:

- conflicts
- impact results
- coverage summaries
- missing knowledge/gaps
- baseline policy comparison

Do not implement full algorithms yet unless required for coherent type behavior.

## Schema requirements

Generate JSON Schema from the canonical Rust types using the repository’s preferred schema mechanism. Avoid manually authored duplicate schemas.

Schemas must be:

- versioned
- testable
- addressable/discoverable by existing schema registry tooling
- documented by generated metadata where available

If the repo already exposes schemas through CLI/API/docs, integrate these new schemas automatically.

## Serialization requirements

Ensure stable, human-comprehensible JSON/YAML/Markdown-frontmatter representations where these are supported.

Prefer explicit tagged enums or conventions that remain forward-migratable.

## Invariants to enforce in code/tests

Examples:

- a derived claim cannot claim `current` without evidence unless explicitly configured as curated/manual
- evidence fingerprints are non-empty for deterministic sources
- unknown custom kind/relation identifiers are namespaced or otherwise validated
- ownership values have deterministic update semantics
- schema version is always serialized
- IDs are normalized and safe for CLI/API usage

Do not overconstrain legitimate future evolution.

## Registry integration

Wire the model into the existing capability/schema registries. A future extractor, CLI route or Cockpit view should be able to discover RKS types without adding another handwritten registry.

## Documentation

Document the domain model in the repo’s canonical docs/knowledge system, generated where possible from code/schema metadata.

Include a concise conceptual document explaining:

```text
Evidence → Claim → KnowledgeNode → Relations
```

and the difference between provenance, confidence, freshness and ownership.

## Tests

Required:

- round-trip serialization tests
- JSON Schema generation tests
- invariant validation tests
- stable fixture/golden examples
- custom kind/relation extensibility tests
- schema versioning smoke tests

## Acceptance criteria

- Canonical typed model exists.
- JSON Schema is generated from the same source.
- No manual duplicate schema/registry is required.
- Types are extensible and migration-aware.
- Core invariants are tested.
- Documentation is integrated.
- Existing checks remain green.
