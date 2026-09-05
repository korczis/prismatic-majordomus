+++
title = "check reports whether state, scope, blockers and checkpoint age are consistent right now"
description = "majordomus check is the read-only question a worker asks before claiming anything is done: does the task record still describe this checkout, are all touched files inside the declared scope, is any open question for this task unresolved, and how long since the last checkpoint. It prints one line per aspect, exits 0 when nothing fails, 10 when something does, 12 when there is no active task."
weight = 41
[extra]
claim_id = "consistency-check"
status = "guaranteed"
source = "docs/claims/consistency-check.md"
+++
{% raw %}

## What it means

`majordomus check` is the read-only question a worker asks before claiming anything is done: does the task record still describe this checkout, are all touched files inside the declared scope, is any open question for this task unresolved, and how long since the last checkpoint. It prints one line per aspect, exits 0 when nothing fails, 10 when something does, 12 when there is no active task.

## How it works

`lib/check.sh` loads the task record, computes the divergence label of its recorded head against `HEAD`, lists touched files from `git status` plus `git diff --name-only <recorded head> HEAD`, tests each against the scope, reads `state/open-questions.md` for `[unresolved] <task id>`, and compares `checkpoint_at` with the profile's interval. `--explain` prints the effective task, profile and policy; `--checkpoint` updates the timestamp and is the command's one documented write.

## How to see it

```bash
majordomus check
# OK   state       t-… — advanced (head 3f2a9c1)
# FAIL scope       config/x.yml — outside claimed scope (lib/auth)  [reproduce: git status --porcelain; git diff --name-only 3f2a9c1 HEAD]
# OK   checkpoint  t-… — 7m ago, interval 15m
# OK   blockers    t-… — none open
```

## What it does not cover

A stale checkpoint is a warning and never fails the check; work in progress is reported, not blocked. `check` does not run tests; `finish` does.

## Why it exists

The source environment had no command that could answer "is this task in a sane state right now" without reading a transcript. Every recovery was a manual runbook.
{% endraw %}
