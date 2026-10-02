# Prompt 01 — Deep Source Audit, Relevance Classification, Gap Map

Read `00-CONTEXT.md` first.

Do not implement broad changes yet.

Your job is to understand both repositories well enough that later migration is evidence-driven.

## Mission

Perform a deep comparative audit of:

- `~/dev/prismatic-platform` as donor;
- current `~/dev/prismatic-majordomus` as target.

Find all reusable skills/doctrines and the infrastructure they depend upon.

Do not only grep filenames containing `skill`, `rule`, or `doctrine`. Trace how concepts are discovered, parsed, typed, validated, rendered, enforced and consumed.

## Build an evidence-backed donor inventory

For every candidate capture:

- canonical/visible name;
- kind;
- source path(s);
- schema/front matter;
- implementation owner;
- dependencies;
- discovery mechanism;
- runtime consumers;
- validation;
- tests;
- docs;
- CI/hooks/gates;
- generated artifacts;
- whether source is generic or Prismatic-specific;
- coupling to source-specific namespaces/data;
- maintenance burden;
- overlap with Majordomus.

Look for hidden supporting infrastructure such as:

- schemas;
- loaders;
- registries;
- macro/codegen mechanisms;
- validators;
- lints;
- test helpers;
- manifest builders;
- documentation generators;
- prompt/session integration;
- provider integration;
- diagnostics;
- provenance;
- command metadata;
- UI metadata;
- knowledge/ADR links;
- repository bootstrapping.

## Build the target capability map

For Majordomus, identify existing equivalents and maturity:

- absent;
- partial;
- duplicate;
- legacy;
- canonical and stronger than donor.

Do not overwrite superior Majordomus functionality merely because the donor has a similarly named file.

## Produce a migration matrix

Create a repository-local working artifact in the canonical temporary/planning location, following target conventions, with columns/fields equivalent to:

```text
donor_id
donor_kind
source_paths
source_commit
purpose
dependencies
target_equivalent
overlap
decision
decision_reason
target_owner
schema_action
migration_action
surface_impact
enforcement_action
tests_required
docs_required
risk
priority
```

Classification must be one of:

- IMPORT_AS_IS
- ADAPT
- REIMPLEMENT
- MERGE
- REFERENCE_ONLY
- REJECT

## Ranking

Prioritize candidates by actual leverage, not novelty.

Highest priority usually means concepts that improve:

- consistency;
- context preservation;
- validation;
- agent cooperation;
- architectural governance;
- reproducibility;
- deduplication;
- explainability;
- machine discovery;
- auditability;
- extension ergonomics.

Explicitly avoid importing things whose only benefit is "Prismatic has it".

## Required analysis

Identify:

1. donor skills worth importing;
2. donor doctrines worth importing;
3. supporting rules/schemas needed;
4. Majordomus concepts that already supersede donor concepts;
5. collisions in IDs/naming;
6. incompatible assumptions;
7. source-specific dependencies that must be cut;
8. opportunities to generalize a donor concept beyond its current implementation;
9. opportunities to consolidate duplicated Majordomus machinery while importing;
10. things that must emphatically not be imported.

## Deliverable

At the end, report:

- donor inventory summary;
- highest-value imports;
- merges with existing Majordomus features;
- rejected source-specific material;
- architectural dependencies;
- proposed order of implementation;
- major risks.

Do not proceed to broad implementation unless necessary to produce reliable evidence.

This prompt is reconnaissance. Make it unusually thorough.
