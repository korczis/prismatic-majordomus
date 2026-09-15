# Phase 02: Preserve Dirty, Detached, Stashed, and Otherwise Unreachable Work

## Goal

Turn uncertain mutable state into recoverable evidence before any history rewrite or cleanup.

This phase is about preservation, not integration.

## 1. Establish a recovery boundary

Use repository policy to choose the strongest practical recovery mechanism.

Prefer a layered approach:

- safety refs/tags for important branch HEADs,
- a Git bundle containing relevant refs where practical,
- patches or copied files for untracked work,
- a machine-readable manifest of object IDs and paths,
- checksums for non-Git preserved files.

Place temporary recovery artifacts in a safe path that will not accidentally be committed. Use an existing repository temp/recovery convention if available.

Never put credentials or secrets into Git history.

## 2. Dirty tracked changes

For each dirty worktree:

- inspect staged/unstaged diff,
- determine whether changes belong to current branch intent,
- detect generated-only noise,
- detect local configuration,
- detect secrets,
- capture valuable changes using the least lossy method,
- record exact recovery instructions.

Do not blindly `git stash -u` every worktree and create a new graveyard.

Prefer a named, attributable preservation record.

If changes are coherent and clearly belong to the branch, committing them before rebase may be cleaner, provided commit policy permits and tests can run.

If they are incomplete, preserve as patch/safety branch rather than pretending they are production-ready.

## 3. Untracked and ignored files

Classify untracked files into:

- source that should be versioned,
- temporary local work needing preservation,
- generated artifacts reproducible from source,
- caches/build outputs,
- credentials/local config,
- obvious junk.

Do not delete any category until classification is evidenced.

Ignored files are not automatically junk. Inspect suspiciously important ignored paths, especially local state, handovers, or generated recovery data.

## 4. Detached HEAD worktrees

For every detached HEAD:

- determine whether HEAD is reachable from a named ref,
- inspect unique commits,
- create a safety ref if unique work exists,
- assign a semantic temporary branch name only when needed for recovery,
- record intended later action.

No unique commit may remain reachable only through a soon-to-expire reflog.

## 5. Stashes

For each stash:

- identify originating branch/worktree if possible,
- inspect diff and untracked payload,
- compare against current branch commits,
- classify duplicate/superseded/unique/config/unknown,
- preserve unique work in a more explicit form if needed.

Do not drop any stash yet unless it is trivially proven duplicate and the campaign has a recovery bundle/checkpoint that still covers it.

Prefer deferring drops to phase 08.

## 6. Dangling and reflog-only objects

Inspect recent unreachable commits/trees/blobs that plausibly correspond to current work.

Do not attempt to rescue every historical garbage object in the repo's lifetime.

Focus on:

- recent timestamps,
- commit messages matching active work,
- objects referenced by worktree/rebase metadata,
- commits close to current branch graphs.

Create safety refs for plausible valuable commits before pruning later.

## 7. Preservation proof

For each risky item, record:

```text
source
classification
preservation_method
recovery_location_or_ref
head/object IDs
checksums if applicable
recovery command/procedure
secret-handling note
```

## Acceptance criteria

Do not proceed until:

- every dirty worktree has recoverability evidence,
- every detached unique HEAD has a durable ref or equivalent recovery artifact,
- every unique stash is preserved or intentionally retained,
- plausible recent dangling work is classified,
- no risky untracked work is about to be lost,
- a campaign-wide recovery checkpoint exists and has been verified.
