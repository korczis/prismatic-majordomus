# NEXUS 17: Ci Delivery And Release Train

**Lane:** hardening  
**Dependencies:** 16  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Integrate the completed Nexus feature into the actual trunk/release/deployment machinery **safely and quickly**, using as few PRs as repository constraints permit.

## Execute
1. Verify current base/master SHA, branch protection and CI requirements, active merge train, versioning/release policy, site deployment dependencies and branch/worktree cleanliness.
2. Consolidate implementation into coherent PRs (goal 3–5 in total for the mission, unless actual repository constraints dictate otherwise). Keep commit history understandable; don't spread 18 prompts over 18 PRs.
3. Confirm stage 16 E2E acceptance passed on the candidate SHA. Run canonical full quality gates, focused/integration tests, generated drift checks, use-case impact/coverage, docs/site checks, security tests and release verification again on the final SHA. Read diagnostics even when a command exits 0.
4. Fix actual failures; distinguish current-basis regressions from stale CI history. If CI is queued and not run, it is NOT green. Never disable gate to push through an arbitrary deadline.
5. Rebase using existing rebaser/integration skill and scoped worktrees, verify after rebase; preserve peer claims and avoid active write conflicts.
6. Apply version bump/changelog/release record dictated by actual semantics and project policy. Push the final SHA; create/refresh PR with clear evidence and issue linkage; request required review through approved channels.
7. Where auto-merge/deploy is authorized and checks actually passed, land and verify trunk HEAD, relevant runtime endpoint and published Pages. Where external approval is required, leave crisp, persistent `blocked` status and complete all nonblocked tasks.

## Required proof
- For each PR: link, head SHA, tested SHA, CI run/status, branch protection status and merge commit if merged.
- For deployment: actual revision and successful endpoint probe, never guessed URL or claimed live state.
- No stale generated artifacts, no orphan worktrees/issues, no uncommitted code relevant to delivery.

## Failure traps
Do not force push protected master, bypass approvals, count queued CI as success, or call uploaded artifacts 'deployed'.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
