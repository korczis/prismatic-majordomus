---
id: gate-ci-on-the-tool-itself
kind: use-case
title: 'Gate CI on the tool itself, with an exit code that is a contract'
summary: 'Run the checks CI runs, read every finding with the command that reproduces it, and rely on exit codes that mean one thing each: no code means warn and continue.'
category: policy
status: active
target: guaranteed
weight: 44
actors: [maintainer, reviewer]
difficulty: basic
commands: [doctor, watch, doctrine, check]
doctrines: [majordomus.enforcement-wiring, majordomus.doctrine-wiring-integrity, majordomus.command-surface, majordomus.command-coverage]
claims: [wiring-reconciliation, exit-code-contract, reproduce-command, doctrine-class-decides, command-surface, command-coverage, no-network, derived-not-declared]
responsibilities: [doctor, watch]
applications: [ci-gated-project]
scenario:
  setup: installed-wired
  given:
    - 'installed, and the two enforcements the policy declares are actually in place as hooks'
  steps:
    - id: doctor
      run: ['doctor']
      note: 'every doctrine the registry declares is reached by the command that claims to run it; every finding carries a reproduce command'
      expect:
        exit: 0
        stdout_contains: ['^OK   wiring      doctor-on-commit', '^OK   command     surface', '^OK   command     coverage', 'doctor: 0 failure']
    - id: watch
      run: ['watch']
      note: 'drift since the last update: policy, projections, state, retention'
      expect:
        exit: 0
        stdout_contains: ['0 drift finding']
    - id: registry
      run: ['doctrine', 'list']
      note: 'which rules are enforced, by what, of which class, and whether each is wired'
      expect:
        exit: 0
        stdout_contains: ['^majordomus.enforcement-wiring +blocking', 'doctor']
    - id: one-rule
      run: ['doctrine', 'show', 'majordomus.enforcement-wiring']
      note: 'one rule: its class decides whether a violation stops the command'
      expect:
        exit: 0
        stdout_contains: ['^id +majordomus.enforcement-wiring', '^class +blocking']
  then:
    - 'exit 0 is clean, 10 is a failing finding, 12 is a missing precondition; nothing exits 0 with a FAIL line'
    - 'every FAIL and WARN names the command that reproduces it'
    - 'nothing here reached the network or evaluated generated text'
---

# Situation

A quality gate that prints warnings and exits zero is decoration. A gate whose findings cannot be reproduced locally is a conversation with a CI log. And a gate that is itself unverified may be checking nothing: the hook exists, but does it call the tool?

# What you run

- `doctor`: the tool’s own health, and every enforcement reconciled against what actually runs
- `watch`: what has drifted since the last update
- `doctrine list` and `doctrine show <id>`: what is enforced, by what, and whether it is wired
- `check`: the same contract inside a task

# Outcome

CI runs the same commands a person runs, gets the same exit codes, and every finding is a command away from being reproduced at a desk. The tool proves its own wiring before it judges anything else.
