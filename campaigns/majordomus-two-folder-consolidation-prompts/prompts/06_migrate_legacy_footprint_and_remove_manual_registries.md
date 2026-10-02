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

# Phase 6 — Migrate legacy footprint, consolidate scripts/config and eliminate manual registries

## Objective

Now perform the broad repository cleanup: move/consolidate Majordomus-related state into `.ai/**` and `.majordomus/**`, retire redundant root scripts/config/provider files, and replace hand-maintained registries with discovery/derived projections.

This is the phase most likely to create accidental breakage. Use the reconciler/migration system from prior phases, not a giant shell move script.

## Inventory-driven migration

Use the classification produced in Phase 1 and re-scan current reality. For each legacy artifact decide one of:

- move into `.ai/**` as authored semantic content,
- move into `.majordomus/**` as implementation/runtime/generated state,
- convert to a tiny managed root bridge,
- replace with Rust CLI functionality,
- replace with generated projection from the repository model,
- keep because it is genuinely user/project-owned and not Majordomus implementation state,
- delete only when proven obsolete and safely migrated.

### Root scripts

For Majordomus operational scripts in root or generic `scripts/`/`lib/` areas, prefer migrating durable logic into the Rust app/control-plane if the repository architecture already trends that direction. Keep shell only for irreducible bootstrap glue. Preserve CLI compatibility via deprecated aliases/wrappers only when needed, with a removal path.

### Config files

Collapse redundant Majordomus config into inference and typed `.majordomus/**` overrides. Do not mechanically move five config files into `.majordomus/` and declare victory. The goal is **less configuration**, not better-organized duplication.

### Provider-specific directories

Where `.claude/**`, codex/gemini configs, MCP files, or other provider-specific material duplicate canonical `.ai/**`, migrate semantics into `.ai/**` and leave/generate only compatibility adapter material. Preserve provider-local settings that are genuinely provider-specific and cannot be represented canonically.

### Generated registries/indexes

Find any lists of rules/doctrines/skills/routes/providers/docs/use-cases maintained by hand or committed as unexplained generated artifacts. Replace them with typed discovery and, when a persisted index is valuable, generate it from canonical inputs with provenance and drift checks.

### `justfile` / task definitions

Do not move root `justfile` merely to satisfy the two-folder slogan if `just` requires it or it is project-wide developer tooling unrelated to Majordomus install footprint. Instead minimize Majordomus-specific duplication inside it and bridge/delegate to canonical CLI commands/derived modules. Separate the product's own source-repo developer tooling from what `majordomus init` injects into **target** repositories.

This distinction is critical: the Majordomus source repository can legitimately contain development files that should never be copied into initialized target repos.

## Compatibility

Implement migrations that recognize legacy paths and map them to canonical destinations. Provide clear warnings/deprecation diagnostics rather than immediate breakage where public workflows may depend on old paths.

Update references throughout:

- Rust code,
- tests/fixtures,
- CI workflows,
- documentation,
- Zola links,
- API/MCP examples,
- install scripts,
- provider adapters,
- shell completion,
- rules/doctrines.

But update generated docs through their generator, not direct editing of generated output.

## Negative tests

Add gates that fail when someone reintroduces:

- an unauthorized Majordomus root config/registry/script,
- a manually maintained discovery registry,
- provider-specific canonical duplication,
- tracked cache/runtime output,
- a directory under `.ai/**` or `.majordomus/**` without a schema-valid README,
- generated artifact with unknown provenance where the policy requires provenance.

## Completion criterion

The source repository and realistic target fixtures demonstrate that the new architecture is real, not layered on top of legacy duplicated paths. Old paths are either migrated, deliberately retained with rationale, or rejected by enforcement.
