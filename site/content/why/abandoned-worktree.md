+++
title = "A worktree nobody can decide to delete"
description = "Isolation is cheap to create and expensive to reason about: nothing records what a worktree was for or whether its work landed."
weight = 140
[extra]
id = "abandoned-worktree"
status = "stable"
source = ".ai/repo/why/moments/abandoned-worktree.md"
+++
{% raw %}

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
{% endraw %}
