+++
title = "A misplaced worktree is brought to its canonical path with its modified, staged, unstaged and untracked work intact, verified by a fingerprint taken before and after the move, and a step is reported as moved only when the two are equal"
description = "majordomus worktree migrate brings a worktree to its canonical path exactly as it is:"
weight = 153
[extra]
claim_id = "worktree-migration-lossless"
status = "guaranteed"
source = "docs/claims/worktree-migration-lossless.md"
+++
{% raw %}

## What it means

`majordomus worktree migrate` brings a worktree to its canonical path exactly as it is:
modified, staged, unstaged and untracked files included, the branch and HEAD unchanged.
Each move is fingerprinted before and after and reported as moved only when the two
fingerprints are equal.

## How it works

`migrate::plan` lists every misplaced worktree with a branch, the container's occupant
first, with what blocks each. `migrate::apply` recomputes it under the repository lock and,
per step, captures a `WorktreeFingerprint` — branch, HEAD, the index, the staged diff, the
unstaged diff, every untracked file with its content hash, every ignored entry by presence
— runs `git worktree move` with absolute paths, captures the fingerprint at the destination,
checks git registers it there, and compares. The occupant of the container goes out to a
staging path and then in; a move across filesystems is made by copy, `git worktree repair`,
verification against a manifest of every entry, and only then removal of the original, and
only with `--allow-copy`. Relative links that no longer resolve are named.

## How to see it

```bash
majordomus worktree migrate --plan      # dirty counts per step; nothing changes
majordomus worktree migrate --format json | jq '.steps[] | {branch, outcome, differences}'
```

## What it does not cover

A locked worktree is blocked until unlocked; a detached worktree has no canonical path and
is never moved; a session's scratch checkout is moved only with `--include-ephemeral`; a
destination that exists is never overwritten. The fingerprint attests ignored content by
presence and size, not by content: it moves with the directory as everything else does.

## Why it exists

"Automatically move the current worktrees" without a proof of preservation would have been
a more sophisticated way of gambling with unfinished work.
{% endraw %}
