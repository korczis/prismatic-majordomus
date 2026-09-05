---
id: block-acceptance-on-an-open-question
kind: use-case
title: 'Block acceptance on a question nobody has answered'
summary: 'Open a question against the task, watch finish refuse completion while it is unresolved, resolve it with the answer, and only then complete.'
category: completion
status: active
target: guaranteed
weight: 52
actors: [agent, reviewer]
difficulty: intermediate
commands: [question, finish, check]
doctrines: [majordomus.blocker-resolution, majordomus.questions-store-integrity, majordomus.verification-integrity]
claims: [open-question-gate, blocker-store, finish-contract, typed-outcome, consistency-check]
responsibilities: [finish, state]
applications: [long-running-work, ci-gated-project]
scenario:
  setup: finish-blocker-open
  given:
    - 'an active task with one unresolved question open against it'
  steps:
    - id: see-it
      run: ['question', 'list']
      note: 'the open question, numbered, on this branch'
      expect:
        exit: 0
        stdout_contains: ['unresolved', 'tabs']
    - id: refused
      run: ['finish', '--outcome', 'completed', '--verify-command', 'true']
      note: 'completion is refused while the question is open; the refusal names it'
      expect:
        exit: 10
        stdout_contains: ['question']
    - id: still-flagged
      run: ['check']
      note: 'check reports the blocker as a failing line, not a warning'
      expect:
        exit: 10
        stdout_contains: ['FAIL']
    - id: resolve
      run: ['question', 'resolve', '1', '--answer', 'tabs are refused; the subset has no tabs']
      note: 'the one line is rewritten as resolved with its answer'
      expect:
        exit: 0
        stdout_contains: ['resolved']
    - id: clear
      run: ['check']
      note: 'nothing blocks now'
      expect:
        exit: 0
        stdout_contains: ['0 failing']
  then:
    - 'finish refused with exit 10 while the question was open'
    - 'the resolved entry keeps the question and the answer together'
    - 'check is green once the store holds no unresolved entry'
---

# Situation

Somebody asked a question that decides how the work is done, nobody answered, and the work carried on. If the task can be accepted anyway, the question was never really a blocker, and the next person discovers the assumption when it is already merged.

# What you run

- `question list`: the unresolved entries for the active task, numbered
- `finish --outcome completed`: refused while an entry is unresolved, with the entry named
- `check`: the same gate as a failing line, so it is visible before anyone tries to finish
- `question resolve <n> --answer`: rewrites that one line as resolved, with the answer beside the question

# Outcome

An open question is a blocker the tool enforces, not a note somebody may read. The record of what was asked and what was decided stays in the repository, and finishing over it is impossible rather than discouraged.
