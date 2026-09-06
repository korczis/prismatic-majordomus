+++
title = "Hand unfinished work to the next session"
description = "Stop mid-task and leave the next worker something to act on that is not a transcript."
weight = 2
[extra]
id = "hand-work-between-sessions"
source = ".ai/repo/use-cases/hand-work-between-sessions.md"
category = "continuity"
maturity = "described"
+++

## Situation

A session ends with the work half done. The usual handover is a paste of the conversation, which the next worker has to read in full to find the three facts that matter, and which is stale the moment the branch moves.

## What you run

- `checkpoint`: a short progress record inside the active task, refused if it grows into a report
- `handover`: an append-only record with computed front matter and the sections the policy requires
- `context`: the next session reads this instead of the transcript, within a line budget

## Scenario

```yaml
setup: active-task
given:
  - 'an active task scoped to lib, with work done inside that scope'
steps:
  - id: checkpoint
    run: ['checkpoint']
    stdin: checkpoint-body.md
    note: 'a short progress record inside the task'
    expect:
      exit: 0
      stdout_contains: ['\.ai/local/state/checkpoints/']
  - id: handover
    run: ['handover']
    stdin: handover-body.md
    note: 'the record the next session resumes from, with the sections the policy requires'
    expect:
      exit: 0
      stdout_contains: ['\.ai/local/state/handovers/']
  - id: resume
    run: ['context']
    note: 'what the next worker reads instead of a transcript'
    expect:
      exit: 0
      stdout_contains: ['^## GIT', '^## TASK', 'narrow the parser']
  - id: verify
    run: ['check']
    note: 'the task is still consistent with git'
    expect:
      exit: 0
      stdout_contains: ['0 failing']
then:
  - 'the handover carries the branch and head it was written at'
  - 'the next context names the task, the profile and the scope'
```

## Outcome

The next worker starts from durable state with the git position it was written at, and is told how far the repository has moved since.
