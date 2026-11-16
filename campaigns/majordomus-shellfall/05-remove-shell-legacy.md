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

# Phase 05: Eliminate Bash application logic and converge entrypoints

Mission: migrate production shell scripts and wrappers incrementally into Rust capabilities / Rhai orchestration / derived workflows. Do not leave the shell logic as a hidden process behind an API.

## Scope

- Migrate `bin/majordomus` lifecycle handlers to canonical Rust CLI/command graph, preserving existing command names, args and exit semantics using temporary thin wrappers only where unavoidable.
- Migrate .ai validation/derive generators, docs/site, test orchestration, quality gates, git/worktree hygiene, GitHub PR train workflows, mesh/service startup, release, deploy, install, self-update and bootstrap code by risk priority from Phase 00.
- For shell parsing/filtering/templating, prefer typed Rust model operations; Rhai for explicit conditional workflow policy only. Git via existing typed adapter rather than `git ... | awk` pipelines; external program invocations via argv arrays.
- Replace manual Just orchestration with generated Just recipes backed by canonical execution.
- CI workflows should invoke one canonical entrypoint; generated CI snippets if and only if repository architecture already supports safe derivation. Keep minimal platform-specific workflow bootstraps.
- `direnv` semantics require a sourced shell file; keep small reviewed `.envrc` adapter, with `majordomus env export` using safe format/escaping, rather than replacing sourced shell semantics with embedded Rhai magic.
- Installer constraints: uninstalled binary cannot run itself. Design and test bootstrap chain for fresh clone/offline/older versions; provide signed/checksummed binary distributions where existing release practice supports this. Never make pipe-to-shell a hard dependency if platform native alternatives exist.
- Shell compatibility surfaces may remain if truly shell-defined (POSIX bootstrap, certain Git hooks, external environment). Put each on a **narrow, machine-checked exception manifest with owner/reason/expiry or review policy**, avoid allowing the list to grow as a loophole.
- Cross-platform Rust: `std::process::Command` argv, `PathBuf`/`OsString`, env vars, file permissions, symlinks, atomic writes; no ubiquitous `/bin/bash` requirement. Handle executable resolution / process groups and Windows gracefully.

## Controlled rollout

- Use strangler migration with one capability family per commit/PR, not massive one-shot rewrite.
- Feature flag/projection compatibility only as a temporary migration tool; don't ship two permanent active engines.
- For each replacement: observe parity; route old interface to new Rust implementation; remove old implementation; update docs and .ai gates; run integration and rollback test.
- Do not unilaterally deploy or publish if required credentials/permissions/approvals are unavailable. Report blockers with evidence and continue nonblocked work.

## Security and negative tests

- Shell metacharacters as literal args; no command injection, env interpolation attack or path traversal.
- Secrets remain redacted in logs/events/caches; subprocess env scoped to capability.
- Git branch/worktree cleanup respects uncommitted changes and protected branches.
- Release rollback and partial deployment failure; terminated process groups; idempotent retries; no duplicate publishing.

## Acceptance

- Repository-owned business/orchestration logic in shell = 0, or itemized justified blockers with migration follow-up.
- Executable compatibility validated, including completion/help/errors.
- No `bash` runtime dependency for core CLI, tests, or canonical workflow execution on supported platforms.
