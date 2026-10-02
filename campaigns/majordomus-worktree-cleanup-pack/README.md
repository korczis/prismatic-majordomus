# Majordomus Worktree Cleanup & Merge Campaign

This pack is designed for Claude Code Fable/Opus with a very large context window and assumes it is executed inside the `prismatic-majordomus` repository.

It is not a "delete stale worktrees" script. It is a staged repository recovery and normalization campaign with explicit evidence, rollback points, deterministic integration, and permanent enforcement.

## Recommended usage

1. Unpack this directory somewhere under the repository, preferably a temporary location such as `tmp/majordomus-worktree-cleanup-pack/` if that matches current repository policy.
2. Start Claude Code from the canonical repository root.
3. Paste only `00_MASTER_ORCHESTRATOR.md` first.
4. Let Claude read `CONTRACT.md` and every file under `phases/` itself.
5. Do not manually cherry-pick random phases out of order unless recovering from a failed run.
6. If execution is interrupted, restart with the master orchestrator and tell Claude to resume from repository evidence and the canonical session/handover state, not from memory.

## Safety model

The campaign follows one non-negotiable invariant:

> No tracked, untracked, ignored-but-valuable, stashed, dangling, or worktree-local work may be destroyed until it has been classified, captured, and proven recoverable.

The cleanup can become aggressive only after preservation evidence exists.

## Intended end state

- canonical repository remains at its normal root,
- worktrees live under the inferred sibling `<repo>-wt/` root,
- each feature worktree maps deterministically to its own feature branch,
- branch/worktree path mapping is validated,
- stale/orphaned worktrees are gone,
- merged branches are pruned locally and remotely when repository policy permits,
- dirty feature work has either been integrated, preserved intentionally, or quarantined with explicit evidence,
- `master` is updated only through validated, serialized integration,
- remote `origin/master` is synchronized and verified,
- stashes/reflogs/dangling commits are audited before pruning,
- root and repository-local clutter is classified and cleaned,
- generated/cache/tmp artifacts are handled by policy rather than folklore,
- worktree lifecycle becomes enforceable through Majordomus tooling/rules/doctrines,
- relevant state is visible through existing CLI/API/OpenAPI/MCP/Cockpit/docs surfaces from one canonical source,
- no manual duplicate registry of worktrees/branches is introduced.

## Files

- `CONTRACT.md` — shared architectural and safety contract.
- `00_MASTER_ORCHESTRATOR.md` — paste this first.
- `phases/01_FORENSIC_INVENTORY.md`
- `phases/02_PRESERVE_DIRTY_AND_UNREACHABLE_WORK.md`
- `phases/03_NORMALIZE_WORKTREE_TOPOLOGY.md`
- `phases/04_BUILD_INTEGRATION_DAG.md`
- `phases/05_REBASE_AND_REPAIR_BRANCHES.md`
- `phases/06_SERIALIZED_INTEGRATION_TO_MASTER.md`
- `phases/07_REPOSITORY_CLUTTER_AND_DUPLICATION_CLEANUP.md`
- `phases/08_PRUNE_GIT_STATE.md`
- `phases/09_ENFORCE_WORKTREE_LIFECYCLE.md`
- `phases/10_EXPOSE_AND_DOCUMENT_CANONICAL_STATE.md`
- `phases/11_FINAL_PROOF_AND_HANDOVER.md`
- `RECOVERY.md` — recovery protocol for conflicts or interrupted runs.

## Execution philosophy

Do not optimize for the smallest diff. Optimize for a repository that is easier to reason about after the campaign than before it.

Do not paper over conflicting branches by merging everything indiscriminately. Build an integration DAG, identify superseded work, and integrate only what remains semantically necessary.

Do not preserve stale machinery merely because deleting it feels scary. Preserve *recoverability*, then remove dead state decisively.
