+++
title = "A task \"in progress\" that nobody has touched for three weeks"
description = "Why an abandoned task looks active, and how computed identity, checkpoint intervals and drift reports tell a live task from a dead one."
weight = 7
[extra]
hook = "found a task \"in progress\" that nobody had touched for three weeks"
responsibilities = ["state", "watch"]
commands = ["checkpoint", "watch", "check"]
claims = ["git-identity", "worktree-ownership", "checkpoint-record", "checkpoint-interval", "consistency-check", "drift-watch"]
+++
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

## What Majordomus does

A task record is not a status. Its identity — branch, head, worktree — is computed from git
at `start` and never typed, and reading the record back compares those facts with git now,
labelling the result `exact`, `advanced`, `diverged` or `different_context`. A record names
the checkout it belongs to, and another checkout is never held to its scope.

The profile a task runs under sets a checkpoint interval. `majordomus checkpoint` records
what was true a moment ago, capped in length so the next context can quote it whole; a body
over the cap is refused rather than truncated, and an empty body simply proves the task is
alive. `majordomus check` reports checkpoint age beside state, scope and blockers.
`majordomus watch` reports it as drift, with the interval it exceeded, together with every
other drift it can see — policy, projection, state, scope, handover, verification,
retention — and exits with its own code, so a script can tell "drifted" from "broken".

## What it does not do

It does not close, reassign or delete a stale task; that is a decision, and it is reported
to whoever makes it. `watch` never blocks anything. It knows the worktrees of one repository
on one machine, so a task in a clone it cannot see is invisible to it.

## Try it

```bash
majordomus watch          # DRIFT checkpoint  t-… — last checkpoint 21d ago, interval 30m
majordomus check --explain
echo "still on the migration; step 3 of 5, the backfill is next" | majordomus checkpoint
```
