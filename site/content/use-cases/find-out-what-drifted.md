+++
title = "Find out what has drifted since anyone last looked"
description = "Ask what has moved, rather than whether anything is wrong."
weight = 8
[extra]
id = "find-out-what-drifted"
source = ".ai/repo/use-cases/find-out-what-drifted.md"
category = "drift"
maturity = "described"
+++

## Situation

The policy changed but the projections did not. A generated file was hand-edited. A task record describes a commit the branch no longer contains. Each is invisible until something downstream behaves oddly.

## What you run

- `watch`: reports drift across policy, projections, state, scope, records and retention
- `update`: --dry-run shows what regeneration would change before it changes it

## Scenario

```yaml
setup: policy-drifted
given:
  - 'installed and generated, then the policy was edited without regenerating'
steps:
  - id: detect
    run: ['watch']
    note: 'the projection no longer matches the policy it was rendered from'
    expect:
      exit: 11
      stdout_contains: ['DRIFT']
  - id: regenerate
    run: ['update']
    note: 'the projections are rendered again from the edited policy'
    expect:
      exit: 0
      stdout_contains: ['CLAUDE.md']
  - id: clean
    run: ['watch']
    note: 'nothing disagrees any more'
    expect:
      exit: 0
      stdout_contains: ['watch: 0 drift']
  - id: what-happened
    run: ['history']
    note: 'the ledger says what changed and when'
    expect:
      exit: 0
      stdout_contains: ['projections.updated']
then:
  - 'drift is a deterministic disagreement between policy, projection and git, reported by name'
```

## Outcome

Every finding carries the command that reproduces it, and the same rules produce it as produce the failures — one vocabulary, one registry, a different question.
