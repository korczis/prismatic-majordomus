# 03 — Per-PR safe integration protocol

For selected PR:

1. Acquire fenced lease. Fetch current remote PR/base, expected immutable SHAs and integration state; retain rollback/recovery identifiers.
2. Validate PR is open, not already merged, not draft unless repo policy authorizes transition, approved when required, no unresolved blocking review threads, required checks tied to exact head/merge ref green, no unresolved security blockers. Never alter protections to merge.
3. If stale, synchronize/rebase safely in owned worktree; preserve authorship and check whether force push is permitted before rewriting any PR head. Re-run applicable checks and wait for authoritative verdict without unbounded blocking.
4. Repair real conflicts minimally in scoped branch; use canonical intent/evidence and tests to preserve semantics; stop and classify if objective product decision/permission is required.
5. Check stacked branch ancestry and dependency graph; merge ancestor first. Refresh candidate after every prior merge.
6. Execute existing integration executor; don't replace established PR strategy or roll directly into protected trunk. Confirm remote `merged` and target SHA on protected branch. Treat timed-out merge API as UNKNOWN, re-read before retry.
7. Reconcile linked issues/criteria/milestones via canonical completion rules; do not close unverified issues. Register one completion transaction and durable version obligation per distinct eligible completion unit.
8. Record evidence (PR URL, commits, gates, approvals, issue criteria, replay key); hand off to release pipeline; checkpoint.
9. Clean only disposable worktrees proven merged and inactive after ensuring no uncommitted content is lost.

Administrative non-merge PR closure only with objective superseded/duplicate proof and an explicit audit trail. Never close unresolved legitimate work to hit zero.
