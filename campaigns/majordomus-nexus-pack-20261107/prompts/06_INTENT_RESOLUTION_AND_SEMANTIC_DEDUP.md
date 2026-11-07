# NEXUS 06: Intent Resolution And Semantic Dedup

**Lane:** intelligence  
**Dependencies:** 02, 05  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Convert prompts into the **right kind of work** without issue spam, repeated investigations or accidental authorization.

## Execute
1. Reuse the actual intent/issue/milestone graph, IDD semantics, task planning and Git/branch linkage. Inspect which state transitions are legal and which statuses must be derived.
2. Implement deterministic-first route classification `QUERY / ACTION / INTENT / CAMPAIGN / CONTROL` with typed evidence and explainable fallback. Use model reasoning only for genuine ambiguity and within budgets.
3. Match against active/prior intents, duplicates, issues, milestones, PRs and touched subsystems by canonical IDs, paths, Git evidence, semantic similarity and explicit user reference. Distinguish `same objective` from `similar wording`. Use confidence thresholds with reversible routing default.
4. Support intent actions `attach`, `refine`, `split`, `supersede`, `merge`, `reopen`, `create`, and controls `pause/resume/cancel/replan` only as authorized by existing work graph. Version and audit all meaningful changes.
5. Preserve user intent and original prompt independently of the recommended plan. If different product directions would materially affect persistent design, consult peers and escalate only if no safe reversible choice exists.
6. Avoid creating issues for read-only questions and small standalone actions. When splitting work, use actual dependencies and completion evidence; derive issue/milestone status rather than hand-authoring it.
7. Simulate paraphrases, conflicting instructions, follow-up correction, duplicate identical event, different users issuing same text, resumed sessions, concurrent requests and prompt injection inside retrieved files.

## Required proof
- At least one 3-prompt dialogue attaches/refines a single intent with no redundant issues.
- QUERY path produces no unnecessary mutation or LLM fan-out.
- Result includes exact evidence and explanation for reuse/split/create.
- Work item graph remains conformant to existing validators.

## Failure traps
Do not use an embedding similarity score as an authority to merge two potentially different intents. Do not create a second issues table.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
