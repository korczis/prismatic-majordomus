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

# Phase 02: Hardened embedded Rhai runtime and policy projections

Mission: add Rhai as an embedded declarative orchestration/policy front end over Phase 01, never as a replacement Rust implementation layer.

## Runtime

- Evaluate current Rhai release and exact enabled feature flags, security tradeoffs, binary impact and repo MSRV. Pin through Cargo.lock per repo convention, audit dependencies. Decide whether a new crate is justified; prefer existing CLI/core boundaries.
- Register typed namespaced modules from canonical capability registry. `md::workflow`, `md::plan`, `md::invoke` snippets are conceptual only; inspect actual Rhai Module API and choose supported syntax.
- Separate pure declaration/plan compilation from effectful execution. Script evaluation MUST NOT execute process/fs/network side effects simply by constructing a workflow. No privilege escalation through `eval`, imports, custom functions or raw OS access.
- Restrict operations, call/expr depth, stack, arrays/maps/strings and modules, sandbox imported paths beneath canonical allowed roots with symlink escape prevention, reject unknown capabilities and dangerous non-whitelisted calls.
- Treat scripts from UI/mesh/untrusted repos as untrusted. Host permissions are not the caller's permissions; effective permissions are intersection of script identity, origin policy, session/intent autonomy and explicit grant.
- Rhai max_operations is not a wall-clock safety guarantee. External work must enforce watchdog deadline, cancellation, process-group kill and resource policy outside Rhai.
- Error types with spans, module/source IDs, cause chain, capability policy decisions and actionable diagnostics.
- AST compilation cache: hash source + transitive imported source + Engine/Rhai version + enabled features + canonical capability ABI/schema + relevant policy/optimizer revision. DO NOT assume serialized AST portability across process/platform/version; distinguish in-process compiled AST reuse from validated persistent cache designs.
- Compilation/discovery must not trigger network or builds on `direnv` or `just --list`.
- Prefer Rhai for human-authored conditional policy/composition. If a Rust capability already defines a complete workflow, derive it directly and create ZERO extra `.rhai` script.
- Provide `majordomus script check|run|plan|explain` or map to existing CLI commands rather than duplicating CLI hierarchy. `--dry-run` must not mutate.

## Fixture test examples (adapt to actual module names)

- Successful workflow construction with typed step args and linked deps.
- Rejected unauthorized fs/process/network/deploy calls.
- Import path traversal, symlink escape, cyclic imports, runaway loops, excessive memory/strings, panicking host function, long-running external subprocess.
- Cache hits and invalidation when imports, engine ABI/policy change; deterministic outputs.
- Concurrent invocations and cancellation safety; error span and stack preservation.
- Portable runtime behavior across Linux/macOS/Windows where supported.

## Acceptance

- End-to-end: Rhai definition -> canonical plan -> single existing executor -> structured event/evidence; demonstrable policy denial and safe cancellation.
- No arbitrary `sh -c` shortcut, no shell interpolation, no global writable filesystem escape, no unverifiable sandbox claims.
