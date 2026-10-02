# Phase 3 — Brownfield Discovery, Existing Knowledge Reuse, and Bootstrap

Use the master contract and completed domain model.

Implement a brownfield-safe repository discovery and bootstrap pipeline.

## Product requirement

Running the RKS bootstrap against an existing repository must produce a useful initial knowledge state without requiring the repository to be reorganized and without duplicating existing documentation.

The operation must be deterministic-first, repeatable and idempotent.

## Discovery pipeline

Implement or extend a generic extraction/discovery mechanism that can produce typed `Evidence` and candidate knowledge structures.

Start with high-value deterministic sources that match the current Majordomus repo and can be tested reliably.

At minimum, where present, discover:

### Repository / git

- repository root
- branch/worktree identity
- HEAD revision
- tracked/untracked status only when relevant
- file inventory filtered by repository ignore policy

### Workspace/package structure

For the languages/tools actually present in Majordomus, extract package/workspace membership, binaries/libraries/services and dependency relationships using native metadata APIs where practical.

### Existing documentation

Discover and classify:

- README files
- docs directories
- ADR/RFC directories
- architecture docs
- runbooks
- contributing docs
- schema/frontmatter-bearing markdown

Register these as existing knowledge sources with `external` ownership by default unless repository metadata explicitly marks Majordomus ownership.

Do not clone content into `.ai/knowledge` merely because it exists elsewhere.

### APIs / schemas / manifests

Where structured sources already exist, register them as high-confidence evidence.

### CI / deployment

Discover meaningful workflows, deployment manifests and operational entrypoints where deterministic extraction is straightforward.

## Extractor architecture

Implement extractors in a generic way that allows later additions without manual multi-surface registration.

Each extractor should expose metadata such as:

- stable ID
- version
- supported evidence kinds
- supported file patterns or capability predicates
- whether it is deterministic or semantic/LLM-assisted
- whether it may inspect potentially sensitive content

Prefer existing Majordomus capability/plugin mechanisms.

## Bootstrap behavior

Add a bootstrap operation that:

1. computes repository inventory,
2. runs relevant deterministic extractors,
3. builds/updates the evidence registry,
4. creates high-confidence observed/declared nodes/claims,
5. registers existing documents without duplication,
6. records gaps where the system cannot infer enough,
7. creates a baseline suitable for brownfield enforcement,
8. persists only the minimum necessary durable state,
9. leaves caches outside tracked canonical data unless repository doctrine says otherwise.

## Idempotence

Without repository changes, repeated bootstrap must produce the same canonical result modulo explicitly non-semantic timestamps.

Add tests proving this.

## Ownership semantics

Ensure:

- external documents are indexed and linked, not overwritten
- Majordomus-generated projections are marked as Majordomus-owned
- hybrid ownership is supported for structured metadata with human-authored content

## Baseline

Create a first baseline model capturing at least:

- current known gaps/debt
- conflicts
- stale/unverified counts if applicable
- coverage snapshot if denominator definitions exist
- repository revision/schema version

Default adoption mode should be non-destructive, likely `observe` unless repo doctrine specifies otherwise.

## Coverage / gap detection

Do not invent vanity percentages. Coverage must only be computed where the denominator is deterministically defined.

Prefer reports like:

```text
components discovered: 24
components with knowledge: 21
missing: 3
```

rather than unsupported “87% knowledge completeness.”

## Fixtures

Create realistic brownfield fixtures, ideally extending existing repo test infrastructure:

### documented legacy fixture

Contains:

- existing README/docs
- manifest/workspace structure
- some undocumented components

Expected behavior:

- docs reused as external sources
- no duplicates generated
- gaps recorded

### conflicting docs fixture

Contains a declared statement contradicted by deterministic code/config evidence.

Do not fully resolve conflicts yet if Phase 4 owns that, but preserve enough evidence for later detection.

### idempotence fixture

Run bootstrap twice and assert stable canonical state.

## Developer UX

Expose enough internal/debug output for developers to understand what was discovered, even if final polished CLI comes in Phase 5.

## Documentation

Add brownfield adoption docs covering:

- what is discovered
- what is not modified
- ownership semantics
- baseline meaning
- idempotence
- how existing docs are treated

## Acceptance criteria

- Existing repos can be scanned without restructuring.
- Existing docs are registered, not duplicated.
- Deterministic evidence is captured with fingerprints.
- Baseline is created.
- Bootstrap is idempotent.
- Extractors are extensible/discoverable.
- Brownfield fixtures prove behavior.
- No network/LLM is required for the deterministic bootstrap path.
