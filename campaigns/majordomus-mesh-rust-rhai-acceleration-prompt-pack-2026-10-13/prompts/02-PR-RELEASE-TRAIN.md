# 02 / PR INTEGRATOR + MICRO-RELEASE TRAIN

**Mission:** Integrate high-value changes continuously, version and deploy only green verified revisions.

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


## Queue model

Enumerate every open PR and annotate base/head SHA, mergeability, conflicts, changed files, affected capabilities, parent intent/issue, checks, review, dependencies and value. Classification READY/NEEDS_REBASE/NEEDS_REVIEW/RED/CONFLICTING/SUPERSEDED/BLOCKED. A superseded PR points to an exact merged diff or implementation evidence. Recheck all facts before any mutation.

## Safe merge state machine

READY→fresh base→scoped checks→adversarial review→merge→trunk CI→canonical release verdict→tag/artifacts→GH Pages deploy→remote smoke verification→evidence close. Handle remote ambiguous response with read-after-write. Never bypass required checks. Retry only transient network failures with bounded backoff. No force pushes to master. Protect unrelated human worktrees.

## Cadence reconciliation

Inspect existing release ADR 0151, PR #1060, version.yml, release.yml, cadence.yml and current release record. One release clock only. An hourly existing actor may need policy adjustment to enable optional 10–20m targets, but do not fork tag authority. If no eligible candidate, skip release. If publish cannot prove source SHA, mark pending/blocked rather than success.

## Deployment proof

GitHub Pages build and link gates, artifact digest, release tag, deployment workflow ID, published version and published SHA should be auditable in one provenance chain. Smoke critical routes: landing, /ui, docs, evidence, graph navigation and published asset links. Account for CDN/cache propagation without lying about live state.

## Outcome metrics

PR backlog burndown by verified completion, failed integration frequency, merge latency, CI cost and merge→deploy latency. Expose canonical status via existing UI/API without independent frontend registries.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
