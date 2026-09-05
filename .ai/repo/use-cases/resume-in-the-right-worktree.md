---
id: resume-in-the-right-worktree
kind: use-case
title: 'Resume in the right worktree, never against somebody else’s task'
summary: 'Find the handover that belongs to this worktree and branch, and see a task record from another checkout reported as foreign rather than enforced here.'
category: continuity
status: active
target: guaranteed
weight: 32
actors: [agent]
difficulty: intermediate
commands: [handover, check, context]
doctrines: [majordomus.handover-integrity, majordomus.state-consistency, majordomus.scope-integrity]
claims: [record-resolution, worktree-ownership, divergence-label, local-state-ignored]
responsibilities: [handover, state, scope]
applications: [several-agents-one-repository]
scenario:
  setup: two-worktrees
  given:
    - 'installed and wired, with a second worktree on another branch holding an active task scoped to lib'
  steps:
    - id: nothing-here
      run: ['handover', '--resolve']
      note: 'no handover was written for this worktree and branch; absence is reported, never a record from elsewhere'
      expect:
        exit: 0
        stdout_contains: ['No relevant handover']
    - id: not-my-task
      run: ['check']
      note: 'the other worktree’s task is not this checkout’s; check reports no active task here instead of holding this checkout to a scope it never claimed'
      expect:
        exit: 12
        stdout_contains: ['no active task']
    - id: read-only-view
      run: ['context']
      note: 'what this worktree knows: its own git identity and no task'
      expect:
        exit: 0
        stdout_contains: ['^## GIT', 'none active']
  then:
    - 'handover --resolve never offered a record from another branch'
    - 'check did not enforce the other worktree’s scope here'
    - 'local state under .ai/local/ is this checkout’s own and untracked'
---

# Situation

Two checkouts of one repository, two workers. A task record that travels with a branch into a worktree that never claimed it would hold the wrong person to the wrong scope, and a handover from another branch offered as the one to resume from would send the next worker down the wrong path.

# What you run

- `handover --resolve`: the most relevant prior handover for this worktree and branch, or a clear absence
- `check`: refuses to evaluate a task this checkout does not own
- `context`: this worktree’s own identity, with no task borrowed from elsewhere

# Outcome

Records are resolved by worktree and branch. What another checkout is doing is visible through overlap, not enforced here, and nothing under .ai/local/ is shared through git.
