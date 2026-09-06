+++
title = "A change that no plan ever asked for"
description = "Work arrives with no bounded contract behind it, so its scope, its acceptance criteria and its ownership are all decided after the fact."
weight = 210
[extra]
id = "code-without-an-issue"
status = "stable"
source = ".ai/repo/why/moments/code-without-an-issue.md"
+++
{% raw %}

## The moment

A substantial change is in the trunk. It is not bad work. Nothing asked for it, nothing
bounded it, and the question of whether it should have been done at all is now being
answered retrospectively, by whoever is reviewing it.

## Why it happens

Producing a change became much cheaper than writing down what the change is for. When a
worker can implement an idea in ten minutes, the ten minutes it takes to state the idea as a
bounded contract looks like pure overhead — until the change turns out to touch four
subsystems, or to be the wrong idea, and the overhead is paid at ten times the price.

## Why a better model does not fix it

An unbounded instruction produces unbounded work, and a better worker produces more of it,
faster, and defends each part of it convincingly. Boundaries are an input, not something
capability supplies.

## What it costs

Review that has to reconstruct intent from a diff. Merge conflicts with work that had a
boundary. And an accumulating body of change that nobody can map to a reason, which is the
state in which a codebase becomes hard to reason about.

## What Majordomus does

Work is bounded at the moment it starts. `majordomus start` requires an objective and a
scope; the scope is normalised and stored, and `check` and `finish` fail on any touched file
outside it. Where the repository carries a plan, an issue is the execution contract: the
paths it may touch, the paths it must not, what it is for, the issues it depends on, its
acceptance criteria and the evidence its completion requires — with no status field, because
status is derived. Commits are attributed to the task that produced them.

## Before and after

```text
before   "improve error handling"   ->  31 files, 4 subsystems, one review

after    $ majordomus start "retry the enqueue on transient errors" \
             --scope lib/queue,test/queue
         $ majordomus check
         FAIL scope  lib/http/router.rs is outside the declared scope
```

## What it does not do

It does not require an issue for every change; a repository with no plan is skipped rather
than failed. It requires that whatever work is under way has said what it may touch.
{% endraw %}
