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

# Phase 00: Forensic inventory, behavior baseline, migration graph

Mission: characterize today's shell dependency, test semantics, command graph, just import/build bridge, runner, environmental bootstrap and portability debt. DO NOT start with blanket file renames.

## Inventory

- Record HEAD SHA, git/worktree status, supported platforms, Rust toolchain, just, direnv, Rhai (if present), CI platforms. Do not overwrite dirty worktrees.
- Inventory tracked/untracked/generated executable content by category: `*.sh`, shebang bash/sh, `bash -c`, `sh -c`, `shell: bash`, `run: |`, Git hooks, Makefiles, docker RUN/ENTRYPOINT, installers, scripts embedded in Markdown, `justfile` and imports, `test/cases`, `bin/`, `.ai` generators and deploy scripts.
- For each file or script surface: path, owner/callers, side effects, dependencies, platform assumptions, args/stdin/stdout/stderr/exit behavior, credentials, runtime, purpose, criticality, test fixture isolation, opportunity to infer from canonical capability. Build machine-readable inventory from source discovery; don't hand curate a second registry.
- Explicitly trace `bin/majordomus`, `apps/majordomus-cli`, `.just/`, `command-graph/bridge.just`, generation refresh and `.envrc`; characterize circularity risks and bootstrap behavior.
- Trace every `test/cases/*.sh` assertion including setup, temp dirs, fake tools, process trees, signal handling, normalization, snapshots, platform gates and teardown. Identify test coverage per feature and find unknown blind spots.
- Characterize `.ai` enforcement and existing Rust integration tests. Identify what can be pure Rust assertions, behavioral black-box tests, Rhai fixture tests, and truly necessary shell contract tests.

## Required deliverables

1. Baseline test results, duration and flakiness sampling. No doctored averages; state environment.
2. Canonical inventory artifacts/report generated from the scanned repository, and a migration DAG ordered by risk and dependencies.
3. Classify each shell surface: REMOVE (duplicated), RUST DOMAIN, RHAI ORCHESTRATION, RUST INTEGRATION TEST, RHAI WORKFLOW TEST, NARROW POSIX BRIDGE, THIRD-PARTY UNOWNED.
4. Capture argv safety, quoting, injection, cwd, env inheritance, exit code and signal contracts as golden fixture tests before porting.
5. Find current canonical owners of commands, execution, events/evidence, schemas, policy, modules and docs. Document gaps without assuming a greenfield design.
6. Quantify measured baseline: shell files, shell LOC, executable shell entry points, Bash-dependent tests, just recipes, duplicated command representations, CI duration and per-platform availability.
7. Flag release/deploy/bootstrap rollback paths and tests to preserve.

## Acceptance

- Every shell artifact classified; no unverifiable claim of completeness.
- Existing behavior and owners mapped.
- Minimal vertical slice nominated (quality/check) with clear measurable goals.
- No functionality deleted in this phase.
