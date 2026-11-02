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

# Phase 07: Governance enforcement, cross-surface integration, release & dogfooding

Mission: make shell elimination and canonical workflow derivation permanent, visible and continuously proved, without writing decorative doctrines.

## Governance

- Add/update .ai doctrine/rules with machine-executable gates in canonical convention, enforcing:
  * repository business orchestration must be typed Rust capability or Rhai policy/workflow;
  * manually maintained command/Just/API/MCP/UI inventories forbidden when canonical derivation exists;
  * new shell executable requires exception justification, owner and explicit approval/review;
  * migrated test has bidirectional traceability to historical assertion coverage;
  * generated projections are deterministic and up to date;
  * Rhai module imports and capabilities obey repository trust boundaries;
  * compile-time and runtime policy failures are actionable and evidenced.
- Enforce no Bash content smuggling via `.just` recipe shell blocks, GitHub Actions `run`, Docker build scripts, inline Rust `Command::new("sh").arg("-c")`, arbitrary Rhai exposed execution strings. Avoid naive grep false positives: parse/scope surfaces where feasible, allow narrowed adapters.
- Govern exceptions with review process rather than permanent unbounded allowlist; create report of counted remaining shell boundaries.
- Generate machine-readable shell audit and cross-surface workflow inventory from actual repo sources; surface in doctor/check, CLI JSON, API, MCP, Cockpit/UI and docs using existing derived endpoints rather than separate implementations.

## Dogfood

- Run actual canonical quality/test/docs/derive/release-preflight workflows by Rust CLI, generated Just, Rhai and UI/MCP where permissions permit, and assert identity/evidence equivalence.
- Introduce a representative new workflow/capability and prove zero manual consumer registration (Just/help/API/OpenAPI/MCP/UI/docs/completions).
- Test `majordomus ui` live execution events, failure details, progress/cancellation, cache state and canonical link to intents/milestones/issues; honor real existing projection stack.
- Test release/deploy dry-run, rollback, no-op regeneration and replay/diagnostics. Do not claim production deployment unless actually performed and independently checked.
- CI local/full gate, supported-platform matrix, current master/PR merge constraints, release version policy and protected branch requirements. Reconcile integration through authorized workflow; report unperformed remote actions instead of pretending.

## Final exit gates

1. shell audit reports zero unauthorized repository-domain Bash/sh logic.
2. All originally inventoried shell test assertions mapped; all deleted tests have validated Rust/Rhai replacements; remaining shell tests only validate genuine shell boundaries.
3. Exactly one canonical workflow/command/capability source; no duplicate metadata with Just, UI, MCP, docs or CI.
4. Derived Just is byte-stable, atomic, portable and accepts exact argv; fresh clone/bridge regeneration tested.
5. Rhai is sandboxed with explicit capability permissions and wall-clock cancellation at executor boundary.
6. Same workflow executed from Rust CLI/Just/Rhai/UI/MCP produces consistent plan, event, evidence and status semantics (allowing different correlation IDs).
7. CI and repository gates enforce invariants; generated docs and API schemas are synchronized.
8. Measured tests and workflow times reported; compatibility tests green for supported platform contract.
9. `.ai` and ADR/documentation accurately describe actual verified behavior, with no claim drift.
10. Branches/PRs merged/released/deployed only if verified and permitted; current state reported honestly.

## Final report format

- BASELINE (sha, shell count/LOC, tests, perf)
- ARCHITECTURE (canonical owners, Rust/Rhai/Just boundaries)
- MIGRATION (removed/retained shell by reason, old -> new test mapping)
- PROOF (commands and precise results, cross-surface evidence, parity tests)
- PERFORMANCE (measured before/after with method)
- GOVERNANCE (active gates, not just documentation)
- ROLLOUT (commits/PRs/releases/deploy proof and blockers)
- RESIDUAL RISK (real constraints and next smallest viable work)

Do not claim completion on a checkbox alone. Provide executable proof.
