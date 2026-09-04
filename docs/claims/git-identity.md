# Identity fields on every state record are computed from git and never authored

## What it means

`repository_id`, `branch`, `head`, `working_tree` and `changed_files` on the task record and on every handover are computed from `git rev-parse` and `git status` at the moment of writing. The worker cannot set them, and a handover body that contains a line looking like one of these fields is rejected. A model will confidently invent a branch name; git will not.

## How it works

`lib/start.sh` and `lib/handover.sh` call the git helpers in `lib/common.sh` and write the results into the record. `handover` greps the incoming body for identity keys before writing and refuses with exit code 10 if any is present. Divergence labels are then computed at read time from these fields, which is only meaningful because they were never authored.

## How to see it

```bash
majordomus start "t" --scope lib
grep -E '^(branch|head|working_tree):' .ai/local/state/current.yaml   # values from git
printf '# Objective\nx\nhead: abc\n# Current State\ny\n# Next Action\nz\n' | majordomus handover
# majordomus: handover: body must not contain identity fields; they are computed
```

## What it does not cover

`owner` and the task description are authored; they are labels, not identity.

## Why it exists

The most valuable rule found in the source environment's handover protocol: the writer captured repository, worktree, branch, head and changed files deterministically and the worker's instructions forbade inventing them. Everything else about trusting state follows from it.
