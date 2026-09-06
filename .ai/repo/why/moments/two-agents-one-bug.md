---
schema: moment/v1
id: two-agents-one-bug
kind: moment
title: 'Two agents fixing the same bug in two branches'
short_title: 'Duplicated work'
hook: 'found two agents fixing the same bug in two branches'
summary: 'Two workers spend a day each on one defect because ownership was implicit and neither could see the other.'
status: stable
severity: high
frequency: common
weight: 20
featured: true
audiences: [ai-native-team, engineering-lead, solo-builder, platform-team]
areas: [coordination, work-tracking]
lifecycle: [implementation, review]
tags: [agents, parallelism, duplicate-work, ownership]
signals:
  - id: same-subsystem-twice
    text: 'Two workers changed the same subsystem this week without either knowing about the other.'
  - id: no-named-owner
    text: 'Nobody could name the owner of a piece of work that was already under way.'
  - id: worktrees-same-defect
    text: 'More than one worktree or branch existed for one unresolved defect.'
examples:
  - id: parallel-agents
    audience: ai-native-team
    title: 'Two coding agents, one failing test'
    before: 'Both find the cause, both write a fix, both open a change; one is discarded and merging the other now conflicts with it.'
    after: 'The second start reports that another worktree already holds a task whose scope contains that path, and names the worktree.'
  - id: solo-two-windows
    audience: solo-builder
    title: 'Two windows, one afternoon'
    before: 'A session started in the morning is still open in another window; the afternoon session re-derives the same fix from the same failing test.'
    after: 'One task is active per checkout, so the second start is refused until the first is handed over or finished.'
  - id: overnight-then-morning
    audience: engineering-lead
    title: 'An overnight run nobody read'
    before: 'A developer spends the morning on a defect an overnight worker already fixed in a branch that was never merged or mentioned.'
    after: 'The overnight worker left a handover record naming the branch, the head and the changed files, and the morning session resolves it before starting.'
commands: [start, check, finish]
capabilities: [repository.scope, repository.scope_classify]
responsibilities: [scope, state]
claims: [scoped-task, overlap-report, scope-enforcement]
doctrines: [majordomus.scope-integrity, majordomus.one-worker-one-scope, majordomus.isolated-parallelism]
use_cases: [run-several-workers-at-once, resume-in-the-right-worktree]
related: [task-in-progress-for-three-weeks, one-agent-undoes-another, worker-output-never-integrated]
aliases: ['duplicate agent work', 'two workers same task', 'conflicting parallel fixes']
---

## The moment

Two sessions, two branches, the same failing test. Both find the cause, both fix it, both
open a change. One is wasted, and merging the other now conflicts with it.

## Why it happens

Each worker knew its task and nothing about the other's. Giving every worker its own git
worktree feels like the fix, and it does stop them overwriting each other's files. It does
not stop them doing the same work. In the environment this tool was distilled from, forty
fully isolated worktrees still produced several thousand concurrently modified files and
dozens of duplicated patches; nine of eighty ever merged. Isolation converted immediate
overwrites into deferred, larger conflicts.

## Why a better model does not fix it

Neither worker made a mistake. Each solved the problem it was given, correctly, with the
information available to it — and the information that would have prevented the waste was
never written anywhere either of them could read. A more capable model on both sides
produces two better duplicate fixes, faster.

## What it costs

The obvious cost is the discarded day. The larger one is the conflict: two independent
fixes to one subsystem are harder to reconcile than one fix, so the waste is paid twice,
once in the writing and once in the merge. Where the duplicated work reaches review, it
also costs a reviewer's attention on a change that should never have existed.

## What Majordomus does

`majordomus start` requires a scope: the paths this task may touch, normalised at the time
of declaration and stored in the task record. Every other worktree of the repository is read
straight from `git worktree list`; if one has an active task whose scope contains or is
contained by yours, `start` says so and names the worktree and the path. `check` and
`finish` fail on any touched file outside the scope. One task is active per checkout, so a
second `start` is refused until the first is handed over or finished.

## Before and after

```text
before   worker A: fix flaky auth test    (branch a, no declaration)
         worker B: fix flaky auth test    (branch b, no declaration)
         merge:    two fixes, one conflict

after    worker A: start --scope lib/auth,test/auth
         worker B: start --scope lib/auth
                   OVERLAP ../checkout-a holds lib/auth (task t-0091, active)
```

## How to verify it

Declare a scope in one worktree, then start an overlapping task in another and read what it
says. The overlap is reported with the worktree and the path, and `check` refuses a file
outside the declared scope.

## What it does not do

Overlap is reported, never blocked: whether two people should work on overlapping paths is
a coordination decision, not a rule. It sees worktrees of one repository on one machine,
not other clones. It does not merge anything and emits no merge commands.
