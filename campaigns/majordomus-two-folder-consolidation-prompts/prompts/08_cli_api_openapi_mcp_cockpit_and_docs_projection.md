# Shared execution contract

You are working in the `prismatic-majordomus` repository. Treat the repository itself, its existing AGENTS.md / CLAUDE.md / `.ai/**` / `.majordomus/**` / Rust code / tests / generated schemas / OpenAPI / MCP / Cockpit / Zola/GitHub Pages sources as the source of truth. **Inspect before changing. Never assume a path, crate, command, schema, capability, server route, provider adapter, or workflow exists just because this prompt mentions the concept.** Reuse, extend, or migrate the existing mechanism whenever one already exists.

The target architecture follows these non-negotiable Majordomus principles:

1. **Minimal repository footprint.** After `majordomus init`, Majordomus-owned persistent state in a target repository SHOULD live under exactly two namespaces: `.ai/**` and `.majordomus/**`. Root-level changes are compatibility bridges only, minimal, stable, machine-owned, and reversible.
2. **`.ai/**` is portable semantic repository intelligence.** Rules, doctrines, skills, agent-facing knowledge, ADRs, handovers, session context, prompts, workflows, use cases, and similar semantic material belong here when appropriate. It must remain conceptually useful without Majordomus and across LLM providers.
3. **`.majordomus/**` is implementation/control-plane state.** Runtime metadata, derived indexes, generated adapters, cache, ownership ledger, migrations, environment integration, materialized views, local runtime state, and Majordomus-specific mechanics belong here. It MUST NOT become a second copy of `.ai/**` knowledge.
4. **One source of truth.** No manually maintained secondary registries, duplicate lists, duplicated documentation, duplicated schemas, duplicated route enumerations, duplicated provider lists, duplicated completion lists, or hand-copied UI data. Prefer discovery + typed models + derived views.
5. **Infer / derive / discover before configure.** Prefer conventions, repository inspection, typed front matter, Cargo metadata, git, route metadata, generated OpenAPI, existing canonical project metadata, and runtime introspection over explicit config. Explicit overrides are the final escape hatch, not the default architecture.
6. **Typed, schema-backed data.** Machine-processable markdown/front matter and persisted manifests must have explicit versioned schemas. Generate schemas from canonical Rust types when practical. Validate them in tests/gates. Do not hand-maintain a schema separately from the canonical type when generation is available.
7. **README hierarchy.** Every directory under `.ai/**` and `.majordomus/**` must have a meaningful `README.md` with valid typed/versioned front matter. It explains the directory purpose, ownership/lifecycle, artifact conventions, applicable schemas, inheritance/merge semantics, discovery behavior, and extension rules. This must be mechanically enforced, not aspirational prose.
8. **No manual registration.** Adding a valid artifact under a discoverable namespace should be sufficient to register it. Applicable consumers such as CLI, API, OpenAPI/Swagger, MCP, Cockpit, docs, search/index, validation, provider adapters, and completion should consume the same typed registry/model.
9. **Generated means disposable and reproducible.** Every generated artifact must have provenance, generator identity/version, canonical inputs or a reproducible derivation path, and lifecycle semantics. Generated files must never silently become canonical input.
10. **Root files are adapters, not truth.** `AGENTS.md`, `CLAUDE.md`, provider-specific files, `.envrc`, `justfile`, root README snippets, or similar root files may contain only tiny stable bridges/markers if strictly required. Repository-domain logic belongs in typed Majordomus code or canonical `.ai/**` content.
11. **Reversible ownership.** Every mutation outside `.ai/**` and `.majordomus/**` must be recorded by a typed ownership ledger with enough information to safely reconcile and uninstall. Never delete or overwrite user-owned edits blindly.
12. **Idempotence.** Re-running `majordomus init` or an equivalent reconciliation on a conforming repository must produce zero changes. This is a hard test requirement.
13. **Self-updating without surprise mutation.** Runtime/cache/index refresh may be automatic and safe. Structural source migrations must be explicit/reconciled. `cd`/direnv must never rewrite arbitrary tracked files or trigger network/build-heavy work.
14. **Performance budgets are testable.** Environment entry/banner/completion paths must be local, bounded, cached where appropriate, and avoid network/build operations. Preserve or improve the environment-entry architecture described in the supplied dynamic CLI banner reference.
15. **Provider neutrality.** Claude, Codex, Gemini, OpenAI-compatible providers, local providers, and future providers are adapters over the same repository model. Provider-specific representations are derived compatibility surfaces, never canonical semantic state.
16. **All relevant surfaces stay in sync.** If the repository already exposes capabilities through CLI, REST/API, Swagger/OpenAPI, MCP, Cockpit, Zola/GitHub Pages, generated docs, or shell completion, extend the canonical model so those surfaces derive from it. Do not bolt on a second implementation just to satisfy a checkbox.
17. **Tests and documentation are part of the implementation.** Unit, integration, golden/snapshot where appropriate, property/idempotence, migration, uninstall, schema, CLI/API/MCP, and performance tests must accompany the change. Update user/developer documentation and generated docs through their canonical generation pipeline.
18. **Backwards compatibility and migration matter.** Existing repositories may already contain legacy Majordomus files, scripts, `.claude/**`, provider-specific configs, root scripts, generated registries, or partially migrated `.ai/**` / `.majordomus/**`. Detect and migrate safely. Preserve user-authored content. Never use destructive broad deletes.
19. **No speculative rewriting.** Prefer the smallest coherent architectural change that creates the generic mechanism. Do not rewrite working subsystems purely for aesthetic reasons.
20. **Eat your own dogfood.** If the repo has rules/doctrines/ADRs/knowledge generation/session handovers, use them. Record meaningful architectural decisions in the existing ADR mechanism. Update the relevant doctrine/rule so the invariant remains enforced after this implementation.

