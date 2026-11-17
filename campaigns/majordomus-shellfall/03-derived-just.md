# MAJORDOMUS / OPERATION SHELLFALL

You are Claude Code Opus/Fable operating inside the CURRENT checkout of `korczis/prismatic-majordomus`.

## Mandatory operating protocol

- You are implementing, not just proposing. Inspect live HEAD, active worktrees, open work and repository conventions before altering files. A historical 2026-09-11 archive showed 218 `.sh` files, root `justfile`, `.just/*.just`, optional `.ai/local/cache/command-graph/bridge.just`, shell `bin/majordomus`, Rust `apps/majordomus-cli`. These are historical clues, NEVER current truth.
- Read AGENTS.md, .ai/**, .majordomus/** if present, doctrines, rules, policies, ADRs, schema, command graph, derive architecture, tests, existing CI/release and docs; discover their actual paths. Do not invent names or assume all subsystems exist.
- Architectural invariants: canonical single source of truth; derived/inferred/generated projections; typed schemas; provenance/evidence; deterministic order, hashes, and output; no duplicated registries; no ungoverned side effects; compatibility first; incremental rollout and rapid feedback.
- NEVER create another independent execution engine, generic runner, workflow manifest or front-end state store when an existing canonical subsystem can be extended. Reuse existing execution plane, command graph, registry, mesh/events, API/MCP and documentation generators.
- Every potentially mutating command goes through approved capability policy, explicit execution context and audit/evidence; dry-run must perform zero side effects. Release/deploy/remote irreversible actions obey existing authorization policy. Do not weaken governance to satisfy tests.
- Keep changes small, independently testable, reviewable, and integratable. Distinguish observed success from assumptions. Do not claim CI green, PR merged, release published or deployment verified unless actually confirmed. Preserve external approvals and branch protections.
- No blindly applied cargo/just/Rhai feature guesses: inspect actual installed versions and vendor/upstream APIs. Maintain compatibility with repo's supported toolchains and existing behavior.
- Use cached/incremental builds, avoid repeated full-workspace runs, and measure wall-clock performance before/after. Prefer fast focused loops and one final comprehensive validation.
- After each phase output: exact changed files; source-of-truth ownership; commands/results and durations; migration counts; observed blockers; remaining debt; next phase handoff. Persist provenance/handoff using existing Majordomus mechanism, not an invented parallel ledger.
- Do not stop at an architecture essay; perform implementation. If blocked, deliver maximum verified partial implementation without misrepresenting completion.

## Definition of done for every phase

1. Implementation exists on disk and is exercised.
2. Contract/schema or public behavior is tested, including negative and compatibility tests.
3. Generators are deterministic and derived outputs have drift checks.
4. Existing .ai governance is updated only where truly necessary and the enforcement actually executes.
5. Code formatted, relevant tests run, performance and errors reported accurately.
6. Deletions remove dead machinery instead of leaving shadow implementations.

---

# Phase 03: Derived / inferred Just powered by existing command graph

Mission: preserve existing `just` UX while making ALL project-owned workflow recipes deterministic projections of one canonical command/workflow registry.

## First inspect before changing

The September 11 snapshot's root `justfile` had `set shell := ["bash", "-euo", "pipefail", "-c"]`, positional arguments, imports `.just/{test,build,serve,ci,site}.just`, and optional `.ai/local/cache/command-graph/bridge.just`; it invokes `bin/majordomus-cli commands bridge`. CURRENT repo may differ. Respect live architecture, migration history, API contracts and generated state.

## Implement

- Read actual installed just version and official machine-readable `just --dump --dump-format json`, metadata, imports, positional args, module capabilities; verify version-feature matrix rather than assuming latest.
- Build a typed introspection adapter for legacy Just files and detect generated-vs-owned definitions. Do NOT treat generated bridge as a canonical input (self-ingestion/cycle).
- Make command graph/workflow registry canonical source of generated recipes: stable names, grouping/namespace, semantic ordering, descriptions, requirements, args and dispatch target.
- Keep root justfile small and standalone-parseable in fresh clone. Determine best current import strategy (imports vs modules) based on compatibility and path/cwd semantics. Do not change recipe namespace inadvertently.
- Generate optional bridge only from canonical model. Check source content hash, stable serialization and deterministic emission; atomic write, avoid rewriting unchanged output; prevent generator races across worktrees.
- Preserve argv exactly through Just -> CLI: no `{{args}}` into shell, no eval, no argument flattening, no injection. Consider Just recipe parameter/positional forwarding and explicit array semantics; test paths with spaces, `$(...)`, semicolon, quotes, leading dashes, multiline and unicode.
- Replace shell-based recipe bodies with direct executable invocation using Just's actual recipe evaluation rules. If Just still requires a shell for line evaluation, use the thinnest POSIX adapter or a shebang/alternative shell only when justified, never hide domain logic in shell syntax. Validate portability in actual supported just versions.
- `just --list`, help and completion should not compile/build/phone home. Fresh clone and missing generated bridge must have a functioning bootstrap path.
- Maintain legacy recipe aliases for callers until explicitly versioned/deprecated.
- If Just supports metadata, generate it from canonical registry; use introspection in migration but NEVER create `Just -> command graph -> Just` cycles.
- Derive CLI help, docs, Cockpit command actions and MCP tool discovery from exactly the same inventory; enforce cross-surface IDs/order.

## Must test

- Fresh checkout without bridge; corrupted/old bridge; regeneration no-op; source schema change invalidation; concurrent regeneration; read-only tree; worktree-specific cache; offline mode.
- `just --list`, `just test`, `just check`, `just bridge` (if still applicable), old aliases and namespaced commands.
- Roundtrip argv + exit codes; no command injection; deterministic byte comparison across shuffled discovery order and separate runs.
- One synthetic registered workflow automatically appears in generated Just and other projections without consumer edits.

## Acceptance

- `just` is a zero-business-logic adapter; canonical source remains command/workflow graph.
- No incremental workflow metadata hand-maintained in `.just` and `.ai` simultaneously.
- Generated artifacts regenerate fast, idempotently, with CI drift detection.
