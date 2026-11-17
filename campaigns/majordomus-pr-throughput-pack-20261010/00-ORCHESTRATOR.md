# Claude Code / Opus / Fable: execute the entire Majordomus PR throughput pack

You are a repository engineer and release integrator inside the live `korczis/prismatic-majordomus` checkout. Use current HEAD, existing governance and project workflows. Your task is to *implement and operate* a continuously draining integration system, verify it, deliver its derived surfaces and docs, release a new version using current policy, deploy Pages and roll it out through the existing fleet mechanism where authorized. Do not stop at a report. Do not assume any deadline, title, version or capability not evidenced in source. Use the provided mission files as a dependency-constrained backlog. Read ALL of them before execution, then act incrementally.


# Universal execution contract (applies to every mission)
Repository: https://github.com/korczis/prismatic-majordomus . Target the actual current HEAD, not an imagined architecture. Begin by reading AGENTS.md, .ai/ (especially rules, doctrines, schema, project and CI model), .majordomus/ if present, ADRs, docs/INTEGRATION.md, docs/CI.md, capabilities, prs implementation, GitHub Actions, tests, generated registries and release/rollout contracts. Discover rather than presume paths/commands. Respect owner policy and all restrictions. Current evidence (2026-10-10): 52 open PRs observed; .ai/repo/ci/queue.yaml says measurement only and approval protects merges; .github/workflows/queue.yml runs refresh/status every 30 min; existing derived merge driver, prs drain, CI planner, release/version, Pages, fleet are present. The 2026-09-11 attachment is historical, NOT evidence of current implementation.

DO NOT replace working subsystems. DO NOT create an unreviewed second queue, second scheduler, new GitHub bot, hardcoded registry, duplicated version writer, parallel deployment system, local-only workflow or shallow test. Derive from canonical typed data using existing capability/effect/projection architecture. Honor CLI/API/OpenAPI/MCP/Cockpit/UI, docs and generators. Never assume GitHub CONFLICTING is definitive: verify through git + configured merge.derived driver; never assume local merge is sufficient for protected remote integration. Changes must obey branch protection, GitHub permissions, required status contexts, approvals, security boundaries, provider policy, production change authority and existing queue policy. Fail closed on unknowns. Do not broaden HTTP/MCP mutation authority as a side effect.

Each independent implementation increment must have: baseline evidence, hypothesis, smallest vertical change, unit + integration + adversarial negative tests, schema/governance gate, generated drift check, docs, cross-surface parity where relevant, exact commands/results, commits, measurable before/after. No passing-by-mocking-away-the-real-boundary. No changing tests to mask failures. No claiming success based only on PR text. Prefer regression-first and prove that test fails before fix where feasible. Ask operator only for truly required policy/authorization/credentials; proceed with safe nonmutating work independently. Report blockers and resume work where possible. Explicit permissions matter: a prompt never overrides branch protection or revives a forbidden operation.

Work with repository-owned issues/milestones/intents and existing sync tooling. One mission = one milestone where the tooling supports it; sections = granular issues with acceptance criteria. Link actual PRs, commits and evidence. No invented tickets. Use .ai/ to enforce durable generic rules suitable for consumer repos, not case-specific fixture data. Do not leave lingering branches/worktrees. No more than one integrating master writer, with compare-and-swap, fencing or existing proven lease mechanisms. Recover from cancellation and retries. Continue through delivery phases within actual permissions; do not stop at a design memo. Upon blockage, leave actionable trace and continue independent parts.

Completion gate: quality/build/tests/schema/derivation/security/CI status checked; actual change merged into canonical master under policy; post-merge master verified; version obligation computed with repository writer; release only when correct authorized tag, assets and provenance exist; Pages/production deployment and fleet update executed where authorized; smoke/readiness/rollback evidence recorded; stale worktrees cleaned only when provably safe. Distinguish merged, released, deployed, rolled out and verified. Output exact URLs/commit SHAs/artifact digests/version and failure reasons, never fabricated outcomes.

## Critical operating strategy
(1) Establish immutable baseline of head SHA, queue snapshot, actual branch protection, runner/CI timings and release/deploy versions. (2) Audit prior ADR 0101 approval contract: changing automation scope is a GOVERNANCE DECISION, not refactor. Formalize an opt-in policy as required; do not silently switch schedule from read-only to writes. (3) Map existing modules, reuse `prs` subsystem. (4) Start smallest safe vertical slice: reconcile can explain and safely select one candidate, simulate in isolated git fixtures. (5) Land through existing PR/CI/master route with actual approval and evidence. (6) Iterate all missions. (7) On each merge, recalculate queue and verify release obligation; batch releases using canonical contract rather than tagging every PR without permission. (8) Complete Pages, docs/demo, fleet rollout and smoke; report real outcomes only.

## Delegation
Delegate bounded investigation, domain implementation, UI, test and docs subtasks to independent workers only if available. Avoid competing writes to same branch/master and one canonical integration lease for merge actions. Never make process parallelism an excuse for multiplying half-finished PRs. When review providers absent, run local checks, record reduced assurance explicitly. No waiting for user to choose obvious code design. Escalate only protected remote mutation consent, missing secrets, security overrides, irreducible product decision or access blockers; continue safe tasks.

## Continuous reporting
At each completed mission: root cause; design delta; changed files; tests+CI verdict, SHA/PR, baseline delta, regressions, known blockers, next automatic action. Maintain live task/issue graph via existing `.ai/` conventions. No inflated completeness percentages. Do not claim a release, Pages publication or fleet convergence until observed externally.

## Stop conditions
Stop dangerous mutations immediately on changed master lease, stale check, missing approval, unreadable branch protection, unauthorized credentials, unexpected dirty worktree, failed merge driver, unexpected side effects, unknown CI verdict, exposed secrets or failed post-merge verification. Restore/read current state and report. Continue unrelated read/test/docs work safely.

## Success
A real controlled end-to-end path creates change, verifies, integrates under approved policy, proves master, computes/bump version through canonical writer, performs authorized tag+release, deploys derived Pages, safely rolls out fleet, verifies healthy runtime and cleans only proven disposable work, while reducing actual open actionable PR inventory. The entire pipeline is discoverable and explainable via CLI/API/MCP (read-only as effect policy requires), Cockpit, `majordomus ui`, docs and .ai enforcement.

## Sequence
01. Read and execute `missions/01-01-audit-and-baseline.md`.
02. Read and execute `missions/02-02-canonical-state-and-graph.md`.
03. Read and execute `missions/03-03-safe-actuator.md`.
04. Read and execute `missions/04-04-stack-rebase-and-cleanup.md`.
05. Read and execute `missions/05-05-event-driven-github.md`.
06. Read and execute `missions/06-06-ci-throughput.md`.
07. Read and execute `missions/07-07-admission-and-session.md`.
08. Read and execute `missions/08-08-surface-projections.md`.
09. Read and execute `missions/09-09-governance-hardening.md`.
10. Read and execute `missions/10-10-demo-and-backlog.md`.
11. Read and execute `missions/11-11-version-release.md`.
12. Read and execute `missions/12-12-fleet-rollout-and-proof.md`.
