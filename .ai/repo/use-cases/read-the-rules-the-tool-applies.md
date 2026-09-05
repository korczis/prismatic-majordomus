---
id: read-the-rules-the-tool-applies
kind: use-case
title: 'See which rules apply here and what enforces them'
summary: 'List the effective rule set, read one rule as the tool reads it, and see which of them the tool enforces and from where.'
category: policy
status: active
target: guaranteed
actors: [maintainer, reviewer, agent]
difficulty: basic
commands: [rules, doctrine]
doctrines: [majordomus.rule-package-integrity, majordomus.doctrine-wiring-integrity]
claims: [rule-resolution, vendored-rule-package, doctrine-registry]
responsibilities: [doctor, policy]
applications: [repository-with-authored-governance, ci-gated-project]
scenario:
  setup: installed
  given:
    - "the vendored baseline plus this repository's own rules"
  steps:
    - id: list
      run: ['rules', 'list']
      note: 'every active rule, its version, class and origin, in one deterministic order'
      expect:
        exit: 0
        stdout_contains: ['^majordomus\.scope-integrity +v1 +blocking +vendor:majordomus']
    - id: read-one
      run: ['rules', 'show', 'majordomus.scope-integrity']
      note: 'the rule as the tool reads it: identity, class, dependencies, enforcement'
      expect:
        exit: 0
        stdout_contains: ['^id: majordomus\.scope-integrity$', '^class: blocking$']
    - id: refuse-unknown
      run: ['rules', 'show', 'nosuch.rule']
      note: 'a rule that does not exist is named, not guessed'
      expect:
        exit: 12
        stdout_contains: ["no rule 'nosuch.rule'"]
    - id: what-is-enforced
      run: ['doctrine', 'list']
      note: 'the rules with an x-majordomus block, and the commands that run them'
      expect:
        exit: 0
        stdout_contains: ['majordomus.scope-integrity', 'blocking']
  then:
    - "the effective set is the baseline plus the repository's rules, with no override"
    - 'a rule without an enforcing block is normative for whoever reads it and says so'
---

# Situation

A reviewer asks which rules hold in this repository and whether anything actually checks them. The answer is scattered across a wiki, a CI file and somebody's memory.

# Outcome

`rules list` is the effective set, `rules show` is one rule as the tool reads it, and `doctrine list` is the subset the tool enforces with the command that runs each one.
