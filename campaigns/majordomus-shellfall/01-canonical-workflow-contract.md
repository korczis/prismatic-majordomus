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

# Phase 01: Canonical workflow / capability contract, NOT a second execution engine

Mission: extend existing Majordomus execution plane and command graph so workflow identity, typed steps, permissions and evidence exist once and Just/Rhai/UI/CLI/MCP consume them.

## Design and implementation

- Identify current canonical execution plane, commands, capability registry, derived API, event stream, evidence ledger, intent/issue links, derive DAG and provenance contracts. Reuse them.
- Add minimum typed workflow abstraction only where missing: `WorkflowId`, `CapabilityId`, `WorkflowDefinition`, `StepId`, typed inputs/outputs, typed dependencies, conditions, side-effect class, required permissions, environment prerequisites, cache policy, timeout/cancellation policy, retry/idempotency, artifacts, evidence policy, provenance and semantic ordering.
- Use schema/versioning appropriate to existing design. Distinguish immutable workflow definition, mutable execution instance and derived presentation; no manually mirrored catalog.
- Define deterministic plan compilation: resolve dependency DAG, reject cycles/missing refs/conflicting outputs/ambiguous ids, stable topological order and error diagnostics. No unstable iteration or implicit shell.
- Actions should be capabilities, not arbitrary command strings. For external programs, implement an explicitly governed `ProcessSpec { executable, argv: Vec<OsString>, cwd, env_delta, stdin_policy, timeout, ... }` adapter with no implicit shell interpolation.
- CLI/REST/MCP/mesh/Rhai must all invoke the same canonical execution API and publish the same event/evidence record, with correlation ID and causation provenance.
- Define workflow discovery from current command graph and project features; infer defaults when safe; use Rhai only for nontrivial compositional policy.
- Preserve existing public commands/flags/return codes; compatibility matrix must be explicit and tested.
- Plan/explain/dry-run are pure outputs; prove no execution on plan.
- Provide executor semantics for failure propagation, cooperative cancellation, process group cleanup, parallel dependency scheduling, limits and isolation.
- Tool discovery and schemas must derive from the same capability declarations; generated OpenAPI/MCP/Just/front-end models must not duplicate metadata.

## Focused tests

- DAG validation/cycles/missing refs/canonical order/priority interactions; ready queue and event sequence.
- Partial failure, cancelled child, timeout, retry only for eligible idempotent steps, dry-run no writes, protected capability denied.
- Argument containing whitespace, quotes, leading `-`, `$(...)`, unicode, newline and semicolon transmitted as literal values.
- Same workflow through CLI and direct execution yields same canonical plan and evidence shape.

## Acceptance

- One capability execution contract is demonstrably shared across at least CLI and an actual Just/Rhai adapter probe.
- No special new executor shadows existing one.
- Workflow definition is versioned/typed/inferable, and negative cases produce actionable diagnostic.
