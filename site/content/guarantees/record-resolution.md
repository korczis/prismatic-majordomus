+++
title = "The right prior record is resolved by worktree and branch, and an unrelated one is never offered"
description = "handover --resolve, checkpoint --show and context all answer the same question: of the records on disk, which one is about the work happening here? The answer is deterministic, it is explained, and when there is no safe answer it says so instead of offering the nearest thing."
weight = 58
[extra]
claim_id = "record-resolution"
status = "guaranteed"
source = "docs/claims/record-resolution.md"
+++
{% raw %}

## What it means

`handover --resolve`, `checkpoint --show` and `context` all answer the same question: of the records on disk, which one is about the work happening here? The answer is deterministic, it is explained, and when there is no safe answer it says so instead of offering the nearest thing.

## How it works

`mj_resolve_latest` in `lib/common.sh` reads every record's front matter and keeps only candidates in the same repository, then ranks:

1. same worktree **and** same branch
2. same branch, when the branch is not detached
3. nothing

There is no repository-wide tier. A record from another worktree or another branch is never a fallback, because a worker cannot tell that borrowed context is wrong until it has already acted on it.

Within a tier the newest wins. `created_at` has second resolution, so two records written inside one second would otherwise be ordered by their random filename suffixes; the ledger is append-only and written in command order, so `mj_record_rank` uses position in the ledger as the tiebreak — the one portable monotonic ordering available without sub-second timestamps.

The result carries its reasoning: the match class, the git label comparing the recorded head with the current one, the record's age, and its task. A malformed record is skipped **with a warning on stderr**, never silently, and `doctor` fails when any record was skipped.

## How to see it

```bash
majordomus handover --resolve
# Handover: .ai/local/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee12.md
# Match: same_worktree_same_branch
# Git state: advanced
# Created: 2026-09-03T20:14:55Z (37m ago)
# Task: t-20260903193012-a4f1

git checkout -b unrelated
majordomus handover --resolve
# No relevant handover.
```

## What it does not cover

It does not judge the content. A record that resolves may still be wrong; the git label tells you how far the repository has moved, and the rest is your judgement.

It does not follow a record across a rewritten history. A `diverged` label means the recorded commit is not an ancestor of the current one, and the record is reported rather than reinterpreted.

It does not merge records. The newest compatible one is offered whole.

## Why it exists

The failure mode this prevents is specific and quiet: a repository accumulates records, a session on an unrelated branch resolves the globally newest one, and the worker proceeds confidently against another task's state. Absence is better than incorrect memory, so the resolver stops rather than reaching further.
{% endraw %}
