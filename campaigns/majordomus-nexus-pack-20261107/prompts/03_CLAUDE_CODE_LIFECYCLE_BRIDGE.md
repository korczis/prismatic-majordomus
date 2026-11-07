# NEXUS 03: Claude Code Lifecycle Bridge

**Lane:** foundation  
**Dependencies:** 02  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Wire supported Claude Code lifecycle **through the existing capture integration** and Nexus receipt/resolution, using real hook schemas and minimal synchronous cost.

## Execute
1. Read current vendor `https://code.claude.com/docs/en/hooks`, installed Claude version, repo settings precedence, permission policies and current `.claude/hooks` scripts. Record supported event schema and decision semantics; do not copy guessed 2025 snippets.
2. Add or extend only useful events: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `SubagentStart/Stop`, `PreCompact`, `PostCompact`, `Stop`, `SessionEnd`. Prefer one generated event adapter dispatching typed events; keep existing shims functional and backward compatible.
3. `UserPromptSubmit`: capture original prompt, return bounded `additionalContext` and correlation reference, never claim replacement capability. Classify source to avoid recursively authorizing provider-generated prompts.
4. `PreToolUse`: consult cached scope/policy capability; deny only actual unauthorized effects with correct JSON, preserve human permissions. `PostToolUse`/failure record actual observed result and attribution, never pretend to rollback completed effects.
5. `SessionStart` builds minimal briefing from current effective `.ai` and active task context; no full-tree scan/network/model call on prompt hot path. `PreCompact` checkpoints durable references. `Stop`/`SessionEnd` update evidence and completion state but **do not** infer task completed just because model stopped. Respect `stop_hook_active`, limit continuation.
6. Test hook timeout/error behavior, malformed JSON, stderr/stdout semantics, `onFailure` behavior, project trust, user/local/managed precedence, cloud support differences, `cwd` switches, multi-worktree sessions, subagent identity, reentrant hook invocation and multiple simultaneous Claude sessions.
7. Optionally implement a mod-based interceptor only if current installed Claude supports the feature and real E2E tests prove incremental value; never let it bypass canonical ingress/governance.

## Required proof
- Interactive/fixture Claude prompt receives relevant bounded context with source refs.
- Capture remains lossless and doesn't block a user due to telemetry outage.
- Explicitly unauthorized tool call is prevented; authorized work isn't derailed.
- Actual hook configuration validates via installed Claude Code tooling; tests cover hosted modes where supported.

## Failure traps
Don't patch npm internals, MITM API calls, or claim universal interception. Treat hook support in cloud/other clients as a coverage matrix, not an assumption.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
