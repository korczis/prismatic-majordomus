# 03 — Worker scheduling, claims and provider adapters

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Use existing mesh scheduler/claims/leases to find authorized idle worker matching required capabilities and repo trust. Local worker first, optional remote; adapters Claude Code/Codex/etc only if installed and configured. An Implement click must invoke actual task executor in isolated worktree, attach task intent/issue, reuse context compiler .ai policies, enforce scope and completion. No fake background worker, no detached unowned process. Include cancellation, budgets, timeout, refusal on conflict, absent worker pending state and evidence for tool outcomes. Verify real file change + test from fixture project; no requirement to grant remote arbitrary command execution.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
