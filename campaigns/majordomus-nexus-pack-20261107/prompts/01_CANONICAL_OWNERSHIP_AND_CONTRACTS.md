# NEXUS 01: Canonical Ownership And Contracts

**Lane:** foundation  
**Dependencies:** 00  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Unify **ownership and types** for Nexus without introducing a competing orchestrator. Establish the contracts that later prompts implement, and delete drift rather than adding an adapter around drift.

## Execute
1. Walk from existing `capability!` registry, `.ai/manifest.yaml`, task lifecycle, intent graph, and execution model to their consumers. Draw explicit ownership and write/read boundaries.
2. Find every `prompt`, `session`, `intent`, `execution`, `decision`, `evidence` and `peer` identity. Determine which are stable provider IDs, repo IDs, model IDs, task IDs or generated IDs. Avoid inventing a second ID for an entity that already has one.
3. Define a thin typed ingress/coordination projection using canonical existing types. Represent source class (`observed/derived/inferred/unknown`) and provenance without breaking persisted schemas. Use versioning and migration rules already established.
4. Separate *raw captured prompt* from *indexable routing receipt*, *intent resolution*, *workflow execution*, and *completion outcome*. Map references, lifetimes, permissions and authority. The purpose is composition, not five new databases.
5. Audit any current event bus/synth or coordinator; reuse it for lifecycle events and cross-surface updates. If none exists, propose the smallest durable typed event boundary consistent with repo storage, not a general event-stream platform.
6. Amend an existing architecture ADR if project decision rules require; create a new ADR only when genuinely novel. Derive diagrams/docs and keep `AGENTS.md`/`CLAUDE.md` generated from policy.
7. Add schema/unit tests and no-duplicate-identity checks.

## Required proof
- Canonical owner matrix with code links and deleted/retired duplicate owners.
- Schema versioned and round-trip tested against real fixture; unknown values remain unknown.
- Generated projections compile and drift check passes.
- The next stages can import/use one canonical contract without hand-maintained parallel command inventories.

## Failure traps
No `NexusStore` if an existing store already owns the facts. No frontend enum mirroring the backend by hand. No global monolithic service invented as architectural theater.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
