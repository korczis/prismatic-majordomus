---
schema: moment/v1
id: code-without-an-issue
kind: moment
title: 'A change that no plan ever asked for'
short_title: 'Work with no contract'
hook: 'found a substantial change that no issue, ticket or plan ever asked for'
summary: 'Work arrives with no bounded contract behind it, so its scope, its acceptance criteria and its ownership are all decided after the fact.'
status: stable
severity: medium
frequency: common
weight: 210
audiences: [engineering-lead, enterprise, solo-builder, agency]
areas: [work-tracking]
lifecycle: [planning, implementation]
tags: [plan, scope, ownership, tracking]
signals:
  - id: no-ticket
    text: 'A significant change landed this month that no issue or plan item asked for.'
  - id: scope-decided-after
    text: 'What a piece of work was allowed to touch was decided while it was being reviewed.'
  - id: cannot-say-why
    text: 'Nobody can say what problem a recent change was meant to solve.'
examples:
  - id: while-i-was-in-there
    audience: solo-builder
    title: '"While I was in there"'
    before: 'A one-line fix becomes a refactor of the surrounding module because nothing bounded it.'
    after: 'The task declares its scope at `start`; `check` and `finish` fail on files touched outside it.'
  - id: unbounded-agent
    audience: engineering-lead
    title: 'A worker given an outcome and no boundary'
    before: 'A worker asked to "improve error handling" edits thirty files across four subsystems, all defensibly.'
    after: 'An issue is an execution contract: the paths it may touch, what it is for, its acceptance criteria and the evidence completion requires.'
  - id: change-without-provenance
    audience: enterprise
    title: 'A change with no stated purpose'
    before: 'The audit trail for a change is the commit message, which says what was done and not why it was authorised.'
    after: 'The commit carries the task, and the task carries the objective, the scope and the outcome.'
commands: [start, plan, check]
capabilities: [repository.scope, repository.scope_classify]
claims: [scoped-task, scope-enforcement, project-schema, task-commit-attribution, task-dependencies]
doctrines: [majordomus.scope-integrity, majordomus.define-done-first, majordomus.project-integrity, project.scope-is-declared]
use_cases: [plan-the-work-as-data, complete-an-issue-only-with-its-evidence, accept-or-refuse-finished-work]
related: [three-roadmaps-none-of-them-true, issue-says-done-tests-disagree, who-did-this-and-under-what-policy]
aliases: ['untracked work', 'scope creep', 'no ticket']
---

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
