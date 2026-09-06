+++
title = "Two sessions in one checkout, standing on each other"
description = "Two workers in one working tree see each other only as unexplained file changes, and each treats the other as noise."
weight = 160
[extra]
id = "second-session-same-checkout"
status = "stable"
source = ".ai/repo/why/moments/second-session-same-checkout.md"
+++
{% raw %}

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
{% endraw %}
