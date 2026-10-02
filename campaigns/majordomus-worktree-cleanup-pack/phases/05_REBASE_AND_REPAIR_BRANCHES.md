# Phase 05: Rebase and Repair Candidate Feature Branches

## Goal

Prepare merge-worthy branches against the latest verified integration base while preserving semantics and minimizing future conflicts.

Execute in DAG order.

## 1. Preconditions per branch

Before rebasing or reconstructing:

- worktree is in canonical location,
- dirty state is either committed coherently or preserved,
- safety ref/checkpoint exists,
- upstream/shared-history risk is known,
- integration base OID is known,
- dependent foundation branches already integrated or deliberately selected as rebase base,
- relevant tests can be invoked.

## 2. Rebase carefully

For safe local feature branches, rebase onto the current verified integration branch.

When conflicts occur:

- understand both sides semantically,
- inspect surrounding architecture and later commits,
- preserve canonical single-source-of-truth patterns,
- remove legacy duplication rather than carrying both implementations,
- regenerate derived artifacts from canonical sources instead of hand-merging generated text,
- update tests alongside behavior,
- do not resolve by blindly choosing ours/theirs.

## 3. Reconstruct when rebase is the wrong tool

If branch history is tangled, stale, or mixes unrelated concerns:

- create a fresh branch from current integration base,
- port the intended semantic changes in coherent commits,
- reuse tests to prove equivalence,
- compare final diff against original branch,
- preserve original branch as recovery evidence until integration completes.

Do not fetishize preserving bad commit history.

## 4. Remove obsolete artifacts during repair

Where conflicts expose legacy duplication:

- prefer canonical implementation,
- delete stale generated copies,
- update imports/routes/registries consistently,
- keep one source of truth.

Do not broaden scope into unrelated refactors unless required to make the branch conform to current architecture.

## 5. Validation per branch

Run focused checks before declaring a branch ready:

- formatting,
- compile/build of affected packages,
- focused unit tests,
- affected integration tests,
- schema validation,
- generated drift checks,
- docs checks if touched,
- worktree topology check.

Record exact commands/results.

## 6. Remote feature branches

If rebasing requires updating an existing remote feature branch:

- verify policy permits rewriting,
- use `--force-with-lease`, never blind `--force`,
- verify lease target,
- record old and new OIDs.

Do not push rewritten feature branches unless needed for repository workflow.

## 7. Ready marker

A branch is `integration-ready` only when:

- rebased/reconstructed against correct base,
- conflicts semantically resolved,
- focused tests pass,
- generated artifacts are synchronized,
- no accidental dirty state remains,
- branch disposition remains valid after rebase.

## Acceptance criteria

All branches scheduled for integration are either:

- integration-ready,
- intentionally dropped as superseded with proof,
- explicitly retained as not-ready and removed from current merge campaign.
