+++
title = "Reconcile the plan with merged work, and refuse an issue another already executes"
description = "Ask which moves a merge supports and what each delivered issue still owes, apply only what the transition guard admits, and see whether an issue this head authors or executes is another session's."
weight = 31
[extra]
id = "reconcile-the-plan-with-merged-work"
source = ".ai/repo/use-cases/reconcile-the-plan-with-merged-work.md"
category = "completion"
maturity = "guaranteed"
+++

## Situation

Pull requests merge and the issues they deliver stay READY. A milestone reads 0/13 while
its work has landed, and a person or an agent then spends hours moving issues one stamp at
a time. Meanwhile a second session takes the same issue, because its picker ran before the
first session's pull request existed.

## What you run

- `plan reconcile [<id>...]`: the plan.transition moves a merge supports now (start, and
  done only on evidence recorded since the merge; never verify), the held ones with the
  guard's reason, every evidence token owed, and the acceptance criteria as unjudged
- `plan reconcile --apply [--done]`: the same moves, written one plan.transition each
- `plan collisions [--issue <id>] [--gate start|push|pr]`: an id another line authored,
  or an issue another open pull request or session executes, with its holder named;
  `start --issue` asks it before a task is written

## Scenario

```yaml
setup: plan-model
given:
  - 'installed, with a canonical project model carrying one milestone and two issues'
steps:
  - id: reconcile
    run: ['plan', 'reconcile']
    note: 'nothing has merged, and with no forge observation the proposal says it is unsettled'
    expect:
      exit: 0
      stdout_contains: ['^plan reconcile — live', 'unsettled', 'classes: recordable 0']
  - id: collisions
    run: ['plan', 'collisions']
    note: 'a head that adds no record and executes no issue collides with nothing'
    expect:
      exit: 0
      stdout_contains: ['executes:  · authors: ', 'plan collisions: clean']
then:
  - 'a merge is proposed as a start, never as an attestation or an acceptance'
  - 'the second executor of an issue is told who holds it before its work lands'
```

## Outcome

The plan says what landed without anyone retyping it, DONE still needs the evidence an issue
requires, and two sessions no longer finish the same issue unaware of each other.
