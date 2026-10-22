+++
title = "Two branches advanced from one trunk never land on one version, and a dependency edit beside the version line still merges"
description = "Two branches that start from the same trunk and each advance the version do not both land"
weight = 41
[extra]
claim_id = "concurrent-branches-never-share-a-version"
status = "guaranteed"
source = "docs/claims/concurrent-branches-never-share-a-version.md"
+++
{% raw %}

## What it means

Two branches that start from the same trunk and each advance the version do not both land
claiming the same number. If both advanced `1.10.0` to `1.11.0` and one lands, the other is
brought to `1.12.0` before it can land; a branch that advanced once against a trunk that has
since advanced twice goes one past the trunk. A major that one side owes is never lowered by
the merge, and a dependency edit made beside the version line is kept.

## How it works

`apps/majordomus-cli/Cargo.toml` and `Cargo.lock` carry `merge=version`, generated into
`.gitattributes` from the writer's own constants. The driver, `majordomus release
merge-version`, reads the version each side declares, rewrites the base and both sides to the
greater of the two with the writer's own rewrite, and merges what remains with git's three-way
merge — so the version line cannot conflict and every other line merges or conflicts exactly
as before. The number the merge result carries is then decided by `release advance` against
the trunk it now contains: `majordomus prs drain --refresh` and `scripts/unblock` both merge
with the driver named by their own executable and advance before deriving.

## How to see it

```bash
just derive-merge-driver                        # declares merge.derived and merge.version
git merge origin/master                         # the version line merges by itself
majordomus release obligation --base origin/master
majordomus release advance --base origin/master
```

## What it does not cover

GitHub's own merge of a pull request does not run local drivers; a branch is reconciled
locally before it lands. Two edits of the same dependency are still a conflict for a person to
settle. The driver never chooses the final number; without the advance that follows, the
merged tree carries the greater of the two versions and its obligation says so.

## Why it exists

Every branch that carries work writes the version line, so without this every refresh of a
branch would conflict on a number neither side should be choosing by hand, and the easiest
resolution — keep either side — is the one that lands two branches on one version.
{% endraw %}
