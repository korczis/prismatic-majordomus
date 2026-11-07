# NEXUS 07: Proceed Idd Execution Unification

**Lane:** intelligence  
**Dependencies:** 01, 06  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Route authorized work through **existing Proceed, IDD and capability execution semantics**, eliminating overlapping orchestration entry points.

## Execute
1. Locate current `proceed`, IDD planning, skills (`implement`, `evolve`, `rebase/integrate` where present), active task lifecycle, execution definitions and mesh scheduling. Identify which layer owns intent planning vs scoped task execution vs monitored capability call.
2. Trace a real prompt to existing work: task start and scope, planning/dependency resolution, capability selection, execution, checkpoint, `check`, evidence and `finish`. Use adapters around existing commands rather than new scripts duplicating behavior.
3. Define a thin Nexus coordination API: `resolve → plan → authorize → dispatch → observe → verify → close`. Each step maps to an existing owner with typed correlation IDs. Avoid dual scheduling queues and duplicate status enums.
4. Ensure `ACTION` uses minimal workflow, `INTENT` uses structured planning where necessary, `CAMPAIGN` creates a bounded work graph, and `CONTROL` targets existing executions. Honor scope and worktree topology.
5. Promote generic capability descriptors/skills from canonical registry with deterministic selection. Defer high-cost peer/LLM steps unless job complexity/risk calls for them.
6. Eliminate obsolete parallel launchers/translators if consumers can be migrated safely; preserve stable CLI/MCP APIs through compatible adapters only where justified.
7. Tests: a small code change, a nontrivial architectural change, blocked scope change, zero-change request, cancellation and resume; assert no double execution and same canonical lifecycle across CLI and MCP.

## Required proof
- Real prompt triggers existing coordinator and task lifecycle, with trace to owning records.
- Exactly one system owns each state field; public surfaces agree.
- `finish completed` cannot succeed without actual required evidence.

## Failure traps
No newly invented `proceed2` skill/daemon, no bypass of canonical quality gates, no event called `completed` after a mere agent response.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
