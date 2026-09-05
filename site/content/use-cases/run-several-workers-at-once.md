+++
title = "Run several workers in one repository without them colliding"
description = "Give each worker a declared scope, and find out immediately when two of them claim the same paths."
weight = 4
[extra]
id = "run-several-workers-at-once"
source = ".ai/repo/use-cases/run-several-workers-at-once.md"
category = "workers"
maturity = "executable"
+++

## Situation

Two agents work the same repository at the same time. Each is individually reasonable; together they edit the same file from different assumptions, and the conflict surfaces as a merge, long after the decision that caused it.

## What you run

- `start`: takes the paths this task may touch, and refuses a second active task in one checkout
- `check`: --overlap reports other worktrees whose claims intersect yours
- `finish`: refuses to accept work outside the claimed paths

## Outcome

Overlap is reported when the task starts rather than when the branches merge, and a task record belonging to another checkout is reported as foreign rather than enforced against yours.
