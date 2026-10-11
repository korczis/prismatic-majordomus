# Milestone 08: CLI/API/OpenAPI/MCP/Cockpit/UI derived integration UX

**Priority:** P1


# Universal execution contract (applies to every mission)
Repository: https://github.com/korczis/prismatic-majordomus . Target the actual current HEAD, not an imagined architecture. Begin by reading AGENTS.md, .ai/ (especially rules, doctrines, schema, project and CI model), .majordomus/ if present, ADRs, docs/INTEGRATION.md, docs/CI.md, capabilities, prs implementation, GitHub Actions, tests, generated registries and release/rollout contracts. Discover rather than presume paths/commands. Respect owner policy and all restrictions. Current evidence (2026-10-10): 52 open PRs observed; .ai/repo/ci/queue.yaml says measurement only and approval protects merges; .github/workflows/queue.yml runs refresh/status every 30 min; existing derived merge driver, prs drain, CI planner, release/version, Pages, fleet are present. The 2026-09-11 attachment is historical, NOT evidence of current implementation.

DO NOT replace working subsystems. DO NOT create an unreviewed second queue, second scheduler, new GitHub bot, hardcoded registry, duplicated version writer, parallel deployment system, local-only workflow or shallow test. Derive from canonical typed data using existing capability/effect/projection architecture. Honor CLI/API/OpenAPI/MCP/Cockpit/UI, docs and generators. Never assume GitHub CONFLICTING is definitive: verify through git + configured merge.derived driver; never assume local merge is sufficient for protected remote integration. Changes must obey branch protection, GitHub permissions, required status contexts, approvals, security boundaries, provider policy, production change authority and existing queue policy. Fail closed on unknowns. Do not broaden HTTP/MCP mutation authority as a side effect.

Each independent implementation increment must have: baseline evidence, hypothesis, smallest vertical change, unit + integration + adversarial negative tests, schema/governance gate, generated drift check, docs, cross-surface parity where relevant, exact commands/results, commits, measurable before/after. No passing-by-mocking-away-the-real-boundary. No changing tests to mask failures. No claiming success based only on PR text. Prefer regression-first and prove that test fails before fix where feasible. Ask operator only for truly required policy/authorization/credentials; proceed with safe nonmutating work independently. Report blockers and resume work where possible. Explicit permissions matter: a prompt never overrides branch protection or revives a forbidden operation.

Work with repository-owned issues/milestones/intents and existing sync tooling. One mission = one milestone where the tooling supports it; sections = granular issues with acceptance criteria. Link actual PRs, commits and evidence. No invented tickets. Use .ai/ to enforce durable generic rules suitable for consumer repos, not case-specific fixture data. Do not leave lingering branches/worktrees. No more than one integrating master writer, with compare-and-swap, fencing or existing proven lease mechanisms. Recover from cancellation and retries. Continue through delivery phases within actual permissions; do not stop at a design memo. Upon blockage, leave actionable trace and continue independent parts.

Completion gate: quality/build/tests/schema/derivation/security/CI status checked; actual change merged into canonical master under policy; post-merge master verified; version obligation computed with repository writer; release only when correct authorized tag, assets and provenance exist; Pages/production deployment and fleet update executed where authorized; smoke/readiness/rollback evidence recorded; stale worktrees cleaned only when provably safe. Distinguish merged, released, deployed, rolled out and verified. Output exact URLs/commit SHAs/artifact digests/version and failure reasons, never fabricated outcomes.

## Implementation scope

### Work item 1: implement and prove

Use existing capability registry, typed Rust serializers, OpenAPI schema generator and Cockpit design system. One canonical integration projection powers CLI status/explain/graph/drain plan, JSON, HTTP read endpoints, OpenAPI/Swagger, MCP read tools/resources, high-level `majordomus ui`, Cockpit mobile-first flow, docs' diagrams/figures. Do NOT expose remote merge/delete/SSH capabilities over HTTP/MCP if effect model prohibits them; local action buttons only when actor authorized and actionable, with CSRF/auth protection, confirmations for destructive operations and server revalidation. UX shows queue stages, active writer, blockers, dependency graph, critical path, next action/actor, CI freshness, master SHA, progress, recent merges, release and rollout status.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 2: implement and prove

No fake counts or separate frontend sorting algorithm. Chart metrics with explicit denominators and timestamps; mobile drawers and keyboard/a11y tests. Browser+API+CLI parity contracts from one fixture. Explain why GitHub conflict badge may differ from local derived merge.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 3: implement and prove

Performance/caching/invalidation tests, disconnected/offline graceful states. Zero consumer registration for new PR/domain instance. Avoid duplicating low-level Cockpit in high-level UI.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

## Completion report
Proof: base and head SHA, linked canonical intent/issues, changed files, tests that failed before/passed after, gate checks, cross-surface derived coverage, authorization state, PR link, merge/deploy state, measured throughput effect, next actionable task. If not landed, report blocked rather than done.
