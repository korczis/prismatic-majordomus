+++
title = "every refused finish appends a task.refused event naming the outcome claimed, the unmet count and the doctrines that refused, and finish --check writes none"
description = "Every time majordomus finish refuses, the ledger gains a task.refused line: the outcome that was claimed, how many contract lines were unmet, which doctrines refused, and every doctrine's verdict. A task accepted after four refusals leaves four task.refused lines and then one task.finished, in that order. finish --check asks whether the contract would pass and records nothing, because a check is a question and a finish is a claim."
weight = 57
[extra]
claim_id = "finish-refusal-is-recorded"
status = "guaranteed"
source = "docs/claims/finish-refusal-is-recorded.md"
+++
{% raw %}

## What it means

Every time `majordomus finish` refuses, the ledger gains a `task.refused` line: the outcome that was claimed, how many contract lines were unmet, which doctrines refused, and every doctrine's verdict. A task accepted after four refusals leaves four `task.refused` lines and then one `task.finished`, in that order. `finish --check` asks whether the contract would pass and records nothing, because a check is a question and a finish is a claim.

## How it works

`lib/finish.sh` evaluates the contract as before. When any line is unmet it appends `task.refused` through `mj_ledger_append`, which refuses an event the registry does not declare or one missing a required field (`task_id`, `outcome`, `unmet`, `refused`), and then exits 10. The event is declared in `share/events.yaml`, documented in `docs/SCHEMAS.md` and rendered by `majordomus history`. `--check` returns before the append.

## How to see it

```bash
majordomus finish --outcome completed            # refused: verification not run
majordomus finish --outcome completed --verify-command "make test"   # accepted
majordomus history --event task.refused --all
# 2026-09-16T…  task.refused        t-…   9b1e2d4  outcome=completed unmet=2
```

## What it does not cover

It records refusals of `finish`, not refusals by git hooks or CI gates, and it does not count anything on its own: the count is what `history --event task.refused` lists. A refusal is local state under `.ai/local/`, which is never committed or published, so the record belongs to the checkout that made it.

## Why it exists

A finish contract makes *done* falsifiable, and until this event existed the falsifications left no trace: a task refused four times before it passed looked the same in the ledger as one that passed first try. That made the tool's central benefit impossible to show — every false *done* it caught vanished with the terminal output. The repository had recorded the gap itself, as `finish-refusal-leaves-no-trace` at priority 1 in `docs/HARDCODING_LEDGER.yaml`. A refusal later satisfied is a caught false *done*, and now it can be counted.
{% endraw %}
