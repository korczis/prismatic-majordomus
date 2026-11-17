# Milestone 12: Deployment, fleet rollout, smoke, rollback, final proof

**Priority:** P0 release


# Universal execution contract (applies to every mission)
Repository: https://github.com/korczis/prismatic-majordomus . Target the actual current HEAD, not an imagined architecture. Begin by reading AGENTS.md, .ai/ (especially rules, doctrines, schema, project and CI model), .majordomus/ if present, ADRs, docs/INTEGRATION.md, docs/CI.md, capabilities, prs implementation, GitHub Actions, tests, generated registries and release/rollout contracts. Discover rather than presume paths/commands. Respect owner policy and all restrictions. Current evidence (2026-10-10): 52 open PRs observed; .ai/repo/ci/queue.yaml says measurement only and approval protects merges; .github/workflows/queue.yml runs refresh/status every 30 min; existing derived merge driver, prs drain, CI planner, release/version, Pages, fleet are present. The 2026-09-11 attachment is historical, NOT evidence of current implementation.

DO NOT replace working subsystems. DO NOT create an unreviewed second queue, second scheduler, new GitHub bot, hardcoded registry, duplicated version writer, parallel deployment system, local-only workflow or shallow test. Derive from canonical typed data using existing capability/effect/projection architecture. Honor CLI/API/OpenAPI/MCP/Cockpit/UI, docs and generators. Never assume GitHub CONFLICTING is definitive: verify through git + configured merge.derived driver; never assume local merge is sufficient for protected remote integration. Changes must obey branch protection, GitHub permissions, required status contexts, approvals, security boundaries, provider policy, production change authority and existing queue policy. Fail closed on unknowns. Do not broaden HTTP/MCP mutation authority as a side effect.

Each independent implementation increment must have: baseline evidence, hypothesis, smallest vertical change, unit + integration + adversarial negative tests, schema/governance gate, generated drift check, docs, cross-surface parity where relevant, exact commands/results, commits, measurable before/after. No passing-by-mocking-away-the-real-boundary. No changing tests to mask failures. No claiming success based only on PR text. Prefer regression-first and prove that test fails before fix where feasible. Ask operator only for truly required policy/authorization/credentials; proceed with safe nonmutating work independently. Report blockers and resume work where possible. Explicit permissions matter: a prompt never overrides branch protection or revives a forbidden operation.

Work with repository-owned issues/milestones/intents and existing sync tooling. One mission = one milestone where the tooling supports it; sections = granular issues with acceptance criteria. Link actual PRs, commits and evidence. No invented tickets. Use .ai/ to enforce durable generic rules suitable for consumer repos, not case-specific fixture data. Do not leave lingering branches/worktrees. No more than one integrating master writer, with compare-and-swap, fencing or existing proven lease mechanisms. Recover from cancellation and retries. Continue through delivery phases within actual permissions; do not stop at a design memo. Upon blockage, leave actionable trace and continue independent parts.

Completion gate: quality/build/tests/schema/derivation/security/CI status checked; actual change merged into canonical master under policy; post-merge master verified; version obligation computed with repository writer; release only when correct authorized tag, assets and provenance exist; Pages/production deployment and fleet update executed where authorized; smoke/readiness/rollback evidence recorded; stale worktrees cleaned only when provably safe. Distinguish merged, released, deployed, rolled out and verified. Output exact URLs/commit SHAs/artifact digests/version and failure reasons, never fabricated outcomes.

## Implementation scope

### Work item 1: implement and prove

Inspect actual fleet declaration and typed fleet plan/status/rollout, mesh readiness, local/out-of-repo consumers, installed CLI self-update and existing transport effect restrictions. Use canonical rollout plan after release verified; require authorized remote mutation and approved SSH hosts/keys, verify release signature/artifacts, update configured machines in batches with bounded concurrency and explicit rollback strategy compatible with existing release/service model. Verify per-machine installed and served versions, launchd/systemd restart health, /api/v1/ready, runtime lease ownership, mesh peer discovery, Cockpit/UI/API/MCP ability, docs Pages, external consumer repo install/update. Partial fleet convergence must remain partial, never green; isolated/unreachable machine has actionable reason.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 2: implement and prove

Preserve prior working version on failed rollout or run repository's supported rollback, validate resumed idempotent converge. Run complete regression quality gates, Github CI pass, real smoke, publish rollout evidence and after/before queue metrics. Final report as evidence ledger with commits, PR URLs, tag, artifact digests, CI links, deployment/runtime versions, actual test counts, remaining blockers and operational next action. Demonstrate end-to-end PR→master→tag→deploy→fleet on controlled sample.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 3: implement and prove

Don't mark DONE until evidence truly exists.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

## Completion report
Proof: base and head SHA, linked canonical intent/issues, changed files, tests that failed before/passed after, gate checks, cross-surface derived coverage, authorization state, PR link, merge/deploy state, measured throughput effect, next actionable task. If not landed, report blocked rather than done.
