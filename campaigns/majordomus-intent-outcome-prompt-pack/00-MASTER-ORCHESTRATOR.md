# 00 — Autonomous Intent-to-Outcome Program

You are the implementation agent inside the current repository. Your mission is to complete the **existing Majordomus intent subsystem into a reliable, autonomous, evidence-backed Intent → Outcome lifecycle**. Keep the underlying architecture canonical, typed, derived and enforced. You are implementing, not generating another proposal.

## First, establish reality

1. Read governing instructions and context from repository root down to affected directories; inspect worktree, branch and HEAD. Do not overwrite unrelated user changes.
2. Trace actual intent model, evidence ledger, milestone/issue/task graph, session lifecycle, mesh/peer layer, CLI, API, OpenAPI, MCP, Cockpit, docs claims, deployment and CI policies. Use path discovery, not guessed file locations.
3. Build a capability matrix with columns `capability | exists | incomplete | tested | enforced | consumers | evidence`. Classify each statement as observed or inferred.
4. Inspect the current public claims and local docs; contradictions are defects. In particular verify stage-derived, criteria-covered, plan-reviewed, intent-realization and command/deployment evidence behavior.
5. Map each of the eight files to native milestone/issue records using existing .ai workflows if available. Reuse existing equivalent work; dedupe rather than recreate. If no native import exists, execute the sections directly and record traceability without fabricating GitHub issues.

## Execution scheduling

Run phases 01 through 08 in dependency order. Parallelize only work whose ownership and prerequisites are independent; a shared model/schema change precedes downstream projections. Each `Issue` heading in a phase is an independently testable task. After each task: focused test, canonical gate, record exact evidence, update existing work records and continue.

Use the existing native Majordomus session/issue/mesh mechanisms, not an external self-written orchestration shell. All provider calls are optional and guarded; the workflow must work with only the active agent. Pause instead of performing unauthorized remote/destructive actions; proceed on safe local tasks. Limit retries and stop on true blockers rather than cycling forever.

## Critical invariants

- Intent is a stable desired state contract, independent of execution implementation.
- Intent criteria are observable and falsifiable, with typed proof definitions.
- Gap analysis must differentiate missing, unobserved and contradicted evidence.
- Plan coverage means every planned issue has a purpose and every criterion has a path to proof.
- Opposition happens before the start transition can succeed.
- Work status is not outcome status; only current verifiable evidence proves outcome.
- Stale evidence revokes satisfaction; no mutable "green" shadow status.
- Newly discovered entities project automatically into all relevant surfaces.
- User-friendly mobile UI is a read/write projection over existing command/API authority.
- The platform and injected consumer repositories share the same tested machinery.
- Permission, privacy, timeouts, idempotency and bounded resource use are mandatory.

## Practical success demonstration

Using disposable repositories and a representative intent, demonstrate: user defines outcome → repo introspected → uncovered criteria assessed → plan synthesized → adversarial review captured → blocking critique prevents plan-start → safe tasks assigned/resumed → tests/commands/deploy evidence verified → intent satisfied → source mutation makes proof stale → intent returns to verifying with actionable replan. Show CLI, API/MCP and UI projection agreement wherever supported and capture exact test evidence.

## Exit conditions

Never claim DONE on a green build alone. Each acceptance-matrix scenario must have a real executable proof or be explicitly BLOCKED/UNIMPLEMENTED. Finish by producing the report template with SHA, file references, test commands, exit codes, notable output, unverified claims, compatibility implications and necessary authorization gates.

## Non-negotiable execution contract
- Inspect actual checked-out HEAD first. Treat all paths and status descriptions here as hypotheses until verified.
- Read applicable AGENTS.md, .ai context chain, .majordomus, policies, rules, doctrines, ADRs, schemas, claims, tests and commands before changes. Never weaken governance to make this task pass.
- Reuse canonical typed models, registries, derived projections, evidence ledger and lifecycle. Do not create another state store, inventory, source of truth, UI model, API variant, or shell-script orchestration universe.
- Distinguish declared / observed / derived / inferred facts, include provenance and freshness. Derived states must not become mutable shadow status fields.
- Work autonomously: discover, design minimally, implement, run tests, repair regressions, document, enforce, demonstrate. Do not stop at analysis; do not ask for routine architecture choices.
- Never assert a passing check without executing it and recording exact command, exit code, revision and meaningful output. Unknown, stale, unavailable and blocked are not success.
- Prefer narrow incremental commits, existing task/issue/milestone machinery and resumable checkpoints; do not push, merge, deploy, publish or modify remote resources unless repository policy and user authorization permit it.
- Unsafe/irreversible, destructive, billable or external-side-effect actions require explicit authorization. Safe local planning/projections can be automatic.
- When a provider/peer is unavailable, degrade deterministically to single-worker local execution; never simulate real peer conclusions.
- Test both the Majordomus repository itself and an independently initialized/injected consumer fixture. All added features must work on both when relevant.
- Keep existing API/CLI contracts compatible or explicitly migrate/version them with schema and behavioral tests.
- Each issue-sized section below is independently testable, linked to its acceptance evidence and suitable for native .ai/GitHub plan projection.

## Required per-issue protocol
1. Locate current code and behavior, show concrete gap and provenance.
2. Specify input/output types, invariants, failure behavior and consumers.
3. Implement using existing subsystem boundaries; remove duplicate legacy path.
4. Write a positive test and at least one adversarial/negative or stale-evidence test.
5. Run focused tests, canonical checks and relevant cross-surface checks.
6. Update executable claim/ADR/doctrine only where project policy warrants it.
7. Record evidence references and mark the issue complete only when it actually passes.

## Required completion report
Report: audited base SHA; existing machinery reused; code changes; migrations; CLI/API/MCP/UI/docs impact; proof and commands with exit statuses; security/performance/compatibility impacts; unresolved blockers; derived satisfaction of every criterion; no invented successes.
