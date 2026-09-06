+++
title = "Prove a rule is actually enforced, not merely written down"
description = "Answer \"is this rule real?\" with a command rather than a reading of the source."
weight = 3
[extra]
id = "prove-a-rule-is-enforced"
source = ".ai/repo/use-cases/prove-a-rule-is-enforced.md"
category = "policy"
maturity = "described"
+++

## Situation

A repository says a rule is enforced. A script exists, a test exists, and nothing invokes the script. Every artifact of enforcement is present and the enforcement is fiction — and no amount of reading the files tells you, because each one looks right.

## What you run

- `doctrine`: list what is declared, its class, and the validator that decides it
- `doctor`: walks declaration to validator to dispatch to exit code to test to CI, from the source
- `check`: run one rule against the current task with --rule

## Scenario

```yaml
setup: installed
given:
  - 'Majordomus installed with the policy declaring two enforcements'
  - 'no git hook invokes the tool yet'
steps:
  - id: list-the-rules
    run: ['doctrine', 'list']
    note: 'every rule with its class, and whether the tool enforces it'
    expect:
      exit: 0
      stdout_contains: ['majordomus.enforcement-wiring', 'blocking']
  - id: find-the-gap
    run: ['doctor']
    note: 'the enforcement the policy declares reaches no hook: named, with the fix'
    expect:
      exit: 10
      stdout_contains: ['^FAIL wiring', 'doctor-on-commit']
  - id: read-the-status
    run: ['doctrine', 'status']
    note: 'the resolved rule set as the tool applies it'
    expect:
      exit: 0
      stdout_contains: ['doctrine']
then:
  - 'a declared enforcement that nothing invokes is a failure, not a green line'
```

## Outcome

Every link of the chain is resolved from the source rather than from the registry's description of itself, and a break is named. A validator no rule declares fails too.
