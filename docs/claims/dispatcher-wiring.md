# Wiring is found in a hook or in any subhook of the directory it dispatches to

## What it means

A repository whose `pre-commit` is a dispatcher that runs everything in `.githooks/pre-commit.d/` still has real enforcement. `doctor` looks in the hook itself and in every file of `<hook>.d/`, and reports the file that actually carries the invocation — not the dispatcher that happened to be searched first.

## How it works

`mj_hook_candidates` in `lib/doctor.sh` expands a declared hook into the hook file plus the sorted contents of `<hook>.d/`. Each candidate is searched for the declared command. The finding names the file where it was found, so the report points at the subhook a person would edit. A subhook that contains the invocation but is not executable is reported as **not wired**, because a dispatcher iterating its directory will skip it — the check follows what the dispatcher does, not what the file says.

## How to see it

```bash
mkdir -p .githooks/pre-commit.d
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .githooks/pre-commit.d/70-majordomus.sh
majordomus doctor
# FAIL wiring  pre-commit — .githooks/pre-commit.d/70-majordomus.sh is not executable
chmod +x .githooks/pre-commit.d/70-majordomus.sh
majordomus doctor
# ok  wiring  pre-commit — invoked by .githooks/pre-commit.d/70-majordomus.sh
```

## What it does not cover

The search is textual: it proves the command appears in a file the dispatcher will execute, and that the file is executable. It does not simulate the dispatcher, so a subhook that is executable and contains the invocation but returns early on some condition is reported as wired. Swallowed exit codes are a separate check.

## Why it exists

Numbered `*.d/` dispatchers are the common shape of a repository that already takes its hooks seriously. Requiring such a repository to flatten its hook layout to satisfy the checker would be the tool bending the repository around itself.
