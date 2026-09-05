---
id: trust-the-policy-before-reading-it
kind: use-case
title: 'Trust the policy and the profiles only after they are validated'
summary: 'Prove that the canonical policy parses with every key it needs declared, that every profile parses and the default exists, and that what the policy declares as enforced is what actually runs.'
category: policy
status: active
target: guaranteed
weight: 41
actors: [maintainer]
difficulty: basic
commands: [doctor, check]
doctrines: [majordomus.policy-integrity, majordomus.policy-completeness, majordomus.profile-requirements, majordomus.enforcement-wiring]
claims: [policy-parse, profile-validate, wiring-reconciliation, exit-code-contract]
responsibilities: [policy, profiles, doctor]
applications: [repository-with-authored-governance, ci-gated-project]
scenario:
  setup: installed-wired
  given:
    - 'installed, and the two enforcements the policy declares are actually in place as hooks'
  steps:
    - id: policy
      run: ['doctor']
      note: 'the policy parses, every value the code reads is declared with no reader-side default, every profile parses and the default profile exists, and every declared enforcement is reconciled against a hook that calls the tool'
      expect:
        exit: 0
        stdout_contains: ['^OK   policy      .ai/repo/policy.yaml — parsed', 'no reader-side default', '^OK   profiles', "default 'implementation' exists", '^OK   wiring      doctor-on-commit', '^OK   wiring      finish-on-push', 'doctor: 0 failure']
    - id: no-task
      run: ['check']
      note: 'outside a task the same contract answers with a precondition code, not a green run that checked nothing'
      expect:
        exit: 12
        stdout_contains: ['no active task']
  then:
    - 'an unknown key in the policy or a profile is a parse failure, not an ignored line'
    - 'a policy value the code reads without a declaration is a failure'
    - 'exit 12 means a precondition is missing; it is never confused with a clean run'
---

# Situation

The policy is the one file everything else is derived from. If it carries a key nobody reads, a typo becomes a silent default; if a profile is missing, the worker runs under nothing; if the policy says a hook enforces something and the hook does not exist, the enforcement is a sentence. None of that is visible by reading the files.

# What you run

- `doctor`: the policy and every profile parsed with unknown keys refused, every value the code reads declared, the default profile present, and each declared enforcement reconciled against what actually runs
- `check`: outside a task, exit 12 rather than a run that checked nothing

# Outcome

What the policy says is what the tool does, and both are proven before any worker reads them. The exit code is the answer: 0 clean, 10 a failing finding, 12 a missing precondition.
