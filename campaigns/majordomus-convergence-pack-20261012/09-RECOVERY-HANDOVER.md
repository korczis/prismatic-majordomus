# PROMPT 09 — Crash recovery, context compaction, interrupted agents

Goal: let another Claude session resume safely without rerunning destructive work or trusting a stale summary.

## Checkpoint contract
Save minimal structured checkpoint via existing campaign/session-context system if present. Include mission ID, timestamp, repo SHA, current branch/worktree, dirty changes, PR IDs and head SHAs, action idempotency keys, validation SHA/results, deployment identity, active task and next safe action. Store secrets nowhere.

## On resume
1. Read current policies then checkpoint. Compare current git refs, PR heads, CI results and deployed state to checkpoint.
2. Mark stale evidence STALE; independently re-verify already claimed merges/deploys.
3. Discover and preserve other actors' branches/worktrees; never blanket-reset or prune.
4. For pending mutation determine whether it already happened before retry. Use exact expected head SHA; avoid duplicate PR/release/tag creation.
5. Rebuild task priority based on current reality. Continue highest-value safe work.
6. If same task failed twice for the same root cause, switch to diagnosis and stop blind retry cycles.

## Fail-safe states
- Code committed, PR absent: verify remote and create one PR if appropriate.
- PR merged, deploy absent: do not remerge; observe CI/deploy, potentially rerun documented deployment workflow if authorized.
- Deploy successful, public stale: verify CDN eventual consistency with bounded retries; report stale otherwise.
- CI red after merge: treat master P0, investigate root cause and corrective PR, do not suppress.
- Missing auth or approval: BLOCKED_EXTERNAL, move to independent work.
- Conflicting changes in shared generated file: stop parallel mutation, serialize derive and revalidate.

Never hallucinate continuity; checkpoint is a lead, source state is authority.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
