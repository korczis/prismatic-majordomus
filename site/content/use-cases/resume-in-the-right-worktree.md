+++
title = "Resume in the right worktree, never against somebody else’s task"
description = "Find the handover that belongs to this worktree and branch, and see a task record from another checkout reported as foreign rather than enforced here."
weight = 13
[extra]
id = "resume-in-the-right-worktree"
source = ".ai/repo/use-cases/resume-in-the-right-worktree.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

Two checkouts of one repository, two workers. A task record that travels with a branch into a worktree that never claimed it would hold the wrong person to the wrong scope, and a handover from another branch offered as the one to resume from would send the next worker down the wrong path.

## What you run

- `handover --resolve`: the most relevant prior handover for this worktree and branch, or a clear absence
- `check`: refuses to evaluate a task this checkout does not own
- `context`: this worktree’s own identity, with no task borrowed from elsewhere

## Outcome

Records are resolved by worktree and branch. What another checkout is doing is visible through overlap, not enforced here, and nothing under .ai/local/ is shared through git.
