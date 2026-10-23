# 05 / EXTERNAL PLUGINS

**Mission:** Enable source-tree-independent plugins with least privilege, versioned manifests and zero consumer registration.

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


## Smallest complete vertical

Implement/extend manifest discovery, integrity, compatible ABI/protocol negotiation, effective permissions, lifecycle, load/unload and health. First plugin type: trusted/signed-or-explicitly-approved Rhai package using existing module mechanisms. Provide a fixture plugin installed outside repo and demonstrate discovery without editing core source or UI/transport registries.

## Trust model

Treat plugin package input as untrusted until policy allows it. Rhai host methods expose only canonical granted permissions. For arbitrary external providers choose supervised process with versioned protocol or WASM isolation as a separate evidence-backed phase. Never load untrusted dynamic Rust cdylib into binary. Explicit paths, network destinations, secret access, quotas, resource limits and audit logs.

## Plugin state machine

DISCOVERED→VALIDATED→TRUSTED→COMPATIBLE→ENABLED→HEALTHY; reject duplicates, unsigned where required, digest mismatch, incompatible engines, dependency cycles, missing provider, revoked privilege. Record all rejections in doctor and graph. Lifecycle must tolerate missing optional plugins offline.

## Cross-surface automaticity

From one canonical plugin-provided capability, assert CLI listing+invoke, MCP listing+invoke, API/OpenAPI, peer negotiation, UI discovery/actions, capability graph, docs/just/completion where exposure permits. No manually inserted consumer entry. Negative tests show denied exposure across every surface. Plugin provenance and health are visible with stable IDs.

## Migration and release

P1 manifest/schema and test fixture; P2 runtime install/discovery; P3 permission gates; P4 cross-surface derivation; P5 security/adversarial review and release. Keep a small externally reproducible getting-started example.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
