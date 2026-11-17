# Majordomus PR Throughput: closed-loop integration and release

**Snapshot:** 2026-10-10; authoritative source is always live `korczis/prismatic-majordomus` HEAD. **Role:** executable Claude Code / Opus / Fable mission pack, not a claim of implementation. This archive changes no repository by itself.

## Mission
Reduce time from completed work to verified, deployed value without reducing evidence quality or governance. Current queue observed 52 open PRs. Existing `.ai/repo/ci/queue.yaml` and GitHub `queue.yml` explicitly measure but never autonomously drain; current policy reserves merge for approval, so implementation must resolve that boundary intentionally rather than silently bypassing it. GitHub mergeability is misleading when local merge needs the repository's `merge.derived` driver.

## Start
Open the real repo in Claude Code, feed `00-ORCHESTRATOR.md` verbatim with access to this entire pack. Process mission files in dependency order. Each mission yields a milestone; granular numbered sections yield issues, using actual `.ai/` project tooling. Prioritize getting canonical observe/explain/drain tested before spending time on design polish. Iteratively land small reviewed slices and measure. **Safe by default:** until policy explicitly authorizes remote merges, actor may read and safely prepare only. Owner approvals/branch protection/security rules cannot be bypassed by these prompts.

## Dependencies
00 orchestrator → 01 audit → 02 domain → 03 drain → 04 stack → 05 automation → 06 CI → 07 lifecycle → 08 projections → 09 hardening → 10 dogfood → 11 release → 12 fleet. In practice, missions 04/06 and 07/08 can proceed in parallel after the canonical contracts stabilize, if worktree conflict budgets permit. Harden relevant features continuously, not only in mission 09.

## Metrics
Capture observed baseline and after: PR age distribution, actionable backlog, merged/day, p50/p95 lead time, CI path time, master verification lag, deployment lag, rollout convergence. Targets are hypotheses to validate against measurement, not fabricated SLOs. Queue can't be called 'drained' if silently blocked.

## Release truth
`implemented != merged != released != deployed != rolled_out != verified`. Track each separately with URLs, SHAs, versions and evidence. No secret exposure. No hardcoded version. No forced pushes to protected branches. If no authorization, publish exact stop condition and proceed with safe work.

## Contents
- `00-ORCHESTRATOR.md`: autonomous execution contract and stages
- `missions/01...12`: milestone-sized deep prompts
- `CONTRACTS.md`: typed behavior, failure modes and gates
- `ACCEPTANCE.md`: proof matrix
- `OPERATIONS.md`: safe execution, emergency stop, rollback and day-2 operations
- `MANIFEST.json`: files and checksums
