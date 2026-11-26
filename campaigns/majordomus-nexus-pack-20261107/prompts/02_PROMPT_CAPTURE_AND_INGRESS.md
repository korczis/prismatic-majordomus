# NEXUS 02: Prompt Capture And Ingress

**Lane:** foundation  
**Dependencies:** 01  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Evolve existing Claude prompt capture into a canonical, idempotent **ingress receipt** without losing the original human prompt or leaking a private prompt archive into knowledge retrieval.

## Execute
1. Inspect `lib/capture.sh`, `majordomus capture`, `share/schemas`, provider adapters, `.ai/local/prompts` storage, capture failure diagnostics, hook scripts and actual tests in current branch. Respect previous archive immutability and JSON+Markdown representation if still canonical.
2. Capture stable event identifiers and source metadata available from the provider; derive repository/Git fields rather than accepting fabricated input. Distinguish human prompts, agent-generated events and provider-injected messages using verified source semantics.
3. Define an ingress idempotency key based on stable provider/session/prompt identifiers where possible; handle missing IDs as uncertain, without falsely conflating repeated human submissions that contain identical text. Persist metadata separately when raw text must remain private.
4. Create a narrowly scoped `receive → validate → persist → index/route` path, reusing the current CLI/runtime boundary. Implement append/atomic write and duplicate replay handling. Never require synchronous LLM inference to record input.
5. Make failed capture visible in doctor/health and able to reconcile later. Preserve the existing rule that an archive error cannot silently discard the user's actual Claude prompt; determine which operations can block only at later tool authorization.
6. Add test vectors: empty prompt, Unicode, multiline, JSON escapes, huge input, same prompt repeated, parallel hook invocations, stale HEAD, unexpected provider fields, missing IDs, filesystem permission errors, inaccessible local store, symlink/path traversal.
7. Add a derived schema/docs projection and a real self-test invoking the actual hook with official JSON input.

## Required proof
- Raw prompt bytes and metadata retain integrity across ingest/replay; no duplicates for identical event IDs.
- Human vs synthetic source is explicit and verified.
- Capture failure leaves actionable evidence and never fabricates a successful receipt.
- No archive text reaches the public API, MCP resource, site, logs or peer model without explicit access control.

## Failure traps
Don't automatically load `.ai/local/prompts` into the Context Compiler. Do not enforce dangerous-tool policy in a prompt archive writer. No unbounded raw transcript ingest.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
