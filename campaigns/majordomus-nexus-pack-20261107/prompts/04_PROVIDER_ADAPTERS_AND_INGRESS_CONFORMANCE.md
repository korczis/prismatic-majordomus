# NEXUS 04: Provider Adapters And Ingress Conformance

**Lane:** foundation  
**Dependencies:** 02, 03  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Make Nexus provider-neutral by **contract**, not by promising unsupported hooks on every CLI or cloud runtime.

## Execute
1. Inventory current `.mcp.json`, `.codex/config.toml`, `.gemini/settings.json`, provider registry and any native hooks/observability channels. Verify each installed client/version and supported event types, authentication restrictions and privacy profile.
2. Define a typed `IngressCapability` matrix with exact supported semantics: `prompt_received`, `session_started`, `tool_preflight`, `tool_result`, `compaction`, `agent_lifecycle`, `completion_signal`, and degraded/unsupported. Derive UI/doctor/docs from registry.
3. Implement the minimal official supported adapters: Claude first and, where installed/authorized and technically supported, Codex/Gemini or generic MCP/CLI external entry. Unknown provider event coverage must remain `unsupported`, never silently `observed`.
4. Preserve original prompt and source type, canonical correlation rules and idempotency semantics even for partial providers. Do not scrape provider private transcript storage or monkeypatch packages when official lifecycle interfaces are available.
5. Run conformance fixtures for a read-only prompt, mutation prompt, synthetic agent message and a resumed session across every claimed supported provider. Generate coverage report and explain missing seams.
6. Add extension guide: new provider adds one adapter + capability descriptor and tests, not modifications to every surface.

## Required proof
- Exact capability matrix by provider/version tested against available runtime.
- New adapter is discoverable and rendered canonically without duplicating business rules.
- Unsupported providers fail transparently or use explicitly downgraded supported path.

## Failure traps
Never claim Codex/Gemini hook support without testing. Do not send private code to another provider merely to satisfy a provider count.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
