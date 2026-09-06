+++
title = "A task \"in progress\" that nobody has touched for three weeks"
description = "A status that was authored rather than computed is true at the moment of writing and decays silently from then on."
weight = 70
[extra]
id = "task-in-progress-for-three-weeks"
status = "stable"
source = ".ai/repo/why/moments/task-in-progress-for-three-weeks.md"
+++
{% raw %}

## The moment

The board says the migration is in progress. The branch is three weeks old, its head is
forty commits behind main, and the worktree it lives in belongs to a session that ended in
August. Nobody knows whether to finish it, restart it or delete it.

## Why it happens

"In progress" was written once, by whoever started the work, and nothing was ever obliged
to say it again. A status that is authored rather than computed is true at the moment of
writing and decays from then on, silently, at the same rate as everything around it moves.
The task registry this tool was distilled from carried a stale entry for four worktrees in
five at one audit; the fix was a repair command that nobody ran, because nothing reported
that it needed running.

## Why a better model does not fix it

There is no worker involved in the failure. The record went stale between sessions, when
nothing was running at all. The only fix is for the status to be derived from facts that
move on their own — git, the filesystem, the clock — rather than stored as a word.

## What it costs

Work is either duplicated or abandoned, and both are decided by guessing. The wider cost is
that the board stops being read: once a few entries are known to be stale, every entry is
treated as unreliable, and the team goes back to asking people.

## What Majordomus does

A task record is not a status. Its identity — branch, head, worktree — is computed from git
at `start` and never typed, and reading the record back compares those facts with git now,
labelling the result `exact`, `advanced`, `diverged` or `different_context`. A record names
the checkout it belongs to, and another checkout is never held to its scope.

The profile a task runs under sets a checkpoint interval. `majordomus checkpoint` records
what was true a moment ago, capped in length so the next context can quote it whole; a body
over the cap is refused rather than truncated. `majordomus check` reports checkpoint age
beside state, scope and blockers. `majordomus watch` reports it as drift, with the interval
it exceeded, together with every other drift it can see, and exits with its own code, so a
script can tell "drifted" from "broken".

## Before and after

```text
before   board: "in progress"          (written once, in July)

after    majordomus watch
           DRIFT checkpoint  t-20260812… — last checkpoint 21d ago, interval 30m
           DRIFT state       t-20260812… — diverged (+40 commits on master)
```

## How to verify it

Start a task, wait past its checkpoint interval, and run `watch`: the drift is reported with
the interval it exceeded. Move the branch on and read the record back: the divergence label
changes without anyone editing it.

## What it does not do

It does not close, reassign or delete a stale task; that is a decision, and it is reported
to whoever makes it. `watch` never blocks anything. It knows the worktrees of one repository
on one machine, so a task in a clone it cannot see is invisible to it.
{% endraw %}
