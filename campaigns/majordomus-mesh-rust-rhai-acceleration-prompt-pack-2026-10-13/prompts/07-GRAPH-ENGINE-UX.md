# 07 / GRAPH INTELLIGENCE

**Mission:** Improve existing canonical graphs with performance, provenance, deterministic layout, drilldown and mobile-accessible UX.

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


## Surface archaeology

Inspect graph::DERIVATIONS, canonical graph schema, existing Cytoscape, Apache ECharts if any, Cockpit routing, published graph snapshot, graph API/MCP and UI docs. Inventory exact graph families rather than inventing them. For every graph enumerate canonical nodes/edges, source, serialization, cached projection, renderer, accessible fallback, authorization and live update channel.

## Canonical graph model

Stable canonical IDs and URLs, typed node/edge kinds, provenance, source SHA, time/freshness, status, source of evidence and deterministic semantic order. No synthetic entity or hardcoded UI categorization. Establish single add-graph workflow that propagates registry→API→MCP→Cockpit→GH Pages→tests, under exposure policy.

## Interaction vertical

Search, focus, 1-hop/2-hop expansion, inbound/outbound edge filters, node details, source/test/claim backlinks, selection encoded in URLs, keyboard navigation, reduced motion, legible legends and informative empty/error/stale states. Mobile layout and accessible no-JS representation. Existing complete accessible graph must not silently shrink into truncated text.

## Performance vertical

Measure derive, serialization, server TTFB, JSON parse, layout, interaction response, memory and UI paint on actual graphs and synthetic sizes. Cache immutable graph snapshot keyed by canonical input/revision, invalidate correctly, precompute stable layouts if justified, avoid whole graph re-layout on small event changes, use client lazy loading for heavy libraries. Benchmark cold/warm and regression gates.

## Live consistency

For mesh/execution graphs use canonical versioned snapshot+event stream with cursor, gap detection, reconnection and resync. Handle duplicated/out-of-order events, removed peers, offline/unknown state and authorization changes. Browser is a projection, never authority.

## Graph products

Prioritize evidence graph (claim→implementation→test→CI), PR dependency graph (PR→blocker→run), plugin graph (package→capability→transport→execution→evidence), mesh graph (worktree→peer→claim), delivery graph (intent→PR→commit→tag→deploy). Only implement families with real canonical input data and measurable value.

## Acceptance and release slices

G1 baseline; G2 stable deep-link IDs/provenance; G3 useful selection/search/detail; G4 large-graph performance; G5 live recovery; G6 mobile+accessibility; G7 plugin/release graph support. Ship each independently after tests and benchmarks, linking actual deployment revision.

## REQUIRED EXIT REPORT (MACHINE-READABLE + HUMAN SUMMARY)

Output a structured, canonical report containing: baseline_sha, final_sha, active_claim, milestones, issues, PRs, verified fixes, commands/results, test failures, benchmarks (baseline/after), version/tag, deployment_url and deployed_sha, derived artifacts, migration notes, blockers, risk, and next ready task. Do not invent missing fields; represent unknown explicitly.
