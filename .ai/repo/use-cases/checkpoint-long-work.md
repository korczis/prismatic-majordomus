---
id: checkpoint-long-work
kind: use-case
title: 'Checkpoint long work so a stop costs minutes, not the day'
summary: 'Record compact progress inside the active task at the profile interval, so a session that ends without warning leaves the next one a place to start.'
category: continuity
status: active
target: guaranteed
weight: 25
actors: [agent, contributor]
difficulty: basic
commands: [checkpoint, check, context]
doctrines: [majordomus.checkpoint-freshness, majordomus.state-consistency, majordomus.task-continuity]
claims: [checkpoint-record, git-identity, consistency-check, continuity-reachable]
responsibilities: [state]
applications: [long-running-work]
scenario:
  setup: active-task
  given:
    - 'an active task scoped to lib, with work done inside that scope'
  steps:
    - id: checkpoint
      run: ['checkpoint']
      stdin: checkpoint-body.md
      note: 'a capped progress record; its branch and head are computed from git, never written by hand'
      expect:
        exit: 0
        stdout_contains: ['\.ai/local/state/checkpoints/']
    - id: fresh
      run: ['check']
      note: 'the checkpoint is fresh against the profile interval and the task is consistent with git'
      expect:
        exit: 0
        stdout_contains: ['0 failing']
    - id: resume
      run: ['context']
      note: 'the newest checkpoint is part of what the next worker reads'
      expect:
        exit: 0
        stdout_contains: ['^## TASK', 'checkpoint']
  then:
    - 'the checkpoint file carries the branch and head it was written at'
    - 'check reports the checkpoint age against the profile interval'
    - 'context names the task and its newest checkpoint'
---

# Situation

A task runs for hours. The session that holds it can be cut off by a context limit, a crash or a person closing the laptop, and everything the worker learned since the last durable record goes with it. A transcript is not a record; it is not resumable and it is not the tool's to keep.

# What you run

- `checkpoint`: a short body on stdin becomes a capped record under the local half, with identity computed from git
- `check`: reports whether the checkpoint is fresh against the profile's interval, beside scope and blockers
- `context`: the next worker reads the newest checkpoint inside the assembled context, not a pasted log

# Outcome

Progress exists as a file the tool can find, capped so that it stays a summary, and the profile decides how stale is too stale. A session that stops mid-task loses at most one interval.
