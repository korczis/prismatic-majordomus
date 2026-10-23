# 10 / BENCHMARKS + DELIVERY ECONOMICS

**Mission:** Turn claims of speed and savings into reproducible performance and throughput measurements.

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


## Measured workloads

Record pinned machine/OS/toolchain, configuration and SHA. Compare warm and cold Rust CLI, plugin discovery, Rhai compile/execute/AST cache, generators, shell wrapper overhead, UI graph derive/serialize/render, CI critical path, PR lead time, release latency. Baselines must be real observations, not invented targets.

## Benchmark design

Repeat enough iterations for stable medians/p95, separately measure startup and steady state, isolate cache states and concurrency. Collect stderr/stdout and verification output without secrets. Benchmark fixture sizing derived from representative repo data and synthetic stress cases. Verify functional equivalence before interpreting a speedup. Preserve no-JS accessibility.

## Observability and economics

Integrate existing telemetry provenance where present. Differentiate estimated vs reconciled usage/cost and missing API data. Report useful ratios: merged PR per hour, accepted intents per hour, verification time per merged change, CI dollars/time if grounded. Do not fabricate measured token savings or API reconciliation. A dashboard without authoritative inputs is a diagnostic display, not accounting.

## Continuous gates

Convert only stable actionable budgets to blocking gates, with source, test, baseline, threshold and failure reproduction. Preserve existing tests. UI performance failure must be independent of developer laptop quirks or explicitly labeled as environment-sensitive.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
