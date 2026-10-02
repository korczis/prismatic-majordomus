# Prompt 05 — Retrospective Migration, Consolidation, and Deletion of Duplicates

The canonical architecture and imported concepts now exist.

Perform the ugly but necessary part humans tend to postpone until "later", that mythical geological era.

## Mission

Migrate current Majordomus content and remove obsolete/parallel implementations.

Search repository-wide for:

- duplicate skills;
- duplicate doctrine text;
- obsolete rules;
- stale schemas;
- manually maintained indexes;
- duplicated command lists;
- frontend taxonomies duplicating backend metadata;
- docs copies of registries;
- source-specific legacy paths;
- old import experiments;
- inconsistent front matter;
- entities missing canonical IDs;
- dead adapters;
- shell implementations superseded by application code;
- generated artifacts committed without drift enforcement.

## Consolidation principles

For each duplicate pair/group:

1. identify canonical owner;
2. migrate consumers;
3. preserve redirects/aliases only if public compatibility needs them;
4. generate aliases where possible;
5. delete redundant implementation;
6. add regression test preventing reintroduction.

Do not solve duplication with a "legacy registry" that lives forever.

## Normalize existing entities

Bring all in-scope skills/doctrines/rules to the canonical schema.

Automate migrations where reasonable.

Ensure every migrated entity:

- validates;
- resolves relationships;
- has stable ID;
- has provenance if donor-derived;
- has documentation;
- participates in canonical discovery.

## Project initialization

Inspect `majordomus init` or equivalent.

If skills/doctrines are materialized into target projects, ensure initialization derives the correct set from canonical metadata/policy instead of copy-pasted lists.

Honor minimal-invasiveness principles and existing `.ai` / `.majordomus` design.

## Clean result

At the end run repository searches proving old duplicate registries and obsolete donor-derived fragments are gone or intentionally compatibility-scoped.
