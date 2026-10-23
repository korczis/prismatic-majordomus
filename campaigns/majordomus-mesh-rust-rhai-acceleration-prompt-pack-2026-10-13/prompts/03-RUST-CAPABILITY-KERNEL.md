# 03 / CANONICAL RUST CAPABILITY KERNEL

**Mission:** Make native Rust the single execution authority across CLI/MCP/API/peer/UI/Rhai.

**Target:** korczis/prismatic-majordomus, discover current default branch and SHA.


## NON-NEGOTIABLE MAJORDOMUS EXECUTION DOCTRINE

1. Read repository `AGENTS.md`, `.ai/README.md`, effective rules, doctrines, relevant ADRs, current documentation and generated capability registry before edits. Resolve scoped context for every touched path. Repository reality supersedes this prompt's guesses.
2. Inspect current master HEAD, current PR status, active worktrees, mesh peers, ongoing sessions, claims, CI and deployment before planning. The October 13 audit findings are leads, not proof of current defects.
3. Use existing Majordomus CLI/MCP/API/mesh/task lifecycle. Discover exact commands from the installed binary and canonical metadata; do not assume examples are real commands.
4. One authority: existing canonical capability declaration. Typed contracts, authorization, errors, provenance, idempotency and side effects are defined once. Surfaces derive from it; no parallel transport registries, UI inventories or hand-maintained documentation tables.
5. Rust owns execution, IO and enforcement. Rhai is bounded composition of explicitly authorized host capabilities. Third-party untrusted plugins do not receive unrestricted native process privileges. Shell and just contain only thin bootstrap/compatibility adapters, ideally generated. No wrapping `bash -c` inside Rhai as a fake migration.
6. Work in independently releasable vertical slices. Declare scope and dependencies, announce to peers, run impact checks, use canonical worktrees and collision checks. Never overwrite another worker's work. Do not assume claim is a lock unless its enforcement proves it.
7. Maintain strict evidence: reproduced failing behavior → code fix → focused regression → cross-surface validation → required CI → merge → version/release → deployed artifact verification. An attempted action is not a completed outcome.
8. Target 10–20 minute micro-release cadence, 30 minute integration-cycle goal, but never bypass required quality gates to meet a timer. Versioning follows repository policy; do not invent releases/tags or unconditional bumps.
9. `--admin` cannot be used to waive failing required safety/quality checks. Respect GitHub repository protection, explicit authorization and least privilege. Do not force-push trunk or delete user-owned unmerged data.
10. Update the existing `.ai` governance, schemas, ADR if warranted, generated docs, GH Pages/Cockpit, API/MCP/CLI, tests, benchmarks and evidence wherever the capability changes. Reuse existing derivation pipelines and version compatibility guarantees.
11. Before reporting complete, show exact revisions, commands and test results, artifact links, deployed revision and smoke-test evidence. Distinguish CONFIRMED, HISTORICAL, POTENTIAL, BLOCKED and RESOLVED findings.
12. If a task is blocked, checkpoint through existing Majordomus state and move to the next ready nonconflicting unit. Do not fabricate unattended execution after the session ends.


## Existing architecture archaeology

Inspect capability! declarations, builtin registry, command parser, type/schema generation, dispatcher, router, MCP projection, peer RPC, authorization hooks and transport-specific special cases. Produce a trace for three sample capabilities from Rust declaration to each consumer and to test evidence. Mark duplicates and missing gates.

## Typed invocation contract

Extend existing types minimally with stable identity/version, typed inputs/outputs/errors, side effect metadata, authz, idempotency, actor/worktree/intent context, timeout/cancellation, trace ID, provenance and feature availability. Preserve compatibility; do not claim Rust compilation enforces JSON schema unless runtime boundaries verify. Decouple local invocation from HTTP server. No HTTP loopback dependency for direct CLI.

## Core enforcement and delivery

All entrypoints call the same permission and validation implementation. Test CLI/MCP/API/peer differential conformance using the same fixtures and expected error codes. Test absence/revocation of peer auth and typed failures. Route UI actions through canonical transport operation, never custom endpoint-only semantics. Derive help, OpenAPI, MCP schemas, editor commands, docs and completions.

## Incremental slices

K1 inventory+tests; K2 dispatch parity; K3 unified error/authorization; K4 derived metadata and docs; K5 remove obsolete adapter implementation. Each slice independently passing and releaseable. Prove by introducing one safe capability without editing each consumer.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
