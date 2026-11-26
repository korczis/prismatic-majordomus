# NEXUS 18: Cleanup Evolution And Operability

**Lane:** hardening  
**Dependencies:** 17  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Finish the mission by eliminating redundant machinery, making Nexus operable by future agents, and turning observed failures into bounded self-improvement without reopening the same architectural discussion next week.

## Execute
1. Search for obsolete hooks/duplicate registries/sort lists, alternate execution states, old proposed `nexus` scaffolds, stale docs and flags. Delete truly obsolete items with migration and rollback strategy; preserve public compatibility when required.
2. Verify `.ai` policy/rule/ADR/KB decisions and generated provider instructions, docs, site, README hierarchy, examples, changelog and use-case scenarios are consistent with actual shipped code. Publish a precise extension guide for a new provider, route class or surface without consumer-specific registration.
3. Add doctor/explain diagnostics and runbook for: hook missing, capture failure, intent resolution ambiguity, unavailable peer, execution stuck, recovery conflict, schema migration, release gate blocked and public projection drift.
4. Integrate Nexus with existing `/evolve`, self-report and incident/failure-analysis workflow if present: capture repeatable optimization hypothesis, evidence, proposed change, test, rollback and measured post-change result. Never self-modify critical governance without review.
5. Ensure the implementation carries performance, cost, security and correctness metrics with honest availability flags; document measured baseline and current run, not guessed percentage improvement.
6. Provide one copy-paste user journey and one automation-friendly journey: normal Claude Code prompt implicitly engages Nexus and existing `.ai`; user can inspect reason/outcome/active work without restating intent.
7. Final release reconciliation: current master/trunk SHA and deployed revision verified after merges, no unmerged production code or missed docs; report any genuinely blocked external gate.

## Required proof
- Redundancy scan and real deletion/migration evidence.
- Fresh clone/install or equivalent E2E proves hooks, rules, discovery and generated surfaces function with no manual per-consumer setup.
- Operational runbook plus sample explain/report derived from real test data.
- Final report separates `implemented`, `verified`, `merged`, `deployed`, `unsupported` and `blocked` accurately.

## Failure traps
No 'done forever' promise without enforceable regression tests. No autonomous governance loop with no rollback/budget.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
