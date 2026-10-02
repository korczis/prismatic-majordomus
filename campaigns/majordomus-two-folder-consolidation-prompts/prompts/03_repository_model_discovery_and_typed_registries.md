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

# Phase 3 — Unified repository discovery, typed registries and capability model

## Objective

Build or consolidate the **single repository introspection spine** that all Majordomus consumers can use. The goal is to stop separate CLI/API/MCP/docs/env/provider code from re-discovering or manually enumerating repository facts independently.

## First inspect existing abstractions

Search for existing repository/project/environment/capability/config/registry types and services. Prefer extending an existing coherent abstraction over creating a grand new model. If several partial models exist, consolidate through adapters or a well-defined facade with a migration path.

## Discovery inputs

Where relevant and already available, derive repository facts from:

- repository root and git state,
- Cargo workspace/package metadata,
- other toolchain manifests only when applicable,
- `.ai/**` typed artifacts and hierarchical READMEs,
- `.majordomus/**` installation/ownership state,
- existing route/service metadata,
- existing provider registry/discovery,
- `just` introspection (`just --dump --dump-format json`) if the project uses just and the installed version supports it,
- environment/toolchain availability,
- configured local endpoints,
- existing generated registries only as a migration input, never as a new canonical truth.

No network calls in the base repository discovery path unless explicitly requested by a higher-level operation.

## Typed registry requirements

A canonical discovery layer must be able to enumerate at least the artifact/capability classes that actually exist in this repository, such as rules, doctrines, skills, ADRs/knowledge, workflows/use cases, providers, services/routes, or commands. Do not invent empty abstractions for categories the repository does not support.

Each registry should derive identity and metadata from the artifact/schema convention. Adding one new valid artifact should make it visible automatically.

## Capability graph / relationships

Where useful, expose relationships such as:

```text
artifact → schema
artifact → parent README context
capability → CLI command
capability → API route
capability → MCP resource/tool
capability → documentation projection
provider → adapter requirements
service → endpoint
```

Do not build a graph database merely because graphs are fashionable. Use the simplest typed representation compatible with existing project architecture, but make relationships queryable enough to remove hard-coded per-surface lists.

## Determinism and caching

Discovery must be deterministic for a given repository state. Define fingerprints/invalidation inputs for cached materializations using relevant files such as git HEAD/index, Cargo manifests, `.ai/**`, `.majordomus/**`, route metadata, just definitions, etc. Reuse any existing environment snapshot/cache mechanism.

Cache is an optimization only. Deleting cache must never destroy canonical information or change semantic output.

## API boundary

Create a stable internal interface through which downstream consumers can obtain:

- repository identity,
- discovered typed artifacts,
- capabilities,
- VCS/toolchain/environment summary where relevant,
- services/endpoints,
- providers and availability without exposing secrets,
- workflow/command hints,
- diagnostics,
- ownership/install state.

Support serializable output where needed for existing CLI/API/OpenAPI/MCP layers, ideally from the same Rust types/schema generation pipeline.

## Eliminate duplicate discovery

Search for code that separately counts/parses rules, skills, providers, routes, or docs metadata. Migrate consumers where safe to the unified discovery layer. Do not perform all UI work yet, but make the model usable by those surfaces.

## Tests and benchmarks

Add/extend tests for:

- discovery determinism,
- zero-registration addition/removal/rename behavior,
- invalid artifact diagnostics,
- hierarchical README context resolution,
- cache invalidation/fingerprint correctness,
- secret redaction/provider availability semantics,
- representative multi-language or minimal repositories if existing fixtures support them,
- cold/warm discovery performance where this path feeds shell entry/banner.

Set or preserve realistic performance budgets discovered from current code/reference. Do not hide expensive network/build calls in discovery.

## Completion criterion

There is one clear code path from repository state to a typed discovery/capability model. Later phases can implement `init/sync`, banner/completion, API/MCP, and UI as projections of that model rather than bespoke scanners.
