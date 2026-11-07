# NEXUS 11: Persistent Intent Composer And Prompt Preview

**Lane:** experience  
**Dependencies:** 06, 09, 10  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Deliver the workflow **new intent → write/discuss on the fly → derive related `.ai`/Git/ADR/KB → questions only for material ambiguity → live plan/prompt pack preview → materialize → execute**, through the existing Majordomus UI/CLI rather than an isolated prototype.

## Execute
1. Audit Cockpit/LiveView/editor components, current intent/IDD views, persisted drafts, realtime bus and API. Reuse existing UI design system and stream mechanism; no new standalone frontend state authority.
2. Implement a versioned, persistent intent draft with typed revision events and conflict handling. Debounce text changes; avoid LLM calls on every keystroke. Derive context recommendations and links with provenance and confidence.
3. Show a useful editor composition: objective, constraints, existing work matches, ADR/KB/rule references, contradictions, material open questions, plan DAG, prompt and prompt-pack preview, impact, risk/approvals and actual execution state.
4. Make the preview **read-only and reproducible**: changes in policy or repository HEAD flag stale previews; materialization creates/updates the canonical intent/issue graph and execution plan transactionally with idempotency.
5. Permit interactive discussion with Nexus and available peers; suggestions are evidence-linked and never mutate canonical state before an explicit materialization/authorization action. Support undo/revision history and resume after reconnection.
6. Derive appropriate CLI/REST/MCP operations from the same declared capability, not separately registered endpoints. Expose all state via API for cross-surface consumers.
7. E2E: create draft, reconnect, append new constraint, see changed linked ADR, preview diff, select the materialization route, execute a minimal task and follow delivery; test competing edits and inaccessible peer/LLM.

## Required proof
- Real persistent draft, working UI controls and backend, no mock data.
- Preview/materialization correlate to one canonical intent and no duplicated issues.
- UI after refresh/resume shows current actual plan and evidence.
- Performance and mobile/responsive sanity checks according to existing design policy.

## Failure traps
No CSS-only storyboard or static diagram pretending to be product. No non-authoritative frontend duplicate business logic.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
