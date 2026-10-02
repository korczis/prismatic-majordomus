# Phase 08: Prune Merged Worktrees, Branches, Stashes, and Stale Git Metadata

## Goal

Now that integration and preservation are proven, remove obsolete Git state decisively.

This is the intentionally destructive phase. Follow the contract strictly.

## 1. Recompute, do not trust old labels

Refresh remote state and recompute candidate safety.

A branch/worktree is deletable only if current evidence proves one of:

- fully merged into integration branch,
- patch-equivalent to integrated state,
- superseded with preserved recovery reference and explicit campaign decision,
- stale with no unique valuable work.

## 2. Worktree removal

For each obsolete worktree:

- verify clean or preserved dirty state,
- verify branch disposition,
- remove using `git worktree remove` where possible,
- verify directory removal,
- verify metadata removal,
- preserve intentionally retained exceptions.

Do not `rm -rf` first and then hope Git notices.

## 3. Local branch pruning

Delete local branches only after verifying:

- integration/equivalence,
- no unique unpreserved commits,
- no active worktree uses them,
- no policy requires retention.

Use non-force deletion where possible.

If force deletion is required for a proven superseded branch, ensure safety ref/bundle evidence exists and record why ordinary merged detection was insufficient.

## 4. Remote branch pruning

Remote feature branch deletion is allowed only if repository workflow clearly permits it and branch is conclusively obsolete.

Do not delete remote branches merely to make `git branch -a` pretty.

If deleting:

- record remote branch OID,
- verify integrated/superseded proof,
- delete explicitly,
- fetch/prune and verify.

## 5. Stash cleanup

Re-evaluate every stash against final integrated tree.

Drop only stashes proven:

- integrated,
- duplicated,
- superseded and safely preserved elsewhere,
- intentional local config that has been exported to a safer non-stash location.

Retain unknown stashes and report them rather than destroying evidence.

## 6. Worktree metadata prune

After ordinary removals:

- run safe `git worktree prune` according to policy,
- verify `git worktree list --porcelain`,
- inspect `.git/worktrees` only for anomalies,
- repair locks/stale metadata deliberately.

## 7. Reflog/dangling cleanup

Do not aggressively expire reflogs or run destructive GC solely for aesthetics.

The goal is logical cleanup, not erasing recovery history immediately.

If repository maintenance policy calls for GC, preserve campaign recovery checkpoints first and use conservative settings.

## 8. Final Git topology snapshot

Record:

- worktrees,
- local branches,
- remote branches relevant to campaign,
- stashes,
- safety refs/tags/bundle location,
- integration branch OID,
- origin integration OID.

## Acceptance criteria

- obsolete worktrees are gone,
- stale metadata is gone,
- merged/superseded local branches are pruned appropriately,
- remote branch cleanup follows policy,
- stash graveyard is reduced without data loss,
- active topology matches canonical policy,
- recovery evidence still exists through final validation.
