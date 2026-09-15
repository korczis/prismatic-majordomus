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

# Phase 9 — End-to-end migration, adversarial audit, cleanup and release readiness

## Objective

Treat the implementation as hostile until proven otherwise. Perform a full end-to-end audit of the two-folder architecture, migrate the Majordomus repository/fixtures, remove leftovers, validate docs/UI, and leave the branch release-ready according to repository conventions.

## 1. Re-run repository-wide inventory

Search for every legacy name/path/config/registry/script identified in Phase 1 plus new variants discovered during implementation. Confirm each is:

- intentionally retained,
- migrated,
- generated from canonical data,
- compatibility-only,
- or removed.

No unexplained duplicate truth may remain.

## 2. Golden target-project scenarios

Create/use representative temporary fixtures covering at least:

### Minimal clean repository

Expected fresh init result is only `.ai/**`, `.majordomus/**`, plus strictly required tiny root bridges.

### Repository with existing AGENTS.md and `.envrc`

User content must survive byte-for-byte outside managed blocks. Second init must be no-op. Uninstall must remove only managed blocks.

### Repository with legacy Majordomus footprint

Migration should consolidate safely and report what moved/changed. It must preserve authored knowledge and not keep legacy manual registries alive as hidden truth.

### Repository with edited managed bridge

Sync/uninstall must surface a conflict and refuse unsafe destructive action by default.

### Repository with newly added semantic artifact

Add one valid rule/skill/doctrine/knowledge artifact. Verify automatic discovery and every applicable projection without manual registration.

### Provider compatibility

Exercise at least the providers supported by current tests/config architecture, verifying only minimal required adapters exist and secrets never appear.

## 3. Init/sync/uninstall cycle

Prove with filesystem/golden diffs:

```text
repo0
→ init
→ repo1
→ init
→ repo1 unchanged
→ sync
→ repo1 unchanged
→ mutate canonical artifact
→ sync
→ only expected derived outputs change
→ uninstall(default)
→ external managed bridges gone
→ authored semantic knowledge preserved
```

## 4. Performance and environment audit

Exercise `.envrc`/environment entry/banner/completion in cold and warm states. Verify:

- no network,
- no build,
- no structural source mutation,
- bounded local work,
- graceful missing service/binary,
- secrets redacted,
- cache invalidation correctness.

Compare behavior to the supplied dynamic banner reference and current documented budgets.

## 5. Schema/migration audit

Validate every persisted/front-matter schema, generated JSON Schema/OpenAPI artifact, migration path, README hierarchy, and compatibility fixture. Test upgrading at least the representative old layouts available in git/fixtures.

## 6. Cross-surface audit

Confirm CLI, API/OpenAPI/Swagger, MCP, Cockpit, generated docs/GitHub Pages, and completion consume canonical model data. Search source for duplicated static enumerations that should no longer exist.

## 7. Security audit

Search logs, fixtures, docs, serializers, provider status and debug output for secret leakage risks. Ensure credential values are neither persisted into `.majordomus` manifests nor surfaced through public API/UI/banner.

## 8. Documentation audit

Read the public docs as a new user. The install/init/remove story should be simple and accurate. Include a concrete before/after tree and reversible uninstall explanation. Ensure docs do not instruct users to manually register artifacts that are now discoverable.

## 9. Cleanliness

Run formatting, lint, full tests, docs build, schema/generation drift checks, relevant benchmarks, and CI-equivalent local gates discoverable in the repository. Remove temporary/debug artifacts. Confirm git status contains only intentional work.

## 10. Final architectural report

Produce/update the canonical handover/ADR/knowledge entry with:

- old vs new architecture,
- exact root-footprint invariant,
- `.ai` vs `.majordomus` contract,
- source-of-truth table,
- ownership/uninstall model,
- migration behavior,
- enforcement matrix,
- performance results,
- cross-surface projections,
- remaining explicit exceptions with rationale.

Do not leave vague “future cleanup” for problems that are within this scope. Either fix them now or document a concrete external blocker.

## Final acceptance checklist

- [ ] Fresh init is minimal.
- [ ] Second init produces zero changes.
- [ ] Sync is deterministic.
- [ ] Default uninstall is conservative/reversible.
- [ ] `.ai/**` holds portable semantic truth.
- [ ] `.majordomus/**` holds Majordomus mechanics/ownership/runtime.
- [ ] Root bridges are tiny and owned.
- [ ] No manual discoverable registries remain.
- [ ] Generated state has provenance and is reproducible.
- [ ] README hierarchy and schemas are enforced recursively.
- [ ] Provider adapters are derived and secret-safe.
- [ ] Env/banner/completion are bounded and data-driven.
- [ ] CLI/API/OpenAPI/MCP/Cockpit/docs stay in sync through canonical types/model.
- [ ] Legacy layouts have deterministic migration.
- [ ] CI prevents architectural regression.
- [ ] User-facing docs match actual behavior.
