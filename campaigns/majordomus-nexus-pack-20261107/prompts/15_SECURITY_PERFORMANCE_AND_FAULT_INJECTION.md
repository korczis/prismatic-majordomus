# NEXUS 15: Security Performance And Fault Injection

**Lane:** hardening  
**Dependencies:** 03, 04, 08, 09, 10  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Harden the whole pipeline against misuse, high latency, prompt injection, partial failure and governance bypass. Make optimistic architecture survive hostile input and realistic process failure.

## Execute
1. Model trust boundaries: human prompt, provider hook JSON, generated context, untrusted repo docs, Git history, model output, peer messages, external provider APIs, arbitrary tool calls, credentials and publication paths.
2. Validate authorize-before-effect semantics at `PreToolUse` and canonical lower-level mutation gates. Prove project hooks can be absent/disabled and CI/repo checks still enforce relevant invariants.
3. Red-team prompt-injection payloads in KB files/PR text, malicious peer suggestions, stale ADR, falsified Git metadata, symlink escapes, unsafe markdown rendering, log PII, secret env, unauthorized provider routing and forged event IDs.
4. Fault injection: hook crash/timeout, shared server down, invalid JSON, corrupted journal, crashed worker during PR/merge/deploy, network partition, duplicate task submit, event reorder, three peers colliding, token budget exhaustion, repeated `Stop` continuation.
5. Measure synchronous hook p50/p95/p99 cold/warm and no-build/no-network path; compare to actual project baseline. Bound memory/cache footprints, I/O and process spawn count. Use performance regression gates consistent with project benchmark policy.
6. Validate schema evolution, replay compatibility and recovery for current and legacy local state; do not corrupt user evidence.
7. Run secret scanning and supply-chain tests, permission boundary checks, least privilege, provider data-disclosure audit.

## Required proof
- Deliberate unauthorized tool attempt is blocked in tests and safe work proceeds.
- Missing hook or model peer cannot silently circumvent canonical CI/mutation gates.
- Worst-case tests terminate within bounded time; recovery doesn't duplicate effects.
- Performance report compares measured baseline to result; regressions addressed or documented as blockers.

## Failure traps
No unconditional fail-closed prompt capture that deletes user input; no blanket fail-open tool authorization; no self-amplifying agent loops.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
