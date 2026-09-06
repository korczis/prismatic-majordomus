+++
title = "Read only the context that fits the budget, and know what was dropped"
description = "Assemble the context a worker needs now in authority order, cut it to a line budget, and see every dropped section named with its reason."
weight = 13
[extra]
id = "read-only-the-context-that-fits"
source = ".ai/repo/use-cases/read-only-the-context-that-fits.md"
category = "continuity"
maturity = "described"
+++

## Situation

A worker that reads everything reads nothing well. The state is spread over the task record, the layer’s scoped documents, the questions store, the decisions and the checkpoints, and a fresh session cannot know which of it matters now. Pasting all of it into a prompt is expensive and wrong in a different way each time.

## What you run

- `context`: assembled from durable state in a fixed authority order, shaped by the task’s profile
- `context --budget-lines <n>`: the same, within a budget; dropped sections are listed by name and reason
- `context --json`: the same as one document
- `check --explain`: the policy and profile that shaped it

## Scenario

```yaml
setup: active-task-records
given:
  - 'an active task that has already produced a checkpoint, a decision and an open question'
steps:
  - id: everything
    run: ['context']
    note: 'git identity, the task and its profile, the scoped documents, open questions, decisions and the newest checkpoint, in authority order'
    expect:
      exit: 0
      stdout_contains: ['^## GIT', '^## TASK', '^## PROFILE']
  - id: budgeted
    run: ['context', '--budget-lines', '30']
    note: 'the same, cut to a budget; what is dropped is named with its reason rather than silently truncated'
    expect:
      exit: 10
      stdout_contains: ['^## GIT', '^## EXCLUDED', 'context budget 30 lines']
  - id: machine
    run: ['context', '--json']
    note: 'the same body as one JSON document for a client that composes its own prompt'
    expect:
      exit: 0
      stdout_contains: ['^\{']
  - id: explained
    run: ['check', '--explain']
    note: 'the effective policy and profile the context was assembled under'
    expect:
      exit: 0
      stdout_contains: ['^# task', 'profile']
then:
  - 'authority order is fixed: git, task, profile, documents, questions, decisions, checkpoint'
  - 'a dropped section is named, never lost without a trace'
  - 'the JSON form carries the same content as the text form'
```

## Outcome

The next worker reads a short, ordered, complete-enough context and knows exactly what was left out. Nothing is in it that the profile excludes, and nothing is invented.
