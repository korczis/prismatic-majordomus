---
id: accept-or-refuse-finished-work
kind: use-case
title: 'Decide whether finished work is actually finished'
summary: 'Evaluate a contract line by line instead of accepting a sentence that says the work is done.'
category: completion
status: active
target: guaranteed
weight: 5
actors: [reviewer, agent]
difficulty: intermediate
commands: [finish, question, check]
doctrines: [majordomus.verification-integrity, majordomus.blocker-resolution, majordomus.profile-requirements, majordomus.note-integrity]
claims: [finish-contract, typed-outcome, consistency-check]
responsibilities: [finish, scope]
applications: [ci-gated-project, long-running-work]
scenario:
  setup: finish-ready
  given:
    - 'an active task with its completion note written'
  steps:
    - id: open-a-question
      run: ['question', 'add', 'should tabs be an error or a warning?']
      note: 'an unresolved question naming the task'
      expect:
        exit: 0
        stdout_contains: ['^opened for t-']
    - id: refused-while-open
      run: ['finish', '--outcome', 'completed', '--verify-command', 'true']
      note: 'an open question is a blocker; completed is refused'
      expect:
        exit: 10
        stdout_contains: ['^FAIL blockers', 'refused']
    - id: resolve-it
      run: ['question', 'resolve', '1', '--answer', 'an error']
      note: 'the question is answered as state, not in prose'
      expect:
        exit: 0
        stdout_contains: ['resolved']
    - id: accepted
      run: ['finish', '--outcome', 'completed', '--verify-command', 'true']
      note: 'every line of the finish contract passes and the outcome is recorded'
      expect:
        exit: 0
        stdout_contains: ['completed']
  then:
    - 'finish writes nothing while any line of the contract fails'
    - 'the verification command, its exit code and duration are recorded'
---

# Situation

A worker reports success. The report is a paragraph, the evidence is the paragraph, and accepting it is a matter of trust rather than of checking.

# What you run

- `question`: anything unresolved is recorded as state, and refuses completion while it stands
- `finish`: --verify-command runs the project's own verification and records its exit code and duration

# Outcome

A refusal names the rules that refused. An outcome is typed — completed, partial, blocked, no_match, failed — so "the thing does not exist" and "I could not do it" stay different facts.
