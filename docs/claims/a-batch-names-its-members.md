# A batch names its members

## What it means

Several pull requests may land in one merge, and when they do the branch that carries them
says which ones, at which heads and in what order, in a tracked file a tool can read. A
branch that merges the heads of two or more open pull requests without that record, or with
a record git disagrees with, does not reach master: a gate of every CI plan refuses it and
names the difference.

## How it works

`majordomus prs compose --apply` builds a batch in a scratch worktree: one `--no-ff` merge
per member onto the current master, then one composition commit holding the manifest,
`.ai/repo/integration/batches/<id>.yaml` of kind `integration-batch/v1`. The manifest lists
each member's number, the head that was merged and the merge commit that carries it.

`majordomus prs batch-check` is the gate. It walks the first-parent line from the merge base
to the head. A merge there whose second parent is the current head of another open pull
request of this repository is a member merge. Two or more of them, or a manifest added or
changed whatever the branch merges, make the branch a batch to be judged: it must carry
exactly one manifest, whose members are exactly those merges in that order, and every other
commit on the line may change only the manifest, the version files and `merge=derived`
paths. Each disagreement is one typed finding with the commit, the path or the manifest's
line. The open heads are the only thing asked of the forge, and only when git alone cannot
decide.

## How to see it

```bash
majordomus prs compose          # the batch that would be composed, and who is left out
majordomus prs batch-check      # this branch against the integration base
majordomus prs explain <n>      # a batch lists its members; a member names its batch
bash test/run.sh 1000_a_composed_branch_that_is_not_a_batch_is_refused
```

## What it does not cover

A member is an open pull request as the forge lists it when the gate runs: a batch merged
before the gate existed is not judged, and neither is a branch that merges commits no open
pull request has as its head. One merged pull request with no manifest is a stack, not a
batch. The gate proves the record and the shape; it does not say which member broke a red
batch. That is found by `git bisect --first-parent` over the batch's range, and reading the
failed jobs to name a member is not built. Composing is refused in a repository whose layer
does not hold ADR 0114 as accepted.

## Why it exists

On 2026-10-07 and 2026-10-08 five batches were built by hand and sixteen pull requests
landed through them. No required check was bypassed, and no tool could say afterwards what
any of the five had carried: membership lived in a pull request's prose, and four of the
five held fixes written on the batch branch, which belong to no member. ADR 0114 records the
decision to make the batch the executor's act instead of forbidding it.