## Execution discipline

- Start by reading repository-local agent instructions and the README/front matter hierarchy relevant to each area you touch.
- Inspect git status before editing. Do not trample unrelated user work.
- Search broadly for existing implementations and references before adding a concept.
- Establish the current architecture and data flow, not just filenames.
- Prefer Rust for durable Majordomus CLI/server/control-plane logic when that matches the existing codebase. Shell should remain tiny glue only.
- Preserve public compatibility unless a deliberate migration is implemented and documented.
- Use existing formatting, linting, test, generated-artifact, docs, and validation commands discovered from the repository.
- Do not claim success from compilation alone. Exercise the feature end-to-end.
- When a test cannot run because of a genuine external prerequisite, document exactly what was run, what was not, and why. Do not quietly weaken the test.
- Never expose credentials or secret values in logs, banners, API, generated docs, or test fixtures.
- Work on an issue/milestone/worktree flow if the repository's current rules require it. Follow existing worktree placement doctrine; do not invent a second worktree convention.
- Commit only if repository-local instructions require or explicitly permit autonomous commits. Never rewrite unrelated history.

## Required handover after this phase

Before finishing, create/update the repository's canonical session context / handover using the existing mechanism. It must contain:

- objective and scope completed,
- architectural findings,
- exact files/modules changed,
- schemas/types added or changed,
- migration/backwards-compatibility implications,
- commands/tests/gates run and results,
- unresolved risks or TODOs that are truly necessary,
- next prompt/phase to execute,
- git/worktree/branch state.

Do not invent a new handover format if the repository already has one.

# Phase 8 — Project the canonical model through CLI, API/OpenAPI, MCP, Cockpit and documentation

## Objective

Complete the outward-facing integration. Every relevant surface should expose the same canonical repository/integration state without maintaining parallel definitions.

## CLI

Inspect current command taxonomy first. Integrate capabilities such as init/sync/doctor/uninstall/env/status/plan into the existing CLI conventions. Help text, examples, completion, and documentation should derive from canonical command metadata where the project already has/gains such machinery.

Useful outputs may include:

- human-readable init/sync plans,
- `--json`/machine-readable repository/integration status if consistent with existing UX,
- ownership/managed mutation inspection,
- migration status,
- doctor diagnostics,
- environment snapshot/banner state,
- explanation of why a root bridge exists and who owns it.

Do not create commands merely to mirror every struct field.

## REST/API + OpenAPI/Swagger

If the server already exposes repository/environment/registry state, extend those canonical response types. Candidate endpoints/capabilities include repository integration status, schemas, discovered artifacts, ownership/migration status, and environment state.

Requirements:

- OpenAPI generated/derived from the same route/types pipeline,
- no duplicate hand-authored endpoint list in docs,
- secrets redacted by type/design,
- stable versioning/error semantics consistent with current API,
- Swagger UI automatically sees changes from OpenAPI generation.

## MCP

Expose useful resources/tools through the existing generic MCP architecture. Prefer capability-driven dynamic enumeration. Potentially useful operations: inspect repository model, list typed artifacts, validate integration, compute sync plan, explain diagnostics. Mutating operations must follow existing safety/authorization design; do not add unrestricted filesystem mutation just because MCP makes it technically amusing.

MCP schemas should derive from canonical types/schema when practical.

## Cockpit

Make the Cockpit a projection of the same data, not a client-side reimplementation. Add a focused integration/repository-health area if one does not already exist, showing for example:

- canonical `.ai` / `.majordomus` status,
- discovered artifacts/capabilities,
- root compatibility bridges and ownership,
- migration/legacy warnings,
- schema/README validation status,
- provider availability with secrets redacted,
- environment/services,
- last/current sync plan or drift.

Use existing Flowbite/Alpine/Cytoscape/etc. only where they are already architectural choices and actually help. Do not add visual complexity for a list that needs six rows.

If the Cockpit already uses websockets/event streaming, wire relevant changes into that existing event model. Do not add a second transport.

## Documentation / Zola / GitHub Pages

Document the architecture through canonical docs sources and generators:

- two-folder contract,
- minimal init diff,
- `.ai` vs `.majordomus` ownership/lifecycle,
- how zero-registration discovery works,
- init/sync/dry-run/uninstall workflows,
- provider adapter model,
- `.envrc`/AGENTS bridge model,
- migration from legacy layouts,
- troubleshooting/doctor diagnostics,
- extension guide for adding new artifact kinds or provider adapters,
- security/privacy notes around provider credentials.

Where possible generate reference tables/pages from schemas/registries/CLI metadata rather than hand-maintain them. Ensure local served docs and GitHub Pages derive from the same canonical inputs.

## Cross-linking

If the project has typed front matter and graph-like relationships, ensure docs/UI can cross-link:

- rule/doctrine → enforcing test/gate,
- artifact → schema,
- capability → CLI/API/MCP/docs exposure,
- migration → affected versions,
- generated artifact → provenance inputs,
- root bridge → ownership record.

## Tests

Add/extend:

- CLI snapshots/doctests/examples,
- API/OpenAPI schema tests,
- MCP discovery/schema tests,
- docs generation/drift tests,
- Cockpit data/API integration tests appropriate to current stack,
- one cross-surface fixture showing zero-registration propagation.

## Completion criterion

Users can inspect and understand the same underlying integration state through CLI, API/Swagger, MCP, Cockpit, and docs, while developers maintain the fact exactly once.
