# NEXUS 14: Ai Telemetry Economics And Adaptation

**Lane:** experience  
**Dependencies:** 02, 09  
**Delivery:** concrete code/tests/evidence; integrate in lane, not one PR per prompt.

## Objective
Connect Nexus routing and delivery outcomes to **provider-neutral AI telemetry and economics**, then measure whether context/routing/mesh improve real development results.

## Execute
1. Inspect existing usage/cost telemetry, Claude Code OTel/statusline hooks and any provider-specific adapters, budget rules, release/reporting and measured baselines. Respect official telemetry setup scope and privacy restrictions.
2. Create/reuse typed usage events with provider/model/session/prompt/intent/execution correlation, token categories/cache, latency/tool payload size, estimated cost and source accuracy. Keep billing reconciliation distinct from estimate when authoritative billing is available and authorized.
3. Ingest through official OTel and statusline hooks where supported; local collection not enabled means `unknown`, not zero. Never enable raw API request/response bodies merely for optimization.
4. Derive metrics: time to first useful action, prompt→delivered intent rate, intent reuse, duplicate work avoided with defensible counterfactual, context relevance, retrieval overhead, resume success, tool overfetch, compaction, peer benefit, cost per verified outcome.
5. Integrate actionable optimization findings into existing report/evolve/continuous improvement loop with provenance and safety/quality guardrails. Routing policy changes must be measured against baseline, not automatically accepted based on one anecdote.
6. Expose selected safe and honest projections in CLI/API/Cockpit/docs, using existing capability/schema contracts. Do not create a standalone economy dashboard if current UI already supports telemetry.
7. Tests: absent credentials, unsupported provider, partial session, billing unavailable, duplicate events, incorrect cost attribution, secret-redacted logs, a false-positive optimization finding and a verified improvement experiment.

## Required proof
- End-to-end correlated usage record for one real or testable local session where supported.
- Clear distinction estimated vs reconciled cost, measured vs inferred saving, recorded vs missing metrics.
- At least one useful grounded optimization finding with an explicit falsification test.

## Failure traps
No invented token savings percentages, no secret export, no auth-token scraping, no new telemetry truth alongside existing provider-neutral schema.

## Execution protocol for this prompt

Read the root `MASTER_CLAUDE_CODE_PROMPT.md` and relevant `contracts/*.md` first. Verify all referenced paths/commands in the **current** repo; the 2026-09-11 findings are historical clues, not authority. Before new paths/IDs, use the active worktree, scope/overlap and peer-claim machinery. If a relevant behavior is already implemented, execute proof and improve gaps instead of duplicating it.

Do the work now. Keep changes bounded and production-safe; split the implementation internally if needed, but keep one coherent ownership boundary. Use the current codebase's actual APIs, test patterns, schemas and release workflow. Checkpoint durable progress, update affected real tests/docs/.ai, run focused and canonical gates, and push/integrate within the lane when policy permits. Use peer consultation only when meaningful uncertainty survives source/test inspection and budget allows.

End with: (a) changed canonical owners, (b) exact commands/results/evidence, (c) deleted duplication, (d) current SHA and lane/PR/merge status, (e) next dependent prompt or actual external blocker. Never call a partial or unmerged change 'deployed'.
