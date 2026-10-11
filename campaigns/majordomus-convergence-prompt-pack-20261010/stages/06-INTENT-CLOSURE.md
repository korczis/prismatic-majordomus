# 06 — Prove intent-driven development end-to-end

## Objective
Turn an authored desired outcome into a critique-reviewed, evidence-backed, satisfied intent without manually synchronizing six status systems.

## Audit
Read ADR 0070, 0073, 0075, 0098, 0107, 0111 and relevant current code/intents/evidence schemas; inspect PR #807, #813, #819, #292 and dependent changes. Distinguish in-master code from open branch plans.

## Tasks
1. One typed intent defines statement, invariants, non-goals, acceptance criteria, and citations. Status/stage derived from plan and ledger, never a second mutable lifecycle.
2. Start an actual session that discovers active intent and repository context; bind issue/task → criterion → milestone → intent with explicit rejection of orphan work unless policy permits marked exception.
3. Before implementation, execute an opposition/review pass through available provider capability or an equivalent local deterministic fallback that clearly distinguishes 'review unavailable' from 'passed'. Bind review to exact plan digest; modifications invalidate it.
4. Derive a minimally sufficient plan, record invariants and proof obligations; ensure generated tasks/issues don't explode merely to mirror prompt headings. Confirm authorization boundaries for autonomous work.
5. Execute one representative nontrivial slice and record each command/test/deployment evidence against commit + input digest, freshness, tool identity, result and provenance. No claimed pass from declaration or inputs-unchanged alone.
6. Completion discharges obligations but must NOT independently declare intent satisfied. Satisfaction computed from ledger and criteria, including optional vs required, invalidated and superseded conditions.
7. Surface causal explanation in CLI/API/OpenAPI/MCP/Cockpit: why currently planned, executing, verifying, satisfied, blocked, unknown; link each criterion to original proof. No hand-coded frontend verdicts.
8. GitHub milestone and issue projection reconciles without creating shadow identifiers or status loops; avoid unnecessary remote writes and handle eventual consistency.

## Proof
Demonstrate a full small end-to-end real scenario and negative cases: unsupported claim, stale digest, optional criterion, missing review, plan change, failed required test, partial deployment, missing issue link, duplicate evidence and environment failure. Projected satisfaction must match engine exactly. Report successful integrated SHA, not an idealized mock.

## Operating contract (mandatory in every phase)
- Work against the repository checked out locally. Treat 2026-10-10 audit as leads, NOT a live truth. Fetch and inspect origin, HEAD, branches, PR checks, CI, tags, manifests, ADRs, .ai/**, docs and actual implementation before changes.
- Repo identity: `korczis/prismatic-majordomus`; detect the actual default branch, do not blindly assume master if it changed.
- Follow AGENTS.md and .ai/ governance; reuse existing capabilities, schemas, rules, registries, evidence ledger and commands. No independent truth in MCP, CLI, HTTP, OpenAPI, Cockpit, website, generated documents or shell wrappers.
- Read applicable existing ADRs and test architecture; amend rather than duplicate. Reject parallel control planes, synthetic statuses, mock production data, optimistic health, stale proof and manual inventories.
- Every issue/step has: invariant, owner/source of truth, scope, tests, diagnostic, acceptance proof, and merge/release impact. Attach work to existing intent/milestone/issue if possible; create only if needed, respecting ID allocation and sync rules.
- Make a reversible baseline: `git status`, `git rev-parse HEAD`, `git worktree list --porcelain`, branches, open PRs, CI runs, version, release, existing runtime state. Never reset, delete worktrees or force push without verifying ownership, merged state and no valuable uncommitted work.
- Implementation sequence: inspect → reproduce → design minimum compatible change → write failing test → implement → targeted checks → cross-surface contract checks → docs/evidence → integration. Never report green when unknown.
- Changes must be reviewed against actual `master`/default branch, use ephemeral branches/worktrees, small semantic deltas, deterministic generated output. De-duplicate generated changes where practical; no bulk unrelated formatting.
- Safety: do not change remote secrets, production deployments, destructive Git refs or branch protection silently. For explicitly authorized autonomous release/deploy actions apply repo policy, verifiable gates, least privilege, atomic rollback and fail closed; stop on missing credentials/permissions rather than bypass safeguards.
- If external advisors/providers are unavailable, continue locally without degrading evidence requirements. Do not promise that a plan/review/test passed unless it actually ran and can be linked to commit/digest.
- No time-box theater: optimize for verified closure, not task count, LOC, number of prompts, or a fabricated speed claim.
- Before final report: compile; focused and impacted tests; model/schema/derive drift checks; smoke clean checkout; CI status and commit SHA; security/resource check if applicable; update documentation; enumerate unresolved blockers precisely.

## Mandatory report per stage
1. Baseline HEAD SHA, release/tag, relevant PR/issues/CI links (actual values, not invented).
2. Root cause with file:function or line references and a reproducible failure; state explicit unknowns.
3. Changes made, boundaries preserved, migrations and deletions of legacy code.
4. Evidence matrix: claim → automated test/gate/run → revision/digest → outcome.
5. Numerical before/after if measured; otherwise `not measured`.
6. Artifacts/commit(s)/PR(s), remaining blockers, next runnable stage.
