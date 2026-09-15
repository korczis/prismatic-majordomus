# Prompt 06 — Full Cross-Surface Integration

Now ensure imported skills/doctrines are first-class across Majordomus.

Do not invent endpoints or UI merely to tick boxes. Integrate where the repository already exposes comparable canonical entities.

## CLI

Relevant commands should derive from the canonical registry.

Support existing patterns for:

- list;
- show/explain;
- validate;
- related entities;
- provenance;
- machine-readable JSON.

No manually maintained list of imported entities.

## API

Expose canonical entities through existing generic APIs/routes where appropriate.

Use typed response models.

Default ordering must be deterministic.

Relationships and provenance should be available if existing API conventions permit them.

## OpenAPI / Swagger

Derive schemas and operation documentation from the same API models/routes.

No manually duplicated Swagger catalog.

Validate OpenAPI generation and Swagger usability.

## MCP

Expose relevant skill/doctrine discovery and inspection through the existing MCP model.

Do not create an independent MCP registry.

If skills can be invoked or resolved by agents, ensure imported skills participate through canonical discovery.

## Cockpit

Add or extend views to make imported entities useful:

- browse/search/filter;
- inspect details;
- see doctrine ↔ rule ↔ skill relationships;
- provenance/source history;
- validation status;
- affected surfaces/capabilities where already modeled.

Frontend must consume canonical backend data. No duplicated JavaScript arrays of doctrine/skill names.

Respect existing design system, responsive behavior and accessibility.

## Docs / GitHub Pages

Generate indexes and cross-links from canonical metadata.

Preserve hand-written explanatory prose.

Do not hand-copy entity inventories into docs.

Ensure imported/adapted concepts are discoverable and linked to:

- related rules;
- skills;
- schemas;
- ADRs/knowledge;
- commands/API where relevant.

## Completion/tooling

If CLI completion or runtime completion is generated, ensure new applicable commands/entities are derived from canonical metadata.

## Cross-surface contract

Add tests proving a representative canonical skill/doctrine has consistent identity and metadata across relevant surfaces.

The representation may differ. The source of truth must not.
