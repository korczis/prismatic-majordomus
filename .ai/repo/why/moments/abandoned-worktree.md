---
schema: moment/v1
id: abandoned-worktree
kind: moment
title: 'A worktree nobody can decide to delete'
short_title: 'Abandoned worktree'
hook: 'found five worktrees and could not say which of them still mattered'
summary: 'Isolation is cheap to create and expensive to reason about: nothing records what a worktree was for or whether its work landed.'
status: stable
severity: medium
frequency: common
weight: 140
audiences: [ai-native-team, solo-builder, agency]
areas: [coordination, work-tracking]
lifecycle: [maintenance, handover]
tags: [worktrees, parallelism, cleanup, staleness]
signals:
  - id: many-worktrees
    text: 'There are more worktrees or branches than anyone can account for.'
  - id: safe-to-delete
    text: 'Nobody can say whether a given branch or worktree still holds anything worth keeping.'
  - id: created-and-forgotten
    text: 'Workers create isolated checkouts routinely and nothing records what for.'
examples:
  - id: forty-worktrees
    audience: ai-native-team
    title: 'Forty isolated checkouts'
    before: 'Every worker gets a worktree; after a fortnight there are dozens, each with uncommitted changes and no record of intent.'
    after: 'Each worktree''s active task record names its objective, its scope and its head, and reading it back says how far git has moved since.'
  - id: two-windows
    audience: solo-builder
    title: 'The branch from last month'
    before: 'A branch called `fix/auth-2` exists; whether it was abandoned or finished is a question only the diff can answer, slowly.'
    after: 'The record says `partial` with a next action, or there is no record and the branch is genuinely orphaned — which is itself an answer.'
  - id: end-of-engagement
    audience: agency
    title: 'Handing the repository back'
    before: 'The client inherits a dozen branches and a paragraph of explanation written from memory.'
    after: 'Each carries a handover record with objective, current state and next action, and the ledger says which were finished and with what outcome.'
commands: [handover, check, watch]
capabilities: [repository.info, health.report, worktree.topology, worktree.migration_plan]
claims: [worktree-ownership, git-identity, divergence-label, handover-record, drift-watch, worktree-topology-derived, worktree-migration-lossless]
doctrines: [majordomus.handover-integrity, majordomus.state-consistency, majordomus.isolated-parallelism, majordomus.one-worker-one-scope]
use_cases: [resume-in-the-right-worktree, hand-work-between-sessions, find-out-what-drifted, work-on-a-branch-in-its-canonical-worktree]
related: [task-in-progress-for-three-weeks, worker-output-never-integrated, two-agents-one-bug]
aliases: ['orphaned branches', 'worktree sprawl', 'is this branch dead']
---

## The moment

`git worktree list` prints five entries. Two are obviously current. The other three have
uncommitted changes, branches whose names are not quite descriptive, and no way to tell
whether they contain an afternoon of work or nothing at all.

## Why it happens

Creating isolation is one command and recording intent is nobody's job. A worktree is a
directory; a directory carries no objective, no owner and no state. So the decision to
delete one is always made on the evidence of a diff, which is expensive to read and
ambiguous when read.

## Why a better model does not fix it

Nothing is running. The information that would settle the question — what this was for, how
far it got, whether it was superseded — was never written by the session that knew it.

## What it costs

Either the sprawl is kept, in which case every audit repeats this question, or it is cleared,
in which case somebody occasionally deletes work. Both are paid repeatedly, and the second
one is paid loudly.

## What Majordomus does

A task record belongs to a checkout and carries the objective, the declared scope and the
git identity computed at `start`. Reading it back compares that identity with git now and
labels it `exact`, `advanced`, `diverged` or `different_context`, so a worktree announces
its own staleness. When a session ends, `handover` writes objective, current state and next
action, each required and non-empty. `watch` reports every drift it can see across the
records this repository holds.

The worktrees themselves have one topology: a branch's checkout is at `<repo>-wt/<branch>`,
derived from git's identity and never registered, so "where is branch X" is a derivation
and not a search. `worktree.topology` gives every checkout a standing — canonical,
misplaced, detached, ephemeral, missing — and every branch a verdict on cleanup
eligibility: merged into the trunk, and clean or not checked out. A stray checkout is
brought home with its uncommitted work, fingerprinted before and after; nothing is deleted
by the tool, and the list of what could go is derived state for a person to act on.

## Before and after

```text
before   $ git worktree list
         ../wt-3  8c31f0e  [fix/auth-2]        ...and then what?

after    $ majordomus handover --resolve --repo ../wt-3
         t-20260812…  diverged (-40 commits)
         # Objective: split the auth callback validation
         # Next Action: the legacy mobile form still needs a decision
```

## What it does not do

It does not delete or merge anything, and it cannot see clones on other machines. It reports
what each worktree says about itself; whether to keep one is a decision it hands to a person.
