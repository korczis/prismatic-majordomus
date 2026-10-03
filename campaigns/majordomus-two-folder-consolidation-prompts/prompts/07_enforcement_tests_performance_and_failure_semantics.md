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

# Phase 7 — Enforcement, tests, performance budgets, diagnostics and failure semantics

## Objective

Turn every important invariant from documentation into executable enforcement. This phase should make architectural regression expensive immediately rather than six months later when somebody discovers three registries and a shell parser have reproduced through binary fission.

## Build an enforcement matrix

For each invariant, identify the cheapest reliable enforcement layer:

- Rust type system,
- schema validation,
- repository discovery invariant,
- unit test,
- property test,
- integration fixture,
- snapshot/golden test,
- doctest/example,
- CLI `doctor` diagnostic,
- pre-commit/project gate if one exists,
- CI job,
- documentation drift check.

Avoid enforcing everything five times. The point is coverage without duplicated logic.

## Required invariants to enforce

At minimum:

1. init idempotence,
2. sync determinism,
3. minimal target-root footprint,
4. external mutation ownership tracking,
5. safe managed-block updates,
6. conservative uninstall preserving authored `.ai/**`,
7. no manual registration for discoverable artifacts,
8. recursive README/front-matter correctness,
9. schema compatibility/versioning,
10. generated provenance/reproducibility,
11. provider adapters cannot expose secrets,
12. root/provider bridge content cannot become duplicated canonical rules,
13. runtime/cache paths are disposable and appropriately ignored,
14. no network/build in shell entry path,
15. environment/banner performance budget,
16. CLI/API/MCP/docs consumers agree on discovered counts/identity where they expose the same concept,
17. migration from legacy fixtures is deterministic,
18. modified user-owned/managed-conflict files are never silently overwritten/deleted.

## Property/idempotence testing

Where practical create property-style tests over temporary repositories:

```text
apply(init, repo) -> repo1
apply(init, repo1) -> repo2
assert repo1 == repo2
```

And:

```text
plan(repo) == plan(repo)
```

And for clean managed external bridges:

```text
repo0 -> init -> uninstall(default)
```

should preserve original user files plus any intentionally retained authored `.ai` knowledge according to documented semantics.

## Cross-surface consistency

Create tests that add a synthetic schema-valid artifact and verify every applicable consumer sees it without another registry edit. Do this through shared model APIs rather than brittle UI scraping where possible, plus at least one end-to-end CLI/API/MCP smoke path.

## Performance

Benchmark or time the performance-sensitive paths. Establish project-realistic targets informed by current behavior and the dynamic CLI banner reference. Prefer explicit budgets such as warm environment snapshot/banner under tens of milliseconds and bounded cold discovery, but calibrate to real code/hardware/CI rather than hard-coding impossible numbers.

Tests should verify no forbidden subprocess/network/build operations occur on env entry if the architecture can instrument them.

## Diagnostics

Improve `majordomus doctor` (or existing equivalent) so a human can understand:

- schema errors,
- missing README hierarchy,
- legacy root artifacts,
- duplicate/manual registries,
- ownership conflicts,
- stale generated materializations,
- migration needed,
- provider availability without secrets,
- env bridge problems,
- incomplete/unrecoverable install state.

Diagnostics should include remediation commands/paths derived from actual capabilities. Avoid generic “something went wrong” prose, humanity already has enough of that genre.

## CI

Integrate the right gates into existing CI without multiplying redundant jobs. Reuse caches and existing generated/docs validation. Keep fast architectural checks early. If a full e2e migration suite is slower, structure it appropriately but keep it mandatory where reliability requires it.

## Completion criterion

A future developer cannot casually reintroduce root sprawl, hand-maintained registries, unsafe uninstall, or provider duplication without tests/gates complaining immediately and specifically.
