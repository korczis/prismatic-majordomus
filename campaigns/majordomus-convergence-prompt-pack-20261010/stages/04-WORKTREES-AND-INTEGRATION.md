# 04 — Safe worktree reconciliation and serial integration

## Objective
Prove that stale branches/merged worktrees are classified safely and ready PRs converge to a small number of integrated changes. No unverified deletion and no unchecked auto-merge.

## Tasks
1. Inspect queue/reconcile architecture, ADR 0021, 0101, Git/GitHub sync rules and PR #882. Reuse existing typed worktree/branch state; do not add a duplicate DB of worktrees.
2. Implement/verify a **dry-run-first** reconciliation command with typed status, provenance, protected paths, git common-dir identity, linked branch ownership, dirty/untracked and active process/session/claim checks, stale remote status and upstream divergence.
3. Destructive cleanup is permitted only for proven safe candidates: integrated content on target SHA, no unique reachable commits, no uncommitted work including ignored-but-valued data, no active peer/session, repository policy met. Prefer quarantine/reversible cleanup with audit log where feasible. Unknown never deleted.
4. Rebase stale but valuable candidates with conflict reports; do not silently resolve logical conflicts by `ours/theirs`. Use an explicit compare against the latest target.
5. Normalize PR semantic diffs: keep generated changes reproducible and deterministic; stage derived updates with the canonical generator and ownership lock. A change may not forge or orphan provenance.
6. Serialize merges against the current master SHA, verify current app-bound required checks, branch policies, review requirements, conflicts, fast-forward/rebase policy and post-merge validation. Handle race: acquire integration lane, refresh base and checks and abort if changed.
7. Classify obsolete and superseded PRs with linked evidence; closing without merging requires preserving unique useful changes. Remove stale branches only after repository-approved audit.

## Proof
Test 15+ adversarial Git topologies: dirty worktree, detached HEAD, linked worktree, gone origin, rebased commit, squash merge, cherry-pick, partial generated drift, pending CI, stale SHA, force-pushed PR, two agents racing, active session, locked worktree, foreign repo, dangling untracked file. Prove 0 false-safe deletions and deterministic decisions; measure open PR WIP and time-to-integration before/after.

## Operating contract (mandatory in every phase)
- Work against the repository checked out locally. Treat 2026-10-10 audit as leads, NOT a live truth. Fetch and inspect origin, HEAD, branches, PR checks, CI, tags, manifests, ADRs, .ai/**, docs and actual implementation before changes.
- Repo identity: `korczis/prismatic-majordomus`; detect the actual default branch, do not blindly assume master if it changed.
- Follow AGENTS.md and .ai/ governance; reuse existing capabilities, schemas, rules, registries, evidence ledger and commands. No independent truth in MCP, CLI, HTTP, OpenAPI, Cockpit, website, generated documents or shell wrappers.
- Read applicable existing ADRs and test architecture; amend rather than duplicate. Reject parallel control planes, synthetic statuses, mock production data, optimistic health, stale proof and manual inventories.
- Every issue/step has: invariant, owner/source of truth, scope, tests, diagnostic, acceptance proof, and merge/release impact. Attach work to existing intent/milestone/issue if possible; create only if needed, respecting ID allocation and sync rules.
- Make a reversible baseline: `git status`, `git rev-parse HEAD`, `git worktree list --porcelain`, branches, open PRs, CI runs, version, release, existing runtime state. Never reset, delete worktrees or force push without verifying ownership, merged state and no valuable uncommitted work.
- Implementation sequence: inspect → reproduce → design minimum compatible change → write failing test → implement → targeted checks → cross-surface contract checks → docs/evidence → integration. Never report green when unknown.
- Changes must be reviewed against actual `master`/default branch, use ephemeral branches/worktrees, small semantic deltas, deterministic generated output. De-duplicate generated changes where practical; no bulk unrelated formatting.
- Safety: do not change remote secrets, production deployments, destructive Git refs or branch protection silently. For explicitly authorized autonomous release/deploy actions apply repo policy, verifiable gates, least privilege, atomic rollback and fail closed; stop on missing credentials/permissions rather than bypass safeguards.
- If external advisors/providers are unavailable, continue locally without degrading evidence requirements. Do not promise that a plan/review/test passed unless it actually ran and can be linked to commit/digest.
- No time-box theater: optimize for verified closure, not task count, LOC, number of prompts, or a fabricated speed claim.
- Before final report: compile; focused and impacted tests; model/schema/derive drift checks; smoke clean checkout; CI status and commit SHA; security/resource check if applicable; update documentation; enumerate unresolved blockers precisely.

## Mandatory report per stage
1. Baseline HEAD SHA, release/tag, relevant PR/issues/CI links (actual values, not invented).
2. Root cause with file:function or line references and a reproducible failure; state explicit unknowns.
3. Changes made, boundaries preserved, migrations and deletions of legacy code.
4. Evidence matrix: claim → automated test/gate/run → revision/digest → outcome.
5. Numerical before/after if measured; otherwise `not measured`.
6. Artifacts/commit(s)/PR(s), remaining blockers, next runnable stage.
