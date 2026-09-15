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

# Phase 1 — Repository forensics and target architecture

## Objective

Perform a deep forensic inventory of the current Majordomus repository with one question in mind: **what must change so `majordomus init` becomes a minimal two-folder, data-driven, reversible integration without duplicating existing mechanisms?**

This phase is primarily discovery plus architectural grounding. Make only low-risk changes required to record the decision cleanly (for example ADR/rule/doctrine scaffolding if the existing project mechanism demands it). Do not launch the broad migration yet.

## Investigate

Trace, with concrete file/module references:

1. Current `majordomus init` implementation and every artifact it creates/modifies.
2. Existing `.ai/**` structure, schemas/front matter, README hierarchy, discovery code, registries, rules, doctrines, skills, ADR/knowledge/session/handover systems.
3. Existing `.majordomus/**` structure and which files are authored vs generated vs runtime/cache.
4. Root-level Majordomus-related artifacts: `AGENTS.md`, `CLAUDE.md`, `.envrc`, shell scripts, `justfile`/`.just/**`, config files, provider configs, MCP configs, generated indexes, docs snippets, install/bootstrap hooks.
5. Rust crates/modules that represent repository/project metadata, config, capabilities, manifests, discovery, environment snapshots, server routes, provider support, schemas, migrations, and CLI subcommands.
6. CLI/API/OpenAPI/MCP/Cockpit/docs generation pipelines and whether they already share a model or maintain parallel lists.
7. Shell completion generation and `just` integration/introspection, if present.
8. Current uninstall/reset/clean/migration behavior and whether ownership of external mutations is tracked.
9. Existing provider integration for Claude/Codex/Gemini/OpenAI-compatible/local providers and whether provider-specific files are canonical or derived.
10. Tests and fixtures around init, repository discovery, generated artifacts, schemas, docs drift, API/MCP exposure, environment entry, and installation.
11. GitHub Actions/CI gates that enforce generated files, docs, schemas, Rust tests, Zola pages, or repository hygiene.
12. Performance-sensitive environment/banner code. Read the supplied `references/Dynamic-CLI-banner.txt` and compare it to reality in the repository.

## Produce an explicit classification

For every Majordomus-related artifact category, classify it as one of:

- `canonical-authored-ai`
- `canonical-majordomus-config-or-ownership`
- `derived-persisted`
- `generated-disposable`
- `runtime-ephemeral`
- `cache`
- `external-compatibility-bridge`
- `legacy-to-migrate`
- `user-owned-do-not-touch`

Also identify duplicate truths, especially lists or metadata separately maintained for CLI/API/MCP/docs/UI/provider configs.

## Define the target invariants using repository-native mechanisms

Add or update the smallest suitable set of rule/doctrine/ADR documents so the project explicitly enforces:

- persistent Majordomus state lives under `.ai/**` and `.majordomus/**` by default;
- root-level artifacts are adapters only;
- no manual registry for discoverable artifacts;
- generated artifacts are reproducible and provenance-aware;
- external mutations are ownership-tracked and reversible;
- `init`/reconcile is idempotent;
- uninstall is conservative and preserves user-authored semantic data by default;
- structural auto-updates never occur merely because a shell entered the directory;
- provider-specific files cannot become canonical sources of truth.

Use the existing distinction between rules vs doctrines correctly. Do not add redundant documents if one existing invariant can be extended.

## Architecture output

Document a target architecture grounded in actual modules. Aim for a canonical typed model conceptually like:

```text
repository filesystem + git + manifests + .ai/** + environment
                    ↓
               discovery
                    ↓
          typed RepositoryModel
                    ↓
     registries / capabilities / ownership
                    ↓
        derived projections/materializations
   CLI API OpenAPI MCP Cockpit docs completion env
```

But name/reuse existing types if the repository already has equivalents. Do not create `RepositoryModel` merely because this prompt uses the name.

Define how desired state and actual state should be represented for `init/sync`, where ownership ledger data should live, and how migrations will be versioned. Explicitly state what belongs in `.ai/**` vs `.majordomus/**`.

## Required validation

Run the repository's existing static/documentation/schema checks relevant to any changed ADR/rule/doctrine files. Ensure no generated drift is introduced.

## Deliverable

At the end, the repository must contain a concrete, evidence-backed migration plan with ordered implementation slices and risks. The next phase must be able to implement schemas/layout without rediscovering the entire world.
