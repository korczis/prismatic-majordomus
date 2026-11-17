# A touched file outside the claimed scope is a failure, not a warning

## What it means

If a task's changes — uncommitted or committed since the task started — include a file outside the declared scope, `check` and `finish` fail on it by name. Files under `.ai/` and the generated instruction files are always allowed, because the tool itself writes them.

## How it works

`lib/check.sh` collects touched files, normalises each, and tests containment against every scope entry with `mj_path_contains`, which normalises both sides. Each outside file is a `FAIL scope` finding with a reproduce command. `finish` re-runs the same logic as the first line of its contract and refuses on failure.

## How to see it

```bash
majordomus start "t" --scope lib/auth
echo x > lib/other/b.txt
majordomus check; echo $?
# FAIL scope       lib/other/b.txt — outside claimed scope (lib/auth)
# 10
```

## What was already changed

A task answers for what changes after it starts. `start` records the paths that were
already dirty and what each held (`.ai/local/state/start-dirty.tsv`, this checkout's own),
and a path is not counted while it still holds exactly that: a tracked file the
repository's own hooks regenerate, somebody's work in progress, an untracked draft. `check`
says how many it left out. A path the task edits after that, creates, or commits is the
task's as before, and a record belongs to the one task that wrote it
(`test/cases/108c_scope_after_a_dirty_start.sh`).

## What it does not cover

Scope is about files, not about semantics: a change inside `lib/auth` that breaks something in `lib/other` is caught by your verification command, not by scope.

## Why it exists

Forty fully isolated worktrees in the source environment still produced about three thousand concurrently modified files because scope was declared optionally and checked nowhere that ran. A declaration only means something when a file outside it fails.
