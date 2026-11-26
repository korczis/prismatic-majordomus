# NEXUS 09: Durable Execution And Recovery

**Lane:** intelligence  
**Dependencies:** 02, 07  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Guarantee **durable intent-level continuity** across Claude compaction, process restart, transient mesh outage and retries, while reusing and extending existing execution/state owners.

## Execute
1. Inspect current execution store and durability status; historical `docs/EXECUTIONS.md` described an in-memory capability-call store, but latest master may have changed. Determine level boundaries: short-lived capability invocation vs longer-lived intent/task execution.
2. Reuse a current durable ledger/event store and existing storage strategy if capable; introduce only the minimal persistent journal/outbox/transition model needed. Version stored schemas; track causes, actor, monotonic sequence and optimistic concurrency.
3. Keep receipt and state mutation atomic or implement outbox/inbox semantics with explicit reconciliation. External actions (Git push, PR creation, merge, deploy) must use idempotency/reconciliation, because at-least-once delivery does not imply exactly-once effects.
4. Implement recovery: reconstruct eligible execution state from durable events and canonical Git/CI facts; detect interrupted runs, resume safe steps only, require approval for risky replays. Make cleanup and retention policy explicit; never truncate original evidence silently.
5. Integrate `SessionStart`, `PreCompact`, `PostCompact`, `SessionEnd` with continuation references, not huge transcript injection. Handover must include active intent, current phase, outstanding obligations, last verified SHA and next safe action.
6. Handle concurrent agent updates, clock skew, event reorder, duplicate receipts, lost ACK after external success, stale HEAD, partial file writes, legacy schema migration and hard crash mid-task.
7. Expose recovery diagnostics on the existing doctor/health/execution surfaces with clear `recoverable`, `blocked`, `corrupt`, `unknown` reasons.

## Required proof
- Kill/restart test continues one intent from last durable verified action without duplicating effects.
- Idempotent replay of duplicate hook/tool events.
- `Stop`/`SessionEnd` does not accidentally mark an unfinished task delivered.
- Explicitly documented limits and actual recovery tests.

## Failure traps
No assertion of exactly-once distributed delivery. No second independently writable execution status table. No re-running a deploy because an ACK was missed.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
