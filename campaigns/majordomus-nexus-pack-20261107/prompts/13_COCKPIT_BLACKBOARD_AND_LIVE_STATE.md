# NEXUS 13: Cockpit Blackboard And Live State

**Lane:** experience  
**Dependencies:** 11, 12  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Make the existing Cockpit/blackboard an operational, attractive, **live projection of Nexus**, not a dashboard of independently curated data.

## Execute
1. Inspect current Cockpit navigation/design tokens/LiveView (if present), API subscriptions/WebSocket, execution feed, peers, task graph and UX tests. Reuse existing interaction patterns and projection payloads.
2. Expose `Intent Workspace`, `Current Execution`, `Decisions/Evidence`, `Mesh Peers`, `Governance Drift` and `Delivery` through existing nav/components as appropriate. Avoid adding a top-level tab for every internal noun if a consolidated view is clearer.
3. Render real state transitions and source links. Distinguish human input from agent suggestion, inferred decision from verified one, active running task from completed/delivered one, stale snapshot from live update.
4. Add meaningful interactions: inspect routing reason, follow source provenance, preview/compare prompt pack, replan within permissions, pause/resume/cancel authorized execution, inspect failure, handover. Every control calls an actual backend capability and exposes returned result; disable controls on unsupported providers.
5. Ensure reconnection, replay from cursor/event sequence, duplicate event suppression and version mismatch behavior; UI state after reload must match API state.
6. Test accessibility/keyboard navigation, reduced motion, narrow screens, empty/degraded states, real streaming events and large work graphs. Preserve design system and page performance budgets.
7. Update generated docs/Pages architecture and publicly safe examples; never publish actual private prompt text or paths.

## Required proof
- A real task's live progress appears without refresh; reconnect catches up accurately.
- At least one control works against real backend and passes negative auth tests.
- Source references and `verified/inferred` status are visible and accurate.
- Cross-surface consistency with CLI/API demonstrated.

## Failure traps
No animated mock dashboard with fake agent telemetry. No duplicate JS enum/catalogue of work states.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
