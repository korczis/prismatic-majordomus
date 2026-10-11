# 05 — Close the release and installation proof chain

## Objective
A released binary is reproducibly identified, verifiably associated with reviewed source/evidence and actually works in an unrelated repository. Verify all supported platforms declared in the canonical distribution model.

## Tasks
1. Audit `.github/workflows/release.yml`, `.ai/repo/ci/release.yaml`, `share/distribution.yaml`, Cargo version, version workflow, attestation/install logic, `docs/generated/distribution-matrix.json`, PR #872 and provenance commit history. Detect latest version and published release live, not from pack.
2. Verify full chain: tag matches manifest → `ci` success on exact SHA → matrix build → per-artifact checksums/signature/provenance attestation → publish atomic release → installer verifies → native platform smoke → GitHub pages/product links consistent.
3. Use clean ephemeral external repositories (including no `.ai/` existing, pre-existing `.ai/`, local overrides, readonly dirs, non-Git dirs if supported). Run one-step install, init, env, CLI help, doctor, API/MCP basic capability, server stop, update and rollback where supported. Never assume source-tree assets exist in installed binary.
4. Verify no path escape, dangling symlink, privilege escalation, secret log leakage, wrong-arch artifacts, partial upgrade corruption or downgrade mismatch. No `curl | sh` hard assumption of trusted data; enforce digest/provenance verification as designed.
5. Ensure version bump policy does not create parallel writers or release loops. Deduplicate tags, stale release jobs, and out-of-order publishing.
6. Make release/install/deploy state typed and visible from canonical capabilities, not a separate UI status. `unknown` when publication/live state unobserved.

## Proof
For supported OS/arch combinations provide exact build and smoke job links, artifact digests, attestation verification results, foreign-repo runtime test, rollback result and version parity. Do not publish if any mandatory platform or provenance check fails. If release permissions missing, prepare and test artifacts locally, stop release step transparently.

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
