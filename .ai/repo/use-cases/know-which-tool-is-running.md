---
id: know-which-tool-is-running
kind: use-case
title: 'Check the version before trusting a diagnosis'
summary: 'Print the version every other surface derives from, and see the argument it refuses.'
category: adoption
status: active
target: advisory
actors: [operator, contributor]
difficulty: basic
commands: [version]
doctrines: []
claims: [exit-code-contract]
responsibilities: []
applications: [ci-gated-project]
scenario:
  setup: bare
  given:
    - 'an empty repository; nothing installed'
  steps:
    - id: print
      run: ['version']
      note: 'the one string the projections, the ledger and the site derive their version from'
      expect:
        exit: 0
        stdout_contains: ['^majordomus [0-9]+\.[0-9]+\.[0-9]+$']
    - id: refuse
      run: ['version', '--no-such-option']
      note: 'an argument the command does not know is a usage error, never ignored'
      expect:
        exit: 2
        stdout_contains: ['unknown option --no-such-option']
  then:
    - 'the version needs no repository and no installation'
    - 'a usage error exits 2, as the exit-code contract says'
---

# Situation

A diagnosis from one machine is compared with a repository on another, and nobody knows whether the two ran the same tool.

# Outcome

`version` prints the version and nothing else, from anywhere, and refuses an argument it does not understand rather than pretending it did.
