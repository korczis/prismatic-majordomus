# NEXUS 05: Evidence Aware Context Compiler

**Lane:** intelligence  
**Dependencies:** 01, 02  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Produce **minimum sufficient, provenance-bearing, low-cost context** per task, derived from `.ai`, scoped source, Git, KB, ADR and relevant work history without dumping whole directories.

## Execute
1. Discover existing `majordomus context resolve/explain/validate`, `.ai/manifest.yaml` sections, `tracks` contract, KB/ADR indexes, source/graph retrieval, session context and cache invalidation. Measure the current briefing/context budget and actual latency.
2. Implement or extend a typed context projection keyed by prompt route, repo HEAD, path scope, policy version, relevant intent and source fingerprints. Preserve deterministic precedence and explicit conflict diagnostics.
3. Retrieve effective scoped rules/doctrines, relevant ADR and KB decisions with provenance, code/tests and Git changes, active task/issue/milestone and prior validated outcomes. Distinguish direct observation, inference and unknown. Never make raw captured prompts an implicit KB.
4. Add relevance ranking and deduplication. Include context selection/exclusion reasons, token/byte budget and cache key so an agent can ask `why this document?` and `what was left out?` via existing explain capability.
5. Use contextual retrieval/semantic graph where actually available; derive cheap baseline from deterministic policy and index. LLM summarization may enrich asynchronously only within budget and permitted trust boundary.
6. Verify cache invalidation on `.ai`/ADR/knowledge/policy/HEAD changes and cross-session compaction; safe fallback when index/service unavailable. No expensive cold build from a prompt hook.
7. Create tests with contradictory ADR vs current implementation, stale session claim, newly added rule, changed tracked path, intentionally irrelevant KB entry, huge source tree and a secret-containing file that scope excludes.

## Required proof
- A real prompt receives targeted source-linked context, not a one-size-fits-all giant preamble.
- Same input/fingerprint yields deterministic output across interfaces.
- A changed governance fact invalidates context and changes relevant decision evidence.
- Cold/warm latency and size are measured, no invented improvement percentage.

## Failure traps
No invalid `.ai` section added without manifest registration. No prompt archive retrieval. No embedding index as a separate authority.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
