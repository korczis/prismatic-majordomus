---
id: read-back-what-happened
kind: use-case
title: 'Read back what happened, and keep the ledger within its cap'
summary: 'Read the append-only ledger as operational history, filtered by task and event, validate every line, and rotate the oldest lines into an archive rather than deleting them.'
category: knowledge
status: active
target: guaranteed
weight: 74
actors: [reviewer, maintainer]
difficulty: basic
commands: [history, search, watch]
doctrines: [majordomus.ledger-integrity, majordomus.retention-caps]
claims: [history-ledger-read, ledger-integrity, event-vocabulary, record-search, record-retention, retention-caps]
responsibilities: [state, watch]
applications: [long-running-work, several-agents-one-repository]
scenario:
  setup: active-task-records
  given:
    - 'an active task that has already produced a checkpoint, a decision and an open question'
  steps:
    - id: history
      run: ['history']
      note: 'the ledger read back oldest first, one event per line, from a closed vocabulary'
      expect:
        exit: 0
        stdout_contains: ['task.started', 'task.checkpoint', 'decision.recorded']
    - id: by-event
      run: ['history', '--event', 'task.checkpoint']
      note: 'only the events of one kind'
      expect:
        exit: 0
        stdout_contains: ['task.checkpoint']
        stdout_not_contains: ['decision.recorded']
    - id: valid
      run: ['history', '--validate']
      note: 'every line is a well-formed event; a malformed line would be a failure, not a skipped record'
      expect:
        exit: 0
        stdout_contains: ['every event is registered', 'history --validate: ok']
    - id: find
      run: ['search', 'parser']
      note: 'a durable record found literally, across kinds, without an index'
      expect:
        exit: 0
        stdout_contains: ['^decision ', '^checkpoint ', 'match']
    - id: caps
      run: ['watch']
      note: 'the ledger, the handovers and the checkpoints are under their retention caps'
      expect:
        exit: 0
        stdout_contains: ['^OK   retention   ledger', '^OK   retention   checkpoints', '0 drift finding']
  then:
    - 'every event has a name from the closed vocabulary'
    - 'the ledger is never silently repaired or truncated'
    - 'rotation archives, it does not delete'
---

# Situation

Something happened to the task last week: a checkpoint, a decision, a handover. The person asking was not there. The ledger has every event, but reading a JSON file end to end is not an answer, and a ledger with a corrupt line in it is worse than none if the tool skips it quietly.

# What you run

- `history`: the ledger as operational history, newest lines by default, `--task`, `--event`, `--since` to narrow
- `history --validate`: every line well-formed, exit 10 otherwise
- `search <text>`: durable records matched literally across kinds
- `watch`: the retention caps on the ledger, the handovers and the checkpoints

# Outcome

What happened is readable by task, event and time, a malformed line is a failure rather than a skipped record, and the ledger stays within its cap by rotating the oldest lines into an archive.
