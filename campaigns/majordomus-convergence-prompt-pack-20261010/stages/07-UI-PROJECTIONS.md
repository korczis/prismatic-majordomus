# 07 — Truthful, usable, responsive UI over canonical capabilities

## Objective
Make `majordomus ui`/Cockpit genuinely useful for real users including mobile, without adding any separately maintained domain state. Confirm what commands actually exist first.

## Audit
Inspect ADR 0012, 0088, 0089, current design rules, Cockpit routes/layout, `health.rs`, UI-integrity checks, existing API/OpenAPI and MCP capability graph, derived site model. Identify current frontend stack rather than adding Flowbite/Alpine/Cytoscape indiscriminately.

## Tasks
1. Inventory all visible factual fields and actions: capability source, typed response, evidence, observed_at, status, relevant route, empty/error/loading behavior and permissions. Eliminate mocked production content, fake counters, hardcoded inventory and optimistic default. `unknown` differs from zero, success and absence.
2. Use shared observed-state envelope or existing equivalent where appropriate; domain engine owns verdict, frontend owns only presentation/sort/filter/interactions. Render freshness and provenance without clutter.
3. Information architecture around high-level tasks rather than low-level module inventory: repo overview, intents/outcomes, active work, peers/mesh, health & provenance, CI/integration, release/deploy, knowledge and diagnostics. Derive navigation and actions from capability metadata with stable IDs.
4. Mobile first: 320/360/390 px viewports, drawers, focus trap, Escape/outside close, body scroll lock, screen-reader semantics, keyboard navigation, touch targets, overflow, no unreadable multi-column tables, reduced motion, empty states.
5. Mutations invoke existing typed capabilities with authorisation, optimistic UX only when safely reversible, explicit pending/failed/unknown outcomes and audit trail. A peer action must never exist solely as a JavaScript fake button.
6. UI tests include network errors, unauthorized, timeout, stale data, capability removed, high cardinality lists, slow API, empty repo, partially installed runtime. Take mobile screenshots/visual regressions against real API fixture, not production demo mock data.
7. One zero-registration test: newly declared canonical capability/intent/service automatically appears as appropriate in CLI JSON/API schema/MCP/UI/docs, no per-surface manual list edits.

## Proof
Real browser interaction test, keyboard/a11y checks, responsive screenshots, fault injection, cross-surface IDs/verdicts and strict UI-integrity gate. No fake green health allowed.

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
