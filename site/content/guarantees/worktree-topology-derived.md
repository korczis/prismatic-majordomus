+++
title = "A branch's worktree path is derived from git identity and the branch name alone — the primary checkout's sibling named with -wt, then the branch with its hierarchy kept — and is the same answer from every directory of every worktree, with nothing registered or configured"
description = "Given the repository and a branch name there is exactly one path for that branch's"
weight = 164
[extra]
claim_id = "worktree-topology-derived"
status = "guaranteed"
source = "docs/claims/worktree-topology-derived.md"
+++
{% raw %}

## What it means

Given the repository and a branch name there is exactly one path for that branch's
worktree — `<repo>-wt/<branch>`, the primary checkout's sibling named with `-wt`, then the
branch's components as directories — and every surface computes it the same way from git's
own identity. No file, section, table or list records it, and adding a branch edits nothing.

## How it works

`RepositoryIdentity::discover` reads `git rev-parse --git-common-dir` and `git worktree
list --porcelain`; the first record is the primary checkout, whatever directory the command
ran in. `path::container_root` appends the suffix to its name; `path::expected_path` joins
the validated branch name and proves the result stays strictly below the container.
`WorktreeService::judge` gives every registered worktree a standing — primary, canonical,
misplaced, detached, ephemeral, missing — and every condition a `DiagnosticCode` with a
remedy. The four `worktree.*` capabilities project the typed `RepositoryTopology`; the
command line renders it; `worktree guard` decides for the pre-commit hook.

## How to see it

```bash
majordomus worktree path feature/providers/streaming   # <repo>-wt/feature/providers/streaming
cd "$(majordomus worktree root)"/feature/x && majordomus worktree root   # the same container
majordomus worktree topology --format json | jq .worktrees[].standing
```

## What it does not cover

A git hook is bypassed with `--no-verify`, and a CI runner cannot see a contributor's local
directories: the gate holds the rule's machinery together, not this machine's filesystem.
Bare repositories are refused by name.

## Why it exists

Fifty-three worktrees in four shapes, eleven of them dirty, and no answer to "where is
branch X" that did not involve looking or inventing. ADR 21 records the decision.
{% endraw %}
