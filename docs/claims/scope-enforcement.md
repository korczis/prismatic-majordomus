# A touched file outside the claimed scope is a failure, not a warning

## What it means

If a task's changes — uncommitted or committed since the task started — include a file outside the declared scope, `check` and `finish` fail on it by name. Files under `.majordomus/` and the generated instruction files are always allowed, because the tool itself writes them.

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

## What it does not cover

Scope is about files, not about semantics: a change inside `lib/auth` that breaks something in `lib/other` is caught by your verification command, not by scope.

## Why it exists

Forty fully isolated worktrees in the source environment still produced about three thousand concurrently modified files because scope was declared optionally and checked nowhere that ran. A declaration only means something when a file outside it fails.
