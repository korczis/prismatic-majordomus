# Recovery Protocol for Interrupted or Failed Cleanup Campaigns

Use this when a phase is interrupted, a rebase/merge goes wrong, or repository state becomes ambiguous.

## Prime directive

Do not "fix" an ambiguous state by resetting or deleting until you identify the last verified checkpoint.

## 1. Reconstruct current state

Inspect:

- current repo root,
- current worktree,
- HEAD and branch,
- `git status --porcelain=v2 --branch`,
- in-progress merge/rebase/cherry-pick/revert state,
- worktree list,
- reflog,
- campaign ledger/handover,
- safety refs/tags/bundle,
- dirty/untracked files.

## 2. Determine last completed phase

Use repository evidence, commits, handover state, and campaign ledger.

Do not rely on conversational memory.

## 3. If rebase is in progress

Before aborting or continuing:

- inspect current conflict set,
- inspect original branch safety ref,
- verify preserved dirty/untracked work,
- determine whether conflict resolutions already encode valuable manual work.

If continuing, resolve semantically and validate.

If aborting, confirm safety ref/recovery checkpoint first.

## 4. If merge is in progress

Determine whether merge was intentional per integration DAG.

Do not commit a half-understood conflict resolution just to leave the state clean.

Abort only after confirming pre-merge state is recoverable.

## 5. If a worktree directory disappeared

Do not manually fabricate `.git` files.

Inspect `git worktree list --porcelain` and `.git/worktrees` metadata.

If branch/ref still exists, recreate worktree canonically.

If only detached/unreachable commits remain, recover from safety refs/reflog/bundle first.

## 6. If branch was deleted prematurely

Recover from, in preference order:

- campaign safety ref/tag,
- Git bundle,
- reflog,
- recorded old HEAD OID,
- remote ref if still present.

Recreate branch at exact proven OID.

## 7. If untracked files were removed

Recover from campaign-preserved copy/checksum location.

Git cannot recover arbitrary untracked files it never knew about. This is why phase 02 exists instead of optimism.

## 8. If origin/master moved concurrently

Do not force-push.

Fetch, compare histories, update integration plan, and replay/rebase validated local integration as repository policy requires.

Rerun affected tests before push.

## 9. Resume protocol

After recovery:

- restore clean, well-understood state,
- update campaign ledger,
- rerun the current phase from its preconditions,
- do not skip safety checks because they already "probably" happened.
