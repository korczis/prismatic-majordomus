# Milestone 03: Rock-solid reconcile / drain actuator with policy and idempotence

**Priority:** P0


# Universal execution contract (applies to every mission)
Repository: https://github.com/korczis/prismatic-majordomus . Target the actual current HEAD, not an imagined architecture. Begin by reading AGENTS.md, .ai/ (especially rules, doctrines, schema, project and CI model), .majordomus/ if present, ADRs, docs/INTEGRATION.md, docs/CI.md, capabilities, prs implementation, GitHub Actions, tests, generated registries and release/rollout contracts. Discover rather than presume paths/commands. Respect owner policy and all restrictions. Current evidence (2026-10-10): 52 open PRs observed; .ai/repo/ci/queue.yaml says measurement only and approval protects merges; .github/workflows/queue.yml runs refresh/status every 30 min; existing derived merge driver, prs drain, CI planner, release/version, Pages, fleet are present. The 2026-09-11 attachment is historical, NOT evidence of current implementation.

DO NOT replace working subsystems. DO NOT create an unreviewed second queue, second scheduler, new GitHub bot, hardcoded registry, duplicated version writer, parallel deployment system, local-only workflow or shallow test. Derive from canonical typed data using existing capability/effect/projection architecture. Honor CLI/API/OpenAPI/MCP/Cockpit/UI, docs and generators. Never assume GitHub CONFLICTING is definitive: verify through git + configured merge.derived driver; never assume local merge is sufficient for protected remote integration. Changes must obey branch protection, GitHub permissions, required status contexts, approvals, security boundaries, provider policy, production change authority and existing queue policy. Fail closed on unknowns. Do not broaden HTTP/MCP mutation authority as a side effect.

Each independent implementation increment must have: baseline evidence, hypothesis, smallest vertical change, unit + integration + adversarial negative tests, schema/governance gate, generated drift check, docs, cross-surface parity where relevant, exact commands/results, commits, measurable before/after. No passing-by-mocking-away-the-real-boundary. No changing tests to mask failures. No claiming success based only on PR text. Prefer regression-first and prove that test fails before fix where feasible. Ask operator only for truly required policy/authorization/credentials; proceed with safe nonmutating work independently. Report blockers and resume work where possible. Explicit permissions matter: a prompt never overrides branch protection or revives a forbidden operation.

Work with repository-owned issues/milestones/intents and existing sync tooling. One mission = one milestone where the tooling supports it; sections = granular issues with acceptance criteria. Link actual PRs, commits and evidence. No invented tickets. Use .ai/ to enforce durable generic rules suitable for consumer repos, not case-specific fixture data. Do not leave lingering branches/worktrees. No more than one integrating master writer, with compare-and-swap, fencing or existing proven lease mechanisms. Recover from cancellation and retries. Continue through delivery phases within actual permissions; do not stop at a design memo. Upon blockage, leave actionable trace and continue independent parts.

Completion gate: quality/build/tests/schema/derivation/security/CI status checked; actual change merged into canonical master under policy; post-merge master verified; version obligation computed with repository writer; release only when correct authorized tag, assets and provenance exist; Pages/production deployment and fleet update executed where authorized; smoke/readiness/rollback evidence recorded; stale worktrees cleaned only when provably safe. Distinguish merged, released, deployed, rolled out and verified. Output exact URLs/commit SHAs/artifact digests/version and failure reasons, never fabricated outcomes.

## Implementation scope

### Work item 1: implement and prove

Audit current prs drain and merge implementation line by line. Extend, do not fork. Closed loop observe→classify→select→prepare→verify→mutate one bounded action→reobserve, until no authorized action. Use existing transaction/lease mechanism with fencing or atomic equivalent and server-side GitHub conflict protections; ensure crashed actor can be recovered.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 2: implement and prove

Bind every decision to head/base/tree SHA and required CI verdict + approvals + branch rules; check again immediately before mutation. Stop on unfamiliar protection and reject stale statuses. Separate optional local preparation (safe) from remote merges (policy-owned). Comply with ADR 0101 approval behind merge: introduce explicit typed approval/opt-in mode through canonical policy, default observation / safe preparation only until authorization, never silently enable unsanctioned merge.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 3: implement and prove

Return typed reason and clear exit semantics consistent with current codes 0/10/12. Retries have bounded backoff and jitter, avoid duplicate side effects; transient API errors don't become 'empty queue'. Journal intent, observed facts, attempted action, proof, outcome. Concurrency two actors/externally moved master/lease expiry/late CI result / network partition are regression tests.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

### Work item 4: implement and prove

Acceptance: controlled sandbox demonstrates N ready changes drained safely, with preexisting safety unchanged.

**Done when:** change is implemented or explicitly evidenced unnecessary in current HEAD; focused negative/positive tests and canonical checks pass; exact proof recorded.

## Completion report
Proof: base and head SHA, linked canonical intent/issues, changed files, tests that failed before/passed after, gate checks, cross-surface derived coverage, authorization state, PR link, merge/deploy state, measured throughput effect, next actionable task. If not landed, report blocked rather than done.
