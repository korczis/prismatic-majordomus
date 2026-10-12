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

# Phase 04: Port shell test suite into Rust and Rhai without reducing coverage

Mission: replace shell-based testing infrastructure, not simply translate Bash expressions into Rhai. Rust owns fixtures/assertions/process lifecycle. Rhai owns DSL behavioral fixtures where useful. Minimal shell-only tests validate shell compatibility boundaries.

## Mandatory migration protocol

1. Obtain live list of `test/cases/*.sh`, test runners and wrappers. Historical archive contained representative `100_execution_plane.sh`, `76_capabilities_projections.sh`, `96_worktree_topology.sh`, `103_release_projection.sh`, `86_installer_platform.sh`, `08_no_forbidden_constructs.sh`, `123_case_bound.sh`; determine CURRENT equivalents.
2. For each case capture assertion inventory, setup and teardown, asserted exit codes, stdout/stderr, files, behavioral semantics, fixture dependency and provenance ID.
3. Generate a coverage equivalence matrix `legacy_case -> new_test(s) -> assertions -> observed run result`; automate drift checks; no mass deletion until replacement tests pass.
4. Implement reusable Rust integration test harness in existing test architecture or appropriate crate: per-test tempdir, deterministic repo fixtures, synthetic `.ai` tree, fake git/github/docker binaries if needed, command runner with direct argv, stdout/stderr bytes, env overrides, CWD, timeout, cancellation and process tree reaping; zero global `$HOME` mutations and no shared-test race.
5. Convert assertions and fixtures to Rust (`assert_cmd`, `tempfile`, `predicates`, `insta`, `proptest` only when justified and compatible with workspace conventions), prefer existing libraries. No shell subprocess as test implementation.
6. Convert genuinely domain-specific Rhai behavior into `.rhai` test fixtures evaluated by Rust, with positive and negative cases. Rhai should not become a second assertion framework when ordinary Rust tests suffice.
7. For independent black-box parity run legacy and Rust tests on the same immutable fixtures and compare relevant observable behavior; avoid parity on incidental whitespace/time or volatile IDs; be explicit on normalization.
8. Preserve platform-specific behavior as conditional explicitly reasoned tests, not silent skip-everything. Cover Windows path rules, executable extension, line endings, process signal semantics as supported.
9. For integration with Git/Just/UI/API use ephemeral repo, ephemeral ports/sockets, lifecycle cleanup, failures and concurrent run tests.
10. Keep minimal shell boundary golden tests for POSIX bootstrap and direnv where real shell evaluation is intrinsic; do not pretend Rust emulation proves shell compatibility.

## Porting batches

A. Pure validators/parser/schema and `.ai` fixtures.
B. Command graph, derived projections, CLI contracts, Just bridge.
C. Session/intent/issue/workflow/execution and evidence.
D. Mesh/server/process/crash recovery and UI/API/MCP projections.
E. Installer/worktrees/self-update/release/deploy/platform boundaries.

Batch gate: do not delete old case until equivalent assertions pass and old/new are compared where relevant. Run both during a short overlap; make new Rust tests authoritative only when parity is verified. Report case counts/coverage per batch.

## Performance requirements

- Prefer testing compiled Rust library directly for pure logic; use binary spawn only for CLI contract tests.
- Cache compiled binaries across fixtures; no `cargo build` per test; avoid spawning a server per assertion when shared fixture isolation is safe.
- Control concurrency and available resources; no arbitrary sleeps; wait for explicit readiness with bounded retry; seed randomized tests deterministically.
- Add targeted property tests for canonical ordering, DAG invariants, env expansion, argv roundtrip and stable derives.
- Profile before/after total wall-clock, CPU time, test number and CI cache behavior.

## Acceptance

- 100% of removed legacy test cases have mapped behavior/assertions and passing Rust/Rhai counterparts; identify genuine exceptions separately.
- No increase in flaky tests or weakening of gate strictness.
- CI no longer depends on Bash for migrated tests.
- Faster feedback is demonstrated with measurements, not advertised.
