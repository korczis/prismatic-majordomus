---
id: catch-an-agent-that-says-it-is-done
kind: use-case
title: 'Catch an agent that says it is done when it is not'
summary: 'One task, start to finish: a worker strays outside its scope, claims it is finished, hands over, and is accepted only once the repository''s own verification passes.'
category: completion
status: active
target: guaranteed
weight: 1
actors: [agent, operator]
difficulty: basic
commands: [start, check, finish, handover, context]
doctrines: [majordomus.scope-integrity, majordomus.state-consistency]
claims: [scoped-task, overlap-report, scope-enforcement, finish-contract, handover-record, typed-outcome]
responsibilities: [scope, state]
---

# Situation

A coding agent is asked for one change. It makes the change, edits a file nobody asked it to
touch, and reports that it is done. Another agent is already working in the same paths. The
session ends before anyone reads the diff, and the next session starts from whatever the chat
remembered. Every one of those is ordinary, and none of them is a model problem.

# What you run

- `start`: claims the paths the task may touch, and reports another worker already claiming them
- `check`: refuses a touched file outside the claimed paths
- `finish`: evaluates the finish contract line by line and writes nothing while a line fails
- `handover`: writes the state the next session starts from, into the repository
- `context`: gives the next worker that state back, without the conversation that produced it

# Scenario

```yaml
setup: challenge
given:
  - 'a repository with Majordomus installed, and a second worktree whose worker already claims lib'
  - 'a test, test/parse.sh, that passes only when the parser message names the column'
steps:
  - id: claim-the-work
    note: 'The task claims lib. Another worker already claims lib in a second worktree, and start says so before any file is touched.'
    run: ['start', 'refuse tabs in the parser', '--scope', 'lib']
    expect:
      exit: 0
      stdout_contains: ['^started t-', 'INFO overlap', 'claims lib']
  - id: the-worker-strays
    note: 'The worker makes the change it was asked for, and also edits a note under docs, which the task never claimed.'
    worker: 'echo "tabs are refused" >> lib/a && echo "parser notes" >> docs/d'
    expect:
      exit: 0
  - id: check-refuses-the-stray-file
    note: 'check names the file outside the claimed scope and exits non-zero. It is a failure, not a warning.'
    run: ['check']
    expect:
      exit: 10
      stdout_contains: ['FAIL scope', 'docs/d']
  - id: the-worker-says-it-is-done
    note: 'The worker reports the task finished. finish evaluates the contract instead of believing it, and refuses.'
    run: ['finish', '--outcome', 'completed', '--verify-command', 'sh test/parse.sh']
    expect:
      exit: 10
      stdout_contains: ['FAIL scope', 'FAIL verification', 'refused']
  - id: the-worker-reverts-the-stray-file
    note: 'The stray edit is taken back.'
    worker: 'git checkout -- docs/d'
    expect:
      exit: 0
  - id: hand-over
    note: 'The session ends. What the next one needs is written into the repository, not left in a transcript.'
    run: ['handover']
    stdin: challenge-handover.md
    expect:
      exit: 0
      files_exist: ['.ai/local/state/handovers']
  - id: the-next-session-resumes
    note: 'A new session asks the repository what it needs to know, and gets the task, its scope and the handover back.'
    run: ['context']
    expect:
      exit: 0
      stdout_contains: ['## TASK', 'LATEST COMPATIBLE HANDOVER', 'name the column']
  - id: finish-refuses-a-failing-verification
    note: 'Scope is clean now, but the repository''s own test still fails. finish runs it and refuses again.'
    run: ['finish', '--outcome', 'completed', '--verify-command', 'sh test/parse.sh']
    expect:
      exit: 10
      stdout_contains: ['OK +scope', 'FAIL verification', 'refused']
      stdout_not_contains: ['FAIL scope']
  - id: the-worker-finishes-the-work
    note: 'The worker does what the handover said was left.'
    worker: 'echo "the message names the column" >> lib/a'
    expect:
      exit: 0
  - id: finish-accepts
    note: 'Every line of the contract passes, so the outcome is recorded, with the verification that proved it.'
    run: ['finish', '--outcome', 'completed', '--verify-command', 'sh test/parse.sh']
    expect:
      exit: 0
      stdout_contains: ['OK +scope', 'OK +verification', 'completed']
      stdout_not_contains: ['FAIL']
then:
  - 'a file outside the claimed scope is refused by check and by finish'
  - 'a worker saying it is done is a claim finish evaluates, and a failing verification keeps it refused'
  - 'the next session resumes from what the repository recorded, not from the conversation'
  - 'the work is accepted only when every line of the contract passes'
```

# Outcome

The work is accepted exactly once, when the repository's own test passes and nothing outside
the claimed paths changed. The two earlier claims of being done were refused with the reason
named and the command that reproduces it. The next session started from the handover the
repository held, and the other worker in the same paths was named before anything was edited.
