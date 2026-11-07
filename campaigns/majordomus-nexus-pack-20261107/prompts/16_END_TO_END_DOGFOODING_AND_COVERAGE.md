# NEXUS 16: End To End Dogfooding And Coverage

**Lane:** hardening  
**Dependencies:** 13, 14, 15  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Prove Nexus works **as a user would actually use it**, from prompt entry through context, intent continuation, execution, validation, integration and UI observation.

## Execute
1. Construct a deterministic demo/test fixture in the current repository convention with a small safe task and governing `.ai` rule/ADR.
2. Start or simulate a genuinely supported Claude Code session/hook. Submit initial prompt, attach follow-up refining requirements, and another duplicate; inspect ingress receipts, resolution decisions and persistent intent IDs.
3. Assert relevant scoped `.ai`, Git/history and KB evidence appears with correct provenance; an intentionally contradictory source yields explanation, not silent reinterpretation.
4. Execute a small real code or fixture change through existing Proceed/IDD, exercising one mesh consultation if peers are legitimately available; if not, test unavailability as documented. Check worktree claims and tests.
5. Kill/restart a worker (test environment) and resume without duplicate side effect; simulate compaction/reconnect and inspect continuity/handover.
6. Inspect outcome through CLI structured output, API/OpenAPI, MCP and Cockpit streaming; assert canonical IDs/status/evidence and public redaction match.
7. Run user flow through intent composer: draft, edit, derive references, preview prompt pack, materialize, execute; verify from a fresh UI client/reconnect.
8. Prove release readiness: linked issue/PR/commit status, local checks and relevant CI where available; do not assert merge/deploy before the release step. Include a post-merge live-probe case to be executed in stage 17.
9. Report negative controls: simple QUERY avoids work graph spam, unauthorized mutation denied, no peers available, stale context invalidated, hook not loaded indicates coverage gap.

## Required proof
- Executable use case with commands, assertion results and linked evidence.
- Published claim precisely matches what passed, no manufactured success screenshots.
- Defect list is either fixed in the same lane or isolated as a real external blocker.

## Failure traps
No mocked frontend that is disconnected from same canonical test execution. No claim of 'every prompt' coverage without showing supported host modes.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
