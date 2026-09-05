---
id: hand-work-between-sessions
kind: use-case
title: 'Hand unfinished work to the next session'
summary: 'Stop mid-task and leave the next worker something to act on that is not a transcript.'
category: continuity
status: active
target: guaranteed
weight: 2
actors: [agent, contributor]
difficulty: basic
commands: [checkpoint, handover, context, check]
doctrines: [majordomus.handover-integrity, majordomus.note-integrity, majordomus.task-continuity, majordomus.context-budget]
claims: [handover-record, no-transcripts, divergence-label, minimum-context]
responsibilities: [handover, state]
applications: [several-agents-one-repository, long-running-work]
scenario:
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
---

# Situation

A session ends with the work half done. The usual handover is a paste of the conversation, which the next worker has to read in full to find the three facts that matter, and which is stale the moment the branch moves.

# What you run

- `checkpoint`: a short progress record inside the active task, refused if it grows into a report
- `handover`: an append-only record with computed front matter and the sections the policy requires
- `context`: the next session reads this instead of the transcript, within a line budget

# Outcome

The next worker starts from durable state with the git position it was written at, and is told how far the repository has moved since.
