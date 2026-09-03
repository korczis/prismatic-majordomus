# Dependencies between tasks are deliberately not represented

## What it means

**Rejected.** There is no field, file or command that says task B depends on task A. One task is active per checkout; coordination between checkouts is the overlap report on scopes. No dependency graph, no ordering, no DAG.

## How it works

Nothing is implemented, on purpose. Coordination is scope containment computed from git worktrees; sequencing is a human decision.

## How to see it

```bash
grep -rn depend .majordomus/state/current.yaml   # nothing to find
```

## What it does not cover

Teams that need sequencing between tasks do it outside the tool.

## Why it exists

No use case in the evidence justified a graph. The one task record with dependency-like fields found in the source environment was a demonstration file whose subtasks were still pending a year later. The omission is published here so a reader sees it was refused rather than forgotten.
