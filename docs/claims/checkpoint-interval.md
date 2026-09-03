# A profile sets how often a worker should checkpoint, and a stale task is reported

## What it means

Each profile has a `checkpoint_interval`. `check --checkpoint` records a checkpoint; `check` and `watch` compare the last checkpoint with the interval and report a task that has gone quiet for longer than its profile allows.

## How it works

`checkpoint_at` on the task record is updated by `check --checkpoint`, by `handover`, and by `finish`. `lib/check.sh` computes the age against the profile interval and emits a warning; `lib/watch.sh` emits staleness drift. The staleness report is deterministic; the worker's checkpointing is not.

## How to see it

```bash
majordomus check --checkpoint
majordomus watch     # DRIFT staleness … when the interval has passed
```

## What it does not cover

**Advisory** on the worker's side: whether it checkpoints is up to it. The report of a stale task is guaranteed; a stale checkpoint is a warning in `check` and never blocks.

## Why it exists

Sessions that never end were a named failure; a checkpoint interval makes "this task has not been touched in an hour" a fact the tool can state.
