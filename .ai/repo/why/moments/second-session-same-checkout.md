---
schema: moment/v1
id: second-session-same-checkout
kind: moment
title: 'Two sessions in one checkout, standing on each other'
short_title: 'One checkout, two sessions'
hook: 'had two sessions writing into one checkout without either knowing'
summary: 'Two workers in one working tree see each other only as unexplained file changes, and each treats the other as noise.'
status: stable
severity: high
frequency: occasional
weight: 160
audiences: [solo-builder, ai-native-team, agency]
areas: [coordination]
lifecycle: [implementation]
tags: [parallelism, ownership, worktrees, state]
signals:
  - id: files-changing-underneath
    text: 'Files changed under a session while it was working, and nothing said why.'
  - id: two-windows-one-repo
    text: 'More than one assistant window is pointed at the same working directory.'
  - id: conflicting-uncommitted
    text: 'The working tree holds uncommitted changes from more than one piece of work.'
examples:
  - id: two-windows
    audience: solo-builder
    title: 'Two windows, one directory'
    before: 'A second session is opened for a quick fix; both sessions commit into one tree and the history interleaves two unrelated pieces of work.'
    after: 'One task is active per checkout: the second `start` is refused until the first is handed over or finished, and it says which task holds it.'
  - id: agent-and-human
    audience: ai-native-team
    title: 'A worker and a person'
    before: 'An overnight worker is still running when somebody starts editing the same tree in the morning.'
    after: 'The active task record names the checkout and the scope; the overlap is reported before the second piece of work begins.'
  - id: shared-machine
    audience: agency
    title: 'A shared build box'
    before: 'Two people on one checkout produce a commit that contains both of their changes and neither of their intentions.'
    after: 'Scope is declared per task and `check` refuses files outside it, so the mixture is caught before the commit.'
commands: [start, check, handover]
capabilities: [repository.scope, peers.list, peers.announce]
responsibilities: [scope, state]
claims: [scoped-task, worktree-ownership, overlap-report, consistency-check, mcp-peers]
doctrines: [majordomus.one-worker-one-scope, majordomus.scope-integrity, majordomus.state-consistency]
use_cases: [run-several-workers-at-once, resume-in-the-right-worktree, serve-the-layer-to-ai-clients]
related: [one-agent-undoes-another, two-agents-one-bug, abandoned-worktree]
aliases: ['shared working tree', 'two agents one directory', 'concurrent sessions']
---

## The moment

Two assistant windows are open on the same directory. Each sees files it did not write
appear and change under it. Each is doing exactly what it was asked. The commit that
results contains both pieces of work and explains neither.

## Why it happens

A working tree has no concept of an owner. Nothing in git objects to two writers, and the
signal that would tell one worker about the other — an active task, its scope, its worktree
— does not exist by default. Opening a second window costs nothing and is often the right
thing to do; what is missing is the record that makes it safe.

## Why a better model does not fix it

Each worker's model of the world is correct for what it can observe. It observes a file
changing and has no channel through which the other worker could have declared itself.

## What it costs

Interleaved commits that cannot be reverted independently, verification that runs against a
tree containing somebody else's half-finished change, and time spent working out which of
two people or workers is responsible for a failure.

## What Majordomus does

One task is active per checkout: a second `start` is refused, naming the task that holds it,
until the first is handed over or finished. Each task declares its scope, and `check` and
`finish` fail on files touched outside it. Across checkouts, the other worktrees are read
from `git worktree list` and an overlapping active scope is reported at `start`. Where
several AI clients attach to this repository's shared server, each can announce what it is
working on and read what the others announced.

## Before and after

```text
before   window 1: editing lib/auth      window 2: editing lib/auth
         (neither knows)

after    $ majordomus start "tidy the token parser" --scope lib/auth
         refused: task t-20260906044337 is active in this checkout
                  (hand it over or finish it first)
```

## What it does not do

It does not lock the filesystem and it does not stop anyone from editing. It refuses to
record a second concurrent task in one checkout, and it reports overlap across worktrees;
the coordination decision stays with the people.
