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

# Phase 06: Portable, blazing-fast, deterministic developer experience

Mission: validate shell-eliminated workflows across supported OSes, worktrees, CI caches, derive DAG, Just, Rhai and Rust tests. Optimize measured hot paths and cold bootstrap. No vanity benchmarks.

## Portability

- Derive support matrix from repo policy; test Linux/macOS/Windows where declared. Do not promise native Windows support if blocked by unrelated OS APIs; document and resolve supported target contract.
- Ensure path/argv/env UTF-8 vs OsString correctness, separator normalization, permissions, symlinks, case sensitivity, line endings, process child cleanup, clock/timeouts and exit codes.
- Fresh clone with no locally generated bridge, no precompiled binary, missing just/direnv, no network; define expected degraded-but-informative behavior. Avoid `cargo build` on shell directory entry.
- Binary download/install provenance and trusted source selection; no shell dependency in native package scenarios.

## Derivation and caching

- Extend existing Merkle/hash DAG where possible; never introduce another cache without inspecting current ownership.
- Hash canonical workflow definition, relevant input facts, resolved imports, capability implementation/version, policy/ABI, target triple and toolchain where outputs depend on them. Distinguish cache key for AST, plan, generated Just, test results and artifacts.
- Explicitly non-cacheable defaults for network, mutable external state, secrets, time-sensitive operations, git mutations, releases and deploys. Cache only pure or proven hermetic deterministic tasks.
- Invalidation for `.ai`, manifests, Just graph, Rust source, toolchain, Rhai imports and projected docs. No cache poisoning across repos, principals, credentials, worktrees or trust boundaries.
- Concurrent generation uses content-addressed atomic commits or appropriate synchronization and immutable snapshots; no partially generated bridge visible to reader.
- Measure parse/plan/execute overhead separately from compilation/test workload. Profile repeated UI/CLI list calls and empty repo entry.

## CI/DX

- Centralize toolchain and cached target dir, sccache/cargo build caching where beneficial, shared stable artifacts with appropriate correctness guards.
- Incremental test scheduling from dependency DAG based on changed inputs, but run necessary comprehensive coverage on release/merge gates.
- Eliminate cargo compile per shell case/test and redundant duplicate CI matrix work. Optimize tests for parallelism and fixture isolation.
- Documentation, CLI help, Just help, Cockpit UX and MCP all present the same workflow categories/status and stable default ordering.
- Diagnostic commands should report why workflow exists and its source, cache outcome, permissions, test provenance and shell exception rationale.

## Evidence

- Record baseline and post-migration p50/p95 on same hardware/CI runners: warm workflow listing, Just regeneration, Rhai compilation/plan, quality check, test suite, CI end-to-end, fresh clone boot.
- Keep performance budget configurable and validated where practicable, avoiding brittle timing gates on heterogeneous runners.

## Acceptance

- Support matrix executed/annotated, deterministic cross-platform derived contents where meaningful.
- Performance regression gates for hot paths; quantified speedup or transparent explanation of misses.
- Shell-eliminated core works without external POSIX runtime on platforms advertised.
