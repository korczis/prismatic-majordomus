+++
title = "State read back is labelled exact, advanced, diverged or different_context against git"
description = "Whenever a task record or a handover is read, its recorded head and branch are compared with the current checkout and labelled: exact (same commit), advanced (the recorded commit is an ancestor of the current one — normal progress), diverged (history was rewritten or the record is stale), different_context (another branch). Stale state is detected and named, never silently trusted."
weight = 43
[extra]
claim_id = "divergence-label"
status = "guaranteed"
source = "docs/claims/divergence-label.md"
+++
{% raw %}

## What it means

Whenever a task record or a handover is read, its recorded `head` and `branch` are compared with the current checkout and labelled: `exact` (same commit), `advanced` (the recorded commit is an ancestor of the current one — normal progress), `diverged` (history was rewritten or the record is stale), `different_context` (another branch). Stale state is detected and named, never silently trusted.

## How it works

`mj_git_label` in `lib/common.sh` compares branches, then commits, then runs `git merge-base --is-ancestor <recorded> HEAD`. `check` fails on `diverged` and `different_context`; `handover --resolve` prints the label under `Git state:` together with the record; `watch` reports either as state drift.

## How to see it

```bash
majordomus start "t" --scope lib && git commit -qam step
majordomus check                      # OK state … — advanced (head …)
git checkout -b elsewhere && majordomus check   # FAIL state … — recorded on branch 'main', now on 'elsewhere'
```

## What it does not cover

The label describes git history, not whether the content of the record is still true. A `Current State` section can be wrong on an `exact` commit.

## Why it exists

Every other "memory file" scheme studied had no staleness signal at all; notes carried dates in their names that disagreed with their content and nothing compared either with git. Read-time labelling was the one mechanism that made stored state safe to act on.
{% endraw %}
