# 03 — Shorten CI time without weakening its verdict

## Objective
Target quick feedback and substantially shorter safe merge validation. All improvements must be measured and explainable, not merely asserted.

## Data and profiling
1. Fetch >= 20 recent successful/failed/cancelled `validate` runs and their jobs/steps when available, by event and SHA. Separate queue wait, runner startup, dependency download, cache restore, compilation, shell suite, browser, integration tests, artifact upload and final verdict dependency wait.
2. Compute p50/p95 and critical path per class of change (Rust, docs, site, mesh, release, generated). Record cancellation waste and flaky retry rates; benchmark isolated runner variance. Historical 59–98 minutes is only a starting observation.
3. Audit `.ai/repo/ci/{gates.yaml,suite.yaml,suite-durations.tsv}`, `scripts/ci-plan`, `scripts/ci/run-plan`, `.github/workflows/validate.yml`, workflow action versions, rust build cache, `CARGO_TARGET_DIR`, sccache if present, test sharding and correctness of union-of-impacts.

## Improve systematically
- Establish a <60 s **fast signal** with meaningful mandatory structural/syntax/test checks, and a separately proven complete relevant merge gate. Do not label the fast signal `full pass`.
- Plan from dependency/impact model not only path names if gaps exist; unknown paths escalate, never skip.
- Persist cache keys on toolchain, lockfile, feature flags, target, OS and build profile; avoid caching invalid artefacts or accidentally leaking credentials.
- Rebalance shards from measured durations, account for variance and outliers, verify every numbered test case runs once, including case 51; preserve reproducibility.
- Reuse compiled outputs between steps/jobs where viable, avoid repeated full rebuilds, unnecessary GitHub API calls and heavyweight site recomputation.
- Detect long tail job, bottleneck in verdict needs graph and slow scarce runner. Keep on-demand gates scheduled and risk-triggered; branch protections must still block merges where needed.
- Distinguish flaky infra from test failures by bounded targeted rerun and evidence, not green-washing or automatic passing after fail.

## Proof
For every claimed speedup present before/after runs of comparable changed path classes, critical path graph, p50/p95, CI minutes per integrated PR and cache hit rate. Mutation tests: remove one required test/gate, inject unknown path, force a red job, stale plan and absent status: verdict MUST reject. Validate merge readiness on actual app-bound checks and current head SHA. Keep exact acceptance target and explicit measured result separate.

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
