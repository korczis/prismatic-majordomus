# NEXUS 10: Ai Governance Reconciliation

**Lane:** intelligence  
**Dependencies:** 05, 09  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Make `.ai/**` a **living verified contract** that benefits from actual outcomes, without turning every chat speculation into a committed rule.

## Execute
1. Inspect existing policy/rule/doctrine/ADR/KB schemas, `.ai/manifest.yaml`, claim/drift validators, generated `CLAUDE.md`/`AGENTS.md` and versioning. Confirm who is authorized to change canon.
2. Add a reconciliation step after relevant implementation/decision outcomes: detect whether a new invariant, design tradeoff, discovered knowledge, claim drift, stale rule or broken linkage requires a proposal or update.
3. Classify proposals by provenance and epistemic status. `observed` and `inferred` are not `verified`. Record evidence (test/commit/CI/review) and explicit alternative; stage `proposed → validated → approved/mutable` according to existing policy.
4. Materialize only changes justified by current governance, using canonical schemas, explicit manifest registry, valid references and generation from source. Avoid rewrites of unrelated `.ai` entries and don't silently edit vendor-pinned rules.
5. Enforce drift checks between code, documentation and governance. A mismatch should be diagnosable at `check/doctor/CI` and in UI; automatic remediation can handle derived projections but must not invent architectural approval.
6. Add integration tests: real new capability requiring policy materialization, simple typo not requiring ADR, rejected inference, contradictory rule and implementation, new rule added that propagates to relevant surfaces, dirty generated output.
7. Document reasoning and extension flow in repo's current conventions.

## Required proof
- One code change produces a correctly validated `.ai` update when required, plus regenerated docs/providers and traceability.
- One Q&A prompt produces no unnecessary governance churn.
- CI detects a deliberate governance/implementation drift fixture.

## Failure traps
Do not treat model consensus as approval. No auto-commit of unreviewed dangerous governance changes. No unmanaged parallel `.ai/nexus/` root folder without manifest and schema.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
