# Majordomus ← Prismatic Platform: Skills & Doctrines Import Pack

## Goal

Use Claude Code with a large context window to inspect:

- source repository: `~/dev/prismatic-platform`
- target repository: `~/dev/prismatic-majordomus`

and selectively import/adapt the **relevant skills, doctrines, rules, schemas, enforcement patterns, tooling conventions, knowledge mechanisms and supporting infrastructure** from Prismatic Platform into Majordomus.

This is **not a blind copy operation**.

The result must be native Majordomus architecture:

- single source of truth,
- inferred / derived / generated where practical,
- typed and schema-backed,
- data-driven,
- auto-discovered,
- no duplicate registries,
- no manual consumer-by-consumer registration,
- documented,
- tested,
- enforced,
- retrospectively migrated,
- visible through the existing Majordomus surfaces where semantically relevant:
  - CLI
  - structured JSON
  - API
  - OpenAPI / Swagger
  - MCP
  - Cockpit
  - documentation / GitHub Pages
  - diagnostics / doctor / validation
  - completion and developer tooling where applicable.

## Important principle

Do **not** make Majordomus pretend to be `prismatic-platform`.

Import the reusable epistemic/governance/developer-experience machinery, not source-repository accidents.

Every imported concept must have:

1. source evidence,
2. relevance justification,
3. adaptation decision,
4. canonical Majordomus owner,
5. migration path,
6. enforcement,
7. tests,
8. documentation,
9. cross-surface integration where applicable,
10. provenance sufficient to understand where the idea came from.

## Recommended execution order

Run the prompts in order from the root of `~/dev/prismatic-majordomus`.

```bash
cd ~/dev/prismatic-majordomus
claude
```

Then feed:

1. `01-source-audit-and-gap-map.md`
2. `02-canonical-import-architecture.md`
3. `03-skills-import-and-runtime-integration.md`
4. `04-doctrines-rules-and-enforcement.md`
5. `05-retrospective-migration-and-deduplication.md`
6. `06-cross-surface-integration.md`
7. `07-tests-gates-docs-and-provenance.md`
8. `08-final-system-validation.md`

Do not skip prompt 01. Humans love skipping archaeology and then wonder why the new cathedral is built on plumbing.

## Execution discipline

For every prompt:

- read `AGENTS.md` and repository-local instructions first;
- inspect the current working tree before edits;
- preserve unrelated user changes;
- do not reset or discard existing work;
- prefer the repository's canonical tooling over ad-hoc shell;
- update session context / handover machinery if the repository requires it;
- commit only when repository conventions call for it;
- never claim a surface is integrated unless validated by tests or direct inspection;
- never copy secrets, credentials, machine-local caches, generated build output or source-repository-specific state.

## Completion invariant

The project is finished only when a representative new imported/adapted skill or doctrine can be introduced at its canonical source and automatically:

- validate against schema,
- be discovered,
- appear in canonical registries,
- become available to applicable CLI/API/MCP/Cockpit/docs projections,
- participate in diagnostics and governance,
- require no duplicated consumer registration.

If adding one item means editing seven inventories, the migration has merely dressed duplication in ceremonial robes.
