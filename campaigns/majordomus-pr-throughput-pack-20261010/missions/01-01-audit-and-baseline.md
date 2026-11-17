# Milestone 01: Audit real queue and critical path

**Priority:** P0


# Universal execution contract (applies to every mission)
Repository: https://github.com/korczis/prismatic-majordomus . Target the actual current HEAD, not an imagined architecture. Begin by reading AGENTS.md, .ai/ (especially rules, doctrines, schema, project and CI model), .majordomus/ if present, ADRs, docs/INTEGRATION.md, docs/CI.md, capabilities, prs implementation, GitHub Actions, tests, generated registries and release/rollout contracts. Discover rather than presume paths/commands. Respect owner policy and all restrictions. Current evidence (2026-10-10): 52 open PRs observed; .ai/repo/ci/queue.yaml says measurement only and approval protects merges; .github/workflows/queue.yml runs refresh/status every 30 min; existing derived merge driver, prs drain, CI planner, release/version, Pages, fleet are present. The 2026-09-11 attachment is historical, NOT evidence of current implementation.

DO NOT replace working subsystems. DO NOT create an unreviewed second queue, second scheduler, new GitHub bot, hardcoded registry, duplicated version writer, parallel deployment system, local-only workflow or shallow test. Derive from canonical typed data using existing capability/effect/projection architecture. Honor CLI/API/OpenAPI/MCP/Cockpit/UI, docs and generators. Never assume GitHub CONFLICTING is definitive: verify through git + configured merge.derived driver; never assume local merge is sufficient for protected remote integration. Changes must obey branch protection, GitHub permissions, required status contexts, approvals, security boundaries, provider policy, production change authority and existing queue policy. Fail closed on unknowns. Do not broaden HTTP/MCP mutation authority as a side effect.

Each independent implementation increment must have: baseline evidence, hypothesis, smallest vertical change, unit + integration + adversarial negative tests, schema/governance gate, generated drift check, docs, cross-surface parity where relevant, exact commands/results, commits, measurable before/after. No passing-by-mocking-away-the-real-boundary. No changing tests to mask failures. No claiming success based only on PR text. Prefer regression-first and prove that test fails before fix where feasible. Ask operator only for truly required policy/authorization/credentials; proceed with safe nonmutating work independently. Report blockers and resume work where possible. Explicit permissions matter: a prompt never overrides branch protection or revives a forbidden operation.

Work with repository-owned issues/milestones/intents and existing sync tooling. One mission = one milestone where the tooling supports it; sections = granular issues with acceptance criteria. Link actual PRs, commits and evidence. No invented tickets. Use .ai/ to enforce durable generic rules suitable for consumer repos, not case-specific fixture data. Do not leave lingering branches/worktrees. No more than one integrating master writer, with compare-and-swap, fencing or existing proven lease mechanisms. Recover from cancellation and retries. Continue through delivery phases within actual permissions; do not stop at a design memo. Upon blockage, leave actionable trace and continue independent parts.

Completion gate: quality/build/tests/schema/derivation/security/CI status checked; actual change merged into canonical master under policy; post-merge master verified; version obligation computed with repository writer; release only when correct authorized tag, assets and provenance exist; Pages/production deployment and fleet update executed where authorized; smoke/readiness/rollback evidence recorded; stale worktrees cleaned only when provably safe. Distinguish merged, released, deployed, rolled out and verified. Output exact URLs/commit SHAs/artifact digests/version and failure reasons, never fabricated outcomes.

## Implementation scope

### Work item 1: implement and prove

Inventory every open PR with base/head, age, draft, GitHub mergeability, local derived-driver mergeability, CI status freshness, branch rules, approvals, commits, diff footprint, linked issue/intent, worktree, declared dependencies, and proof of duplicate/absorption. Construct dependency DAG, compare master SHA and identify fan-out roots and observable merge blockers. Inspect docs/INTEGRATION.md and ADR 0101 before touching acting semantics. Trace why queue.yml only observes and who authorized it.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 2: implement and prove

Locate prs refresh/status/drain/cleanup and current contracts, tests, exit codes, durable journals and explicit human approvals. Inspect CI duration data, queue run timings, busy runner groups, scheduler concurrency and actual newest master behavior. Establish baseline metrics: actionable count, genuinely blocked count, oldest actionable, p50/p95 created-to-merge (only if data sufficient), integrated/day, CI queue/run time, stale count, dependency depth, version/deploy delay. Produce executable JSON report from existing model, not hand-curated lists.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 3: implement and prove

Propose phased migration that preserves current approval guarantees. Acceptance: audit reproducible; no PR mutated; every unknown explicitly marked; before snapshots attached to milestone.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

## Completion report
Proof: base and head SHA, linked canonical intent/issues, changed files, tests that failed before/passed after, gate checks, cross-surface derived coverage, authorization state, PR link, merge/deploy state, measured throughput effect, next actionable task. If not landed, report blocked rather than done.
