# NEXUS 12: Cli Api Mcp Schema And Projections

**Lane:** experience  
**Dependencies:** 01, 09, 10  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Expose Nexus through **canonical discoverable capability declarations**, generating CLI/JSON/HTTP/OpenAPI/MCP/docs outputs without manually synchronized registries.

## Execute
1. Inspect existing Rust `capability!` registration and typed output conventions, command bridge `just`/CLI discovery, OpenAPI/MCP generator, schemas, diagnostics and existing execution endpoints.
2. Determine which existing commands/capabilities naturally own Nexus ingress, resolve/explain, intent draft, execution state, recovery, evidence and diagnostics. Extend them before creating a new command group.
3. Add minimal typed canonical operations for uncovered use cases (concepts: inspect prompt correlation, explain intent decision, preview, materialize, resume, cancel, status). Only publish an operation if implemented and a user scenario genuinely consumes it.
4. Derive OpenAPI schema and MCP tooling from the canonical operation descriptor and JSON schemas. Ensure stable IDs, deterministic canonical order, error model, auth/scope and pagination where applicable. No secret/raw prompt exposure from public tools.
5. Integrate generated command bridges/completions, docs and use-case discovery via existing repo generators. Preserve backward compatibility for existing public API semantics, version breaking schema changes properly.
6. Cross-surface contract suite: the same fixture yields consistent IDs, states, provenance summary and canonical sorting across Rust model, CLI JSON, HTTP, MCP and docs generation.
7. Add negative tests for missing IDs, bad route, invalid scope, unauthorized control, partial provider support, stale policy and recovery failures.

## Required proof
- New canonical operation appears in every applicable surface without separately editing inventories.
- OpenAPI docs, MCP descriptors and CLI help derive from one definition.
- Golden tests and generated drift checks pass.

## Failure traps
No manual Swagger edits, custom HTTP route out of registry or frontend-owned status translation.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
