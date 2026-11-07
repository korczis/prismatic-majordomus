# NEXUS 08: Mesh Council And Collision Avoidance

**Lane:** intelligence  
**Dependencies:** 07  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Make existing AI/LLM mesh useful for **evidence-based autonomous consultation and conflict-free delegation**, not theatrical multiple-model voting.

## Execute
1. Discover actual shared MCP peer lifecycle, `majordomus_peers`, `majordomus_announce`, blackboard/event bus, lease semantics, provider capability/availability, task claims and Git worktree collision guards.
2. Integrate peer discovery into Nexus decisions. Before proposing new work, fetch current announced scopes, inspect Git branches with collision checker, claim paths and IDs through the existing mechanism. Treat announcements as claims, not locks.
3. Define a bounded council protocol: proposal (builder), adversarial critique (independent reviewer), verification (tester/architect), evidence-based reconciliation. Assign jobs based on capability, availability, relevant scope and budget, not hardcoded model preference.
4. Implement scoped, privacy-aware consultation payloads; no raw `.ai/local/prompts` or secrets exported by default. Include exact source refs and falsifiable claims. Respond to missing peers with `unavailable`, not fake quorum.
5. Make delegation collision-safe: per-resource worktrees/leases, bounded fan-out, timeouts, cancellation, late result handling, shared correlation and deduplication. Reconcile peer output through the canonical decision/evidence owner.
6. Ensure adversarial review adds measurable value for risky architecture changes while simple queries avoid 3-model overhead. If an available peer contradicts tests/ADR, evidence wins and dissent is preserved.
7. Test: two peers claim same files, stale peer reconnect, lost server, duplicate messages, contradictory recommendations, provider timeout, unknown model permission, adversarial prompt injection and a read-only query that never launches a council.

## Required proof
- Real or appropriately isolated integration test showing scope claims, collision detection and resolved work assignment.
- Decision retains peer findings and links to code/tests; no invented consensus.
- Privacy and budget boundaries hold under failure.

## Failure traps
No new service named `council` if current mesh already has equivalent orchestration. No automatic external model activation without authorization.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
