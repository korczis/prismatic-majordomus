---
schema: moment/v1
id: task-in-progress-for-three-weeks
kind: moment
title: 'A task "in progress" that nobody has touched for three weeks'
short_title: 'Stale in progress'
hook: 'found a task "in progress" that nobody had touched for three weeks'
summary: 'A status that was authored rather than computed is true at the moment of writing and decays silently from then on.'
status: stable
severity: medium
frequency: common
weight: 70
featured: true
audiences: [engineering-lead, ai-native-team, solo-builder, agency]
areas: [work-tracking, coordination]
lifecycle: [implementation, maintenance]
tags: [tasks, staleness, drift, ownership]
signals:
  - id: stale-in-progress
    text: 'Something is marked in progress that nobody has touched for weeks.'
  - id: branch-behind
    text: 'A branch for active work is far behind the trunk and nobody noticed.'
  - id: finish-restart-delete
    text: 'Nobody can say whether an old piece of work should be finished, restarted or deleted.'
examples:
  - id: three-week-migration
    audience: engineering-lead
    title: 'The migration on the board'
    before: 'The board says in progress; the branch is forty commits behind and the session that owned it ended in August.'
    after: '`watch` reports the checkpoint age against the interval the profile sets, as drift, with its own exit code.'
  - id: overnight-worker-stopped
    audience: ai-native-team
    title: 'A worker that stopped mid-task'
    before: 'An overnight worker stopped somewhere; the task record still claims the work is under way.'
    after: 'The record''s identity was computed from git and is compared with git now: `exact`, `advanced`, `diverged` or `different_context`.'
  - id: engagement-handover
    audience: agency
    title: 'Handing over an engagement'
    before: 'Four of five worktrees carry a task record that no longer describes anything real, and nothing said so.'
    after: 'Each record names the checkout it belongs to, and a stale one is reported rather than trusted.'
commands: [checkpoint, watch, check]
capabilities: [health.report, repository.info]
claims: [git-identity, worktree-ownership, checkpoint-record, checkpoint-interval, consistency-check, drift-watch]
doctrines: [majordomus.checkpoint-freshness, majordomus.state-consistency, majordomus.task-continuity, majordomus.one-worker-one-scope]
use_cases: [checkpoint-long-work, find-out-what-drifted, resume-in-the-right-worktree]
related: [two-agents-one-bug, abandoned-worktree, milestone-status-unreconstructable]
aliases: ['stale task', 'abandoned work in progress', 'status decay']
---

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
