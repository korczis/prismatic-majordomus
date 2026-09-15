# Prompt 02 — Canonical Import Architecture and Provenance Model

Read `00-CONTEXT.md` and the audit produced by Prompt 01.

Now design and implement the canonical architecture that makes imported skills/doctrines first-class Majordomus entities rather than copied Markdown.

## Mission

Establish or extend the canonical model for:

- skills;
- doctrines;
- rules;
- schemas;
- provenance;
- discovery;
- validation;
- relationships/dependencies;
- generated projections.

Reuse existing Majordomus architecture wherever possible.

## Hard invariant

The end-state pipeline should conceptually be:

```text
canonical declarative/source artifacts
        ↓
schema validation
        ↓
typed discovery
        ↓
canonical registry
        ↓
relationship/provenance resolution
        ↓
validation + diagnostics
        ↓
derived projections
   CLI / JSON / API / OpenAPI / MCP / Cockpit / docs / completion
```

There must not be separate hand-maintained registries for each consumer.

## Canonical identity

Every entity that needs machine processing must have a stable identity.

Define or reuse:

- canonical ID;
- kind;
- schema version;
- title/summary;
- applicability/scope;
- dependencies/relationships;
- status/maturity if the project already models it;
- tags/domains only if semantically useful;
- source/provenance;
- enforcement references where relevant.

Do not make human-readable filenames the only identity if the existing architecture already has stronger IDs.

## Provenance

Imported/adapted donor concepts need traceability without coupling runtime behavior to the donor checkout.

The target must remain self-contained after import.

Record enough information to answer:

- where did this concept originate?
- which donor commit/path informed it?
- was it copied, adapted, merged, or reimplemented?
- what replaced it?
- what local entity now owns it?

Do not make Majordomus require `~/dev/prismatic-platform` at runtime.

## Schemas

Extend canonical schemas rather than inventing per-feature YAML.

Schema evolution must be versioned according to repository convention.

Validation errors must be actionable and indicate:

- file/entity;
- violated invariant;
- expected shape;
- remediation.

## Relationships

Where useful, allow machine-resolvable relations such as:

- skill implements doctrine;
- rule enforces doctrine;
- command exposes capability;
- docs explain entity;
- test validates invariant;
- schema governs artifact;
- donor item informed target item.

Avoid free-text-only relationships when stable IDs exist.

## Zero registration

Adding one valid skill or doctrine under the canonical discovery scope must not require editing:

- Rust arrays;
- CLI tables;
- API registries;
- MCP registries;
- Cockpit navigation;
- docs indexes.

If any unavoidable protocol boundary requires generated code, generate it from canonical metadata and validate drift.

## Implement

Implement the foundational changes needed by later prompts:

- canonical types/loaders/registry extensions;
- provenance support;
- schema changes;
- relationship resolution;
- diagnostics;
- deterministic discovery;
- generated index hooks where existing architecture expects them.

Add focused tests now.

Do not yet import every donor item. Build the landing zone correctly first.
