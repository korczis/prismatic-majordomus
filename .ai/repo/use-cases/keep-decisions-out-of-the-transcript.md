---
id: keep-decisions-out-of-the-transcript
kind: use-case
title: 'Record a decision so the next worker can find it'
summary: 'Write down what was decided and why as state, then find it again without reading a conversation.'
category: continuity
status: active
target: guaranteed
actors: [agent, contributor]
difficulty: basic
commands: [decision, search, history]
doctrines: [majordomus.decision-records, majordomus.ledger-integrity]
claims: [decision-record, record-search, history-ledger-read, event-vocabulary]
responsibilities: [state, watch]
applications: [long-running-work, several-agents-one-repository]
scenario:
  setup: active-task
  given:
    - 'an active task scoped to lib'
  steps:
    - id: decide
      run: ['decision', 'add', 'refuse tabs in the parser', '--why', 'two encodings for one token']
      note: 'what was decided, why, and which task decided it; superseded by a later entry, never edited'
      expect:
        exit: 0
        stdout_contains: ['recorded: refuse tabs in the parser']
    - id: read-back
      run: ['decision', 'list']
      note: 'the decisions of this branch, newest first'
      expect:
        exit: 0
        stdout_contains: ['refuse tabs in the parser', 'Why: two encodings']
    - id: find-it
      run: ['search', 'tabs']
      note: 'a literal scan over the durable records, no index'
      expect:
        exit: 0
        stdout_contains: ['decision', 'match']
    - id: what-happened
      run: ['history']
      note: 'the ledger names the event and the task'
      expect:
        exit: 0
        stdout_contains: ['decision.recorded', 'task.started']
  then:
    - 'a decision is one line of state with an author, a task and a reason'
    - 'nothing has to be re-explained to the next session'
---

# Situation

A session decides something in the middle of a task: a trade-off, a refusal, a convention. The decision lives in the conversation, so the next worker either re-derives it, contradicts it, or reads the whole transcript to find it.

# Outcome

The decision is a record naming the task that made it and why; `decision list` reads it back, `search` finds it by a word, and the ledger says when it happened. A later decision supersedes it by naming it; nothing is edited.
