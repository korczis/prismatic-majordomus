# 01 — Baseline, WIP and dependency graph

## Objective
Establish a trustworthy, timestamped operational picture and reduce queue entropy. Do not implement new business features in this stage.

## Tasks
1. Check exact HEAD/default branch SHA, release tags, all worktrees and local dirty changes. Collect GitHub PR metadata for **all pages**: head/base SHA, creation age, draft, conflicts, required checks by app, CI outcomes, changed paths, labels, issue/intent dependencies. Never assume first 100 results is all.
2. Build a dependency DAG using actual code overlaps and declared links. Classify each PR: correctness P0, CI/integration P0, product-closure P1, other P2, obsolete/superseded. Explicitly identify ambiguous/conflicting cases needing review.
3. Compute per PR semantic source delta versus derived/generated drift; report counts and top collision files. Classify worktree as merged, active, abandoned-but-valued, stale-rebasable, or unknown. `unknown` is never safe to delete.
4. Inspect queue/merge orchestration (ADR 0101) and evidence validity against **current** SHA. Check whether old checks, dispatches, app mismatch, pending check or master drift can ever be misread as pass.
5. Define WIP/backpressure admission invariant in canonical existing governance model. Prefer enforceable gate/command over written-only doctrine. Do not reject active user work without explicit reason.
6. Produce baseline JSON using existing schemas, if possible; expose metrics through existing command/API/Cockpit projections rather than manually duplicated status file.

## Proof
- Reproduce queue classification from same input in random order with identical output.
- Fixture including two competing branches, renamed worktree, untracked changes, dirty index, merged branch, stale SHA and unknown check produces safe, deterministic decision.
- No destructive action this stage. Enumerate top 10 integration candidates and their blockers with links and SHAs.
- Output current WIP, median/P95 PR age, merge throughput over 24h/7d, and actual unknown counts (or `not available`).

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
