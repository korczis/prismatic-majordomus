+++
title = "Overlap between parallel workers is computed on claimed paths in both containment directions"
description = "When a task starts, Majordomus looks at every other worktree of the same repository, reads its active task record if it has one, and reports any scope entry that contains or is contained by yours: lib/auth overlaps lib/auth/oauth in both directions. It is a report, not a block."
weight = 44
[extra]
claim_id = "overlap-report"
status = "guaranteed"
source = "docs/claims/overlap-report.md"
+++
{% raw %}

## What it means

When a task starts, Majordomus looks at every other worktree of the same repository, reads its active task record if it has one, and reports any scope entry that contains or is contained by yours: `lib/auth` overlaps `lib/auth/oauth` in both directions. It is a report, not a block.

## How it works

`lib/start.sh` runs `git worktree list --porcelain`, opens `.ai/local/state/current.yaml` in each other worktree, keeps only records with outcome `active`, and tests every pair of paths with `mj_path_contains` in both orders after normalising. `check --overlap` runs the same report on demand. There is no sidecar registry to rot; git is the authority on which worktrees exist.

## How to see it

```bash
# in worktree A
majordomus start "refresh tokens" --scope lib/auth/oauth
# in worktree B
majordomus start "auth cleanup" --scope lib/auth
# INFO overlap     wt-a — claims lib/auth/oauth — contained by your lib/auth  [reproduce: majordomus check --overlap]
```

## What it does not cover

It sees worktrees of one repository on one machine. Whether two people should work on overlapping paths is a coordination decision, so the report never blocks.

## Why it exists

The source environment matched claims by exact string equality, so `apps/x` and `apps/x/` never overlapped and a subtree never overlapped its parent. Its two consumers even disagreed about the matching rule. One normalised matching function, used by every caller, is the fix.
{% endraw %}
