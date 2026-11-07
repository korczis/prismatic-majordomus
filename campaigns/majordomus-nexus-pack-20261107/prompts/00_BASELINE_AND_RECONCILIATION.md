# NEXUS 00: Baseline And Reconciliation

**Lane:** foundation  
**Dependencies:** none  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Establish the **current factual baseline**, protect existing work, and make the implementation plan self-correcting rather than dependent on the September source snapshot.

## Execute
1. Inspect Git remote/default branch, actual HEAD SHA, workspace/toolchain, worktree state, active tasks, issues/PRs/CI and release status. Distinguish local observation from unverified remote state. Stop only on actual unsafe collision; continue independent inspection otherwise.
2. Load `AGENTS.md`, `CLAUDE.md`, `.ai/README.md`, `.ai/manifest.yaml`, policy, applicable doctrines/rules and task lifecycle. Respect `.ai/local` exclusion from context.
3. Build a source-linked ownership inventory for `capture`, session lifecycle, IDD, `proceed`, mesh, peers, immutable prompt archive, execution runtime, ledger/event bus, intent graph, schema, capability registry and UI. Find actual definitions with search and targeted file reads; do not assume names exist.
4. Inspect `.claude/settings.json`, hooks and existing CLI/provider adapters. Compare to current official Claude Code hook schema and installed `claude` version; note cloud/managed settings differences.
5. Detect duplicate implementation candidates and existing pending branches working on Nexus-like functionality. Check collision paths and peer board, announce scope and prospective identifiers if available.
6. Produce a compact *implementation delta* showing already satisfied, partial, absent, conflicting, unsafe and unknowable capabilities. Identify existing canonical owners and precisely which file/test proves each claim.
7. Select the narrowest production-safe first slice: one prompt entering existing capture and receiving a derived, provenance-bearing policy/context receipt. Baseline hook latency, tests and deployment checks (where available).

## Required proof
- A machine-readable or Markdown evidence artifact under the repository's **existing** documentation/report convention. No new registry for a discovery report.
- Current HEAD and effective policy identified, with source paths and test commands.
- Git/worktree/collision safety verified or explicit external blocker.
- Executable plan updated from reality; no code is deleted merely because an old snapshot described it as redundant.

## Failure traps
No synthetic peer announcements, no `git reset --hard`, no guessed commands, no publishing local prompt data, no opening speculative PRs, no relying on stale CI as green.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
