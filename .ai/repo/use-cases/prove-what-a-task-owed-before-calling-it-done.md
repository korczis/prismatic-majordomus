---
id: prove-what-a-task-owed-before-calling-it-done
kind: use-case
title: Prove what a task owed before calling it done
summary: A task declares what it owes, records evidence against each obligation, and is refused when that evidence no longer describes the tree it was taken over.
category: completion
status: active
target: guaranteed
weight: 40
actors: [maintainer, agent]
difficulty: basic
commands: [start, evidence, check, finish]
doctrines: [majordomus.obligation-closure]
claims: [obligation-closure]
responsibilities: [finish]
applications: []
---

# Situation

A worker says the work is done. The finish contract already asks whether the change stayed
in its scope, whether a verification command ran, whether the record still matches the
branch and whether anything is open — all facts about the working tree. It does not ask
whether the tests that were run describe the code that now exists.

The gap is not that evidence is absent. It is that evidence, once recorded, stays true. A
test run before a change says nothing about the tree after it, and a report that treats the
two as one is how "done" comes to mean nothing.

# Scenario

```yaml
setup: task-owes
given:
  - a repository with the layer installed and a task active
steps:
  - id: owes-and-cannot-prove-it
    run: ['finish', '--outcome', 'completed', '--note', 'done']
    expect:
      exit: 10
      output: 'owed, and no evidence was recorded'
  - id: honest-is-never-refused
    run: ['check']
    expect:
      exit: 0
      output: 'obligation'
  - id: record-it
    run: ['evidence', '--covers', 'tests', '--type', 'test', '--command', 'bash test/run.sh']
    expect:
      exit: 0
      output: 'evidence: tests recorded'
  - id: now-it-discharges
    run: ['check']
    expect:
      exit: 0
      output: 'discharged over inputs'
then:
  - the obligation is discharged while the files it names are unchanged
  - a change to any of those files makes the same evidence stale, and finish refuses again
  - an outcome other than completed is never refused for owing something
```

# Outcome

The task cannot reach `completed` on assertion. Each obligation it declared is discharged by
a recorded command, and the discharge is bound to the hash of the files the obligation names,
so that changing them takes the proof away rather than leaving it behind. What a report says
about the work is then a fact about the repository instead of a claim about the worker.
