# 08 — Dogfood and install sustainable enforcement

## Objective
Prove the entire process on Majordomus itself and on a clean external repo, then install narrow regression ratchets that improve on every change without growing ceremonial governance.

## Tasks
1. Choose one actual user-intent-sized change touching canonical source, compiled runtime and a presentation surface. Run full lifecycle through the real CLI, test, PR, validate, merge, version/release if policy permits and external install. No simulated completions.
2. Check every lifecycle transition's emitted evidence ID, revision, provenance and freshness. Verify failed dependencies produce blocked/unknown and prevent false completion.
3. Add regression gates to existing .ai model: no unchecked duplicate domain registries, generated drift, missing source/criterion links, stale check SHA, unsafe cleanup, release without attestation, unbounded network messages, or UI optimistic status. Reuse existing validator; do not add ceremonial Markdown-only rules.
4. Benchmark the exact same representative flow before/after changes: median/p95 lead time, CI queue/runtime, PR merge age, number of rebases/conflicts, automation manual steps, release smoke time, UI error correctness.
5. Perform adversarial review: supply deliberately corrupt JSONL, invalid signature, revoked trust, expired lease, slow HTTP client, stale evidence and unknown provider. Record real outputs and fixes.
6. Delete obsolete code/duplicated generators, update ADR addenda where decisions change, keep docs synchronized and release notes derived. Ensure acceptance probes themselves fail when you intentionally remove required implementation.
7. Inspect final `git diff`, origin HEAD, release and GitHub checks; report strictly measured achievements, open issues and why. Do not declare production ready unless operational evidence exists.

## Proof
The acceptance matrix is completed with links and machine-readable evidence records; every line marked PASS refers to executed code and checked SHA. Remaining failures listed with owner, risk, and next concrete patch. One clean end-to-end demonstration is more valuable than dozens of new declarations.

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
