# Milestone 00: Live baseline, repository audit, PR census and architectural decisions

You are Claude Code Opus working INSIDE the currently checked-out `korczis/prismatic-majordomus` repo. Read `AGENTS.md`, `CLAUDE.md`, `.ai/` tree context, canonical project records, ADR 0101, `docs/INTEGRATION.md`, `docs/CI.md`, `docs/CLAIMS.yaml`, `CONTRIBUTING.md` and implementation/tests. Discover actual paths: do not assume all these still exist. Study recent master and all OPEN pull requests through `gh`/GitHub APIs (authenticated if available). The public navigation reported 52 PRs on 2026-10-10, but this is NOT verified per-PR data; retrieve actual list. Never use stale 2026-10-06 rollout state (16 open then) as current. Capture observation time, master SHA, PR head/base SHA, CI state, check names, updated time, conflicts, dependencies and overlapping scopes. Inspect GitHub branch protection/rulesets read-only and compare required checks to actual jobs. Do not extrapolate from 2026-09-11 finding that master was unprotected.

Make no large refactor before publishing a baseline to canonical tracked documents/records using existing conventions. Use the existing `prs refresh`, `prs status`, `prs drain --dry-run`, repo-derived model and direct `gh` only where needed to verify. The integration engine already uses real git merge-tree/merge driver rather than GitHub `mergeable` and replans after every merge; do NOT duplicate it.

## Issue 00.1: Evidence-backed inventory

Inventory relevant runtime components, CLI, MCP, API, UI, discovery, registries, execution, derive pipeline, cache, command graph, lifecycle, evidence, test harness, CI, version/release/deploy and docs generators. For each give authoritative source, consumers, tests, measured or unknown cost, owner, duplication, drift risk, current state with source/commit links. Separate observations from hypotheses and goals. Identify the actual 0.20.x code rather than trusting historical archive.

## Issue 00.2: PR-by-PR census and disposition

Classify EVERY open PR using existing integration disposition semantics and deterministic rank. Record: number, title, head SHA, age, behind count, merge tree relation, checks, blocking reasons, overlapping files, affected milestones/issues, required action, risk, likely value, next safe action. Detect orphaned/stale branches and PRs already semantically superseded. Never delete without proof and repository policy. Produce conflict/dependency DAG and prioritized safe queue. Audit #855 and #874 to avoid reimplementing merged features. Identify which PRs already solve execution, MCP, UI, CI, derivative and installer problems.

## Issue 00.3: Measure baseline and critical path

Run canonical commands for representative no-op derive, one-rule change, Rust change, docs-only change, relevant checks, release path (dry run if supported), and PR refresh/status. Measure wall, CPU, IO, cache hit/miss, fanout, memory, contention, generated output churn. Collect real durations, not guesses; derive previously took 699s on one Linux run and over 1h on loaded Mac (commit f1a892eb) but these are anecdotes. Use repeatable fixture and machine environment metadata. Do not run destructive production deploy. Define SLO candidates from actual baselines rather than fictional numbers.

## Issue 00.4: Architectural convergence decision

Write compact ADR only if existing policy merits one: identify actual canonical records and which components need extensions, duplication to remove, incremental graph boundaries, evidence semantics, enforcement owner, projections owner and migration plan. Classify proposals as REUSE, EXTEND, MERGE, DELETE or NEW (only when no suitable existing mechanism exists). Add milestones/issues through existing plan system, synchronize to GitHub where conventional. List explicit anti-goals and feature freeze for unrelated scope.

## Issue 00.5: Audit acceptance and report

Run current gate suite on unchanged head and record failures honestly. Evidence deliverables: SHA, timestamp, PR census coverage, benchmark artifacts, gate/branch-rule audit, current architecture map, worklist with exact linked issues, baseline report, risk matrix and roll-forward strategy. No claim of implementation progress if only audited. Stop only if externally blocked; report why and next executable command. Continue to 01 only after baseline evidence exists.
