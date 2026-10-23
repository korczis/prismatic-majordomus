# 06 / SHELL + JUST ELIMINATION

**Mission:** Replace domain shell logic with canonical Rust capabilities and bounded Rhai workflow composition; keep generated thin entry adapters.

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


## Comprehensive inventory

Scan scripts/, bin/, lib/, .github/, justfile imports, hooks, CI, test cases and `.ai/repo/automation`. Classify adapter/domain/validator/generator/workflow/test/deploy/tool bridge. Attribute every behavior to canonical Rust or authorized Rhai target; identify platform exceptions (POSIX bootstraps, GitHub runner steps). Record exact callers and exit contracts.

## Migration rule

Port behavioral tests to Rust before deleting original behavior. Build Rust execution for side effects; Rhai for typed safe composition. Generate wrappers from canonical descriptors, preserving exact CLI args/exit behavior and quoting. No `sh -c`, `eval` or embedded shell pipelines disguised as Rhai. `just` remains human ergonomic frontend derived from registry/workflow metadata, not independent pipeline logic.

## Prioritized batches

Rank by frequency, fragility, CI runtime, semantic duplication, risk and port size. Start with the known GitHub projection shell/Rust parity class and high-value generators/validators, not with installer or portable bootstraps. Prove old/new parity on representative fixtures. Remove old implementation only after demonstrated compatibility and clean migration.

## Enforcement

Extend existing shell check rule to detect NEW domain shell executable units or opaque Rhai shell escape without explicit exemption. Allow bootstrap exceptions with rationale, expiry/review and test. Generate projection snapshots and fail drift gate on hand modification. Track shell/domain LOC and remaining migration exceptions as derived metrics, not hardcoded counts.

## DX and performance

Cold directory entry must not trigger build/network; wrappers must have bounded overhead. Measure cold/warm command and generator times. Verify macOS/Linux compatibility and supported shells. Keep .envrc tiny, platform environment handling inside canonical Rust environment snapshot.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
