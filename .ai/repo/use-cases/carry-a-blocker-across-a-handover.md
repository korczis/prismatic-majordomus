---
id: carry-a-blocker-across-a-handover
kind: use-case
title: 'Hand work over with a question still open, and keep it blocking'
summary: 'Close a task into a handover while a question is unresolved, start the follow-up task, and see the same question still refuse acceptance there.'
category: continuity
status: active
target: guaranteed
weight: 27
actors: [agent, reviewer]
difficulty: intermediate
commands: [handover, start, check, question, finish]
doctrines: [majordomus.handover-integrity, majordomus.blocker-resolution, majordomus.questions-store-integrity]
claims: [blocker-survives-handover, handover-record, open-question-gate, finish-contract, record-resolution]
responsibilities: [handover, finish, state]
applications: [long-running-work, several-agents-one-repository]
scenario:
  setup: finish-blocker-open
  given:
    - 'an active task with one unresolved question open against it'
  steps:
    - id: hand-over
      run: ['handover', '--close']
      stdin: handover-body.md
      note: 'the record the next session resumes from; --close marks the task handed over so a new one may start'
      expect:
        exit: 0
        stdout_contains: ['\.ai/local/state/handovers/']
    - id: follow-up
      run: ['start', 'the follow-up', '--scope', 'lib']
      note: 'the next task; the prior handover for this worktree and branch is named on the way in'
      expect:
        exit: 0
        stdout_contains: ['^INFO handover', 'same_worktree_same_branch', 'next: majordomus context']
    - id: still-blocked
      run: ['check']
      note: 'the question opened under the previous task still blocks this branch'
      expect:
        exit: 10
        stdout_contains: ['^FAIL blockers', 'unresolved question', 'tabs']
    - id: still-listed
      run: ['question', 'list']
      note: 'the entry, numbered, with the task it was opened under'
      expect:
        exit: 0
        stdout_contains: ['^1  \[unresolved\]', 'tabs']
    - id: refused
      run: ['finish', '--outcome', 'completed', '--verify-command', 'true']
      note: 'completion of the follow-up is refused by the same entry'
      expect:
        exit: 10
        stdout_contains: ['^FAIL blockers']
  then:
    - 'the handover carries the branch and head it was written at'
    - 'a question is a fact of the branch, not of the task that happened to open it'
    - 'nothing was resolved by handing over; the next worker sees the same blocker'
---

# Situation

A worker opened a question, ran out of session and handed the work over. The next worker starts a fresh task. If the question belonged to the old task, the new task would be acceptable with the question still unanswered, and the handover would have laundered a blocker into a footnote.

# What you run

- `handover --close`: the continuation record, and the task marked handed over
- `start <task> --scope <paths>`: the follow-up; the prior handover is named
- `check`: the unresolved question is a failing line on this branch
- `question list`: the entry, numbered, with the task that opened it
- `finish --outcome completed`: refused by the same entry

# Outcome

A blocking question keeps blocking after the work is handed to a new task. The record of who asked and under which task survives, and only an answer clears it.
