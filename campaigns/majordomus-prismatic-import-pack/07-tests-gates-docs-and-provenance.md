# Prompt 07 — Hardening: Tests, Gates, Docs, Provenance, Drift Prevention

Treat this as production hardening.

## Test layers

Implement or extend tests for:

### Schema
- valid skill/doctrine;
- invalid required fields;
- invalid IDs;
- broken relationships;
- incompatible schema versions;
- provenance shape.

### Discovery
- recursive discovery if supported;
- deterministic ordering;
- duplicates rejected;
- unrelated files ignored correctly;
- new entity auto-discovered.

### Migration
- representative legacy forms normalize correctly;
- obsolete form rejected after migration where intended.

### Relationships
- doctrine ↔ rule;
- skill ↔ doctrine;
- schema ↔ entity;
- provenance ↔ imported entity;
- dangling reference diagnostics.

### Cross-surface
- CLI structured output;
- API;
- MCP;
- Cockpit data model or integration endpoint;
- generated docs/index.

### Zero-registration
Add a fixture entity in the canonical discovery location and prove consumers see it without editing their registries.

### Negative regression
Introduce fixtures representing the exact duplication/legacy errors this migration eliminates and assert canonical validation fails.

## Gates

Integrate into the existing quality pipeline.

Do not create redundant workflows if existing `check`, CI, pre-push or validation commands should own the checks.

Validation should cover:

- schema;
- canonical IDs;
- duplicate semantic entities;
- relationship integrity;
- generated drift;
- docs/index drift;
- schema/OpenAPI drift;
- forbidden manually maintained duplicate inventories where detectable.

## Documentation

Update canonical docs with:

- architecture;
- authoring guide for skills;
- authoring guide for doctrines/rules;
- provenance/import semantics;
- how discovery works;
- how to validate;
- how surfaces are generated;
- troubleshooting;
- extension example;
- migration notes.

## Provenance audit

Verify every donor-derived canonical entity records sufficient origin information.

Do not copy donor source commit hashes by guesswork. Obtain them from Git.

Do not make provenance dependent on the source checkout at runtime.

## Security

Search for accidental imports of:

- `.env*`;
- tokens;
- credentials;
- private URLs;
- machine-local absolute paths;
- user-specific state;
- caches;
- conversation dumps not meant for source control.

Fail the migration if any such material was accidentally copied.
