# 01 / CI + EVIDENCE RECOVERY

**Mission:** Recover master credibility, eliminate false reds/greens, accelerate feedback with sound test selection.

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


## Evidence capture

Pin HEAD and CI run ids; inspect shellcheck toolchain differences, test/cases/486 and 1097 history, site link-check and deployment failures. The observed 318 vs 26 GitHub-sync drift discrepancy is a historical incident: independently reproduce or invalidate it on current HEAD. Inspect docs/CLAIMS.yaml and generated SITE_CLAIMS. Distinguish recorded test vs current test execution.

## Reproducible test policy

Each observed failure gets a smallest reproducer and classification: product, harness, environment, flaky, fixture drift, external outage, projection drift. Re-run negative-path tests and prove expected nonzero code propagates. Do not replace tests with grep assertions that produce false matches. Pin shellcheck version where necessary, use semantic assertions, shrink fixtures, repair real faults.

## CI DAG and optimization

Trace workflow DAG and job duration using actual GitHub Actions timestamps; derive critical path. Separate fast affected-scope preflight from mandatory full release verdict. Implement keyed cache on toolchain/lockfile/features/platform; test invalidation and cold fallback. Avoid caches with hidden dependency on generated products. Prefer Rust integration tests over shell unit tests for canonical Rust features without reducing coverage. CI toolchain must match local verified rules.

## Claim freshness contract

Claim→source implementation→test→run→commit/inputs→current verdict. Guarantee no test absent or stale appears green. If execution evidence missing, explicitly say unknown/unverified. Make evidence graph update from canonical schema without manual counters. Test stale, missing, skipped, failed, generated drift and dirty-tree inputs.

## Acceptance

Master gates are honest, recent and reproducible. At least one current false signal fixed if found. Show time before/after and test coverage preserved. Final CI state and release-bound SHA proven, not inferred.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
