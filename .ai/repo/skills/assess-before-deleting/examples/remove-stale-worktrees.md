# Remove worktrees that look finished

The disk is filling and several linked worktrees belong to branches that were merged. Each
one is a candidate, not a decision: a merged branch can still hold uncommitted work.

```text
Apply the assess-before-deleting skill to every linked worktree whose branch is merged into
master. Classify each with git status, the unpushed commits and the peer board; push or
rescue anything that holds work, remove only the low-risk ones, then run majordomus doctor.
```
