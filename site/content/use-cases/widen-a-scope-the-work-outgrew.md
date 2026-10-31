+++
title = "Widen a scope the work outgrew, and say why"
description = "A change lands outside the scope the task claimed; widen the scope with a reason instead of closing the task, and see check accept the change under the same task id."
weight = 37
[extra]
id = "widen-a-scope-the-work-outgrew"
source = ".ai/repo/use-cases/widen-a-scope-the-work-outgrew.md"
category = "completion"
maturity = "guaranteed"
+++

## Situation

A task claimed the paths it meant to change, and the work found one it did not foresee: a gate that wants a page updated, an advisory that wants a lock bumped. `check` refuses the change, rightly. Closing the task and starting another costs its id and a handover; editing the record by hand is what the record must never need.

## What you run

- `check`: names the file outside the claimed scope and refuses
- `scope widen`: adds the path with the reason; the task keeps its id, and the record and the ledger say who widened it, when and why
- `check`: accepts the change under the widened scope

## Scenario

```yaml
setup: check-scope-violation
given:
  - 'an active task scoped to lib, with a change under docs that the task never claimed'
steps:
  - id: refused
    run: ['check']
    note: 'the change under docs is outside the claimed scope'
    expect:
      exit: 10
      stdout_contains: ['outside claimed scope']
  - id: unexplained
    run: ['scope', 'widen', 'docs']
    note: 'a widening without a reason is refused and writes nothing'
    expect:
      exit: 2
      stdout_contains: ['--because is required']
  - id: widen
    run: ['scope', 'widen', 'docs', '--because', 'the change needs its page']
    note: 'the path joins the scope; the task keeps its id'
    expect:
      exit: 0
      stdout_contains: ['^widened t-', 'added=docs', 'scope=lib.docs$']
  - id: accepted
    run: ['check']
    note: 'the change is now within the scope'
    expect:
      exit: 0
      stdout_contains: ['within scope']
then:
  - 'the task id is the one start gave'
  - 'the record names who widened the scope, when and why'
  - 'the ledger carries a task.scope_widened event'
```

## Outcome

The scope says what the task changes, including what the work taught it, and every widening carries its reason. Nothing was closed to widen it, and nothing was edited by hand.
