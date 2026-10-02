# Shared Context for All Prompts

You are modifying `~/dev/prismatic-majordomus`.

You may inspect `~/dev/prismatic-platform` as a source repository.

Treat `prismatic-platform` as an architectural donor, not as an authority over Majordomus.

## Core requirements

All imported/adapted functionality must converge on Majordomus principles:

- canonical source of truth;
- discovery over registration;
- derivation over synchronization;
- generated projections over copied inventories;
- typed Rust/domain models where runtime behavior exists;
- schemas for machine-readable declarative artifacts;
- deterministic behavior;
- explicit provenance;
- no invisible magic that cannot be diagnosed;
- no secrets in generated or committed artifacts;
- no shell scripts carrying domain logic if that logic belongs in `apps/majordomus-cli` or canonical application code;
- no frontend copy of backend taxonomies;
- no docs-only rules that enforcement never executes.

## Inspect before assuming

Discover actual paths and conventions. Do not assume either repository uses a path merely because this prompt names a conceptual category.

Inspect at minimum:

- `AGENTS.md`
- `.ai/**`
- `.majordomus/**`
- skills
- doctrines
- rules
- schemas
- ADRs
- session/handover systems
- knowledge base
- generated indexes
- CLI
- API/router
- OpenAPI
- MCP
- Cockpit
- GitHub Pages / site generator
- validation/gating
- tests
- git hooks
- CI
- completion
- environment/bootstrap
- developer workflow tooling.

## Import classes

Classify donor material into:

- `IMPORT_AS_IS` — rare; semantically generic and structurally compatible.
- `ADAPT` — reusable concept, but names/schema/runtime integration need Majordomus-native form.
- `REIMPLEMENT` — intent is valuable but source implementation is coupled to Prismatic Platform.
- `MERGE` — Majordomus already has equivalent functionality; consolidate into the stronger canonical implementation.
- `REFERENCE_ONLY` — useful documentation/idea, but importing it would add duplication or unnecessary machinery.
- `REJECT` — source-specific, obsolete, unsafe, redundant, or contrary to Majordomus architecture.

No item is imported without one of these classifications.

## Evidence record

For every accepted donor item maintain machine-readable provenance conceptually containing:

- source repository;
- source path;
- source commit SHA;
- source item kind;
- source canonical ID if any;
- decision;
- target canonical ID/path;
- adaptation notes;
- migration notes;
- imported/adapted timestamp only if repository convention permits timestamps;
- compatibility/schema version where applicable.

Prefer generating this provenance from canonical imported metadata. Do not create a second inventory that must be maintained by hand.
