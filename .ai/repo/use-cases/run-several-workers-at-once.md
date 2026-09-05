---
id: run-several-workers-at-once
kind: use-case
title: 'Run several workers in one repository without them colliding'
summary: 'Give each worker a declared scope, and find out immediately when two of them claim the same paths.'
category: workers
status: active
target: guaranteed
weight: 4
actors: [agent, operator]
difficulty: intermediate
commands: [start, check, finish]
doctrines: [majordomus.scope-integrity, majordomus.state-consistency]
claims: [scope-enforcement, scoped-task, overlap-report, git-identity]
responsibilities: [scope, state]
applications: [several-agents-one-repository]
scenario:
  setup: two-worktrees
  given:
    - 'a second worktree of the same repository with an active task scoped to lib'
  steps:
    - id: claim-a-scope
      run: ['start', 'narrow the parser', '--scope', 'lib']
      note: 'the task starts, and the overlap with the other worktree is reported'
      expect:
        exit: 0
        stdout_contains: ['^started t-', 'overlap']
    - id: see-the-overlap
      run: ['check', '--overlap']
      note: 'scope containment against every other worktree, never blocking'
      expect:
        exit: 0
        stdout_contains: ['overlap']
    - id: nothing-to-refuse
      run: ['finish', '--check']
      note: 'no file outside the claimed scope has been touched'
      expect:
        exit: 0
        stdout_contains: ['0 failing']
  then:
    - 'two workers on one repository see each other by scope, and the second is told before it starts'
---

# Situation

Two agents work the same repository at the same time. Each is individually reasonable; together they edit the same file from different assumptions, and the conflict surfaces as a merge, long after the decision that caused it.

# What you run

- `start`: takes the paths this task may touch, and refuses a second active task in one checkout
- `check`: --overlap reports other worktrees whose claims intersect yours
- `finish`: refuses to accept work outside the claimed paths

# Outcome

Overlap is reported when the task starts rather than when the branches merge, and a task record belonging to another checkout is reported as foreign rather than enforced against yours.
