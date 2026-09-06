+++
title = "The ledger is readable back as operational history, filtered by task, event and time"
description = "Majordomus has always appended events to state/ledger.jsonl. majordomus history is the command that reads them back: what happened, when, for which task, at which commit, what verification ran, and what outcome was accepted."
weight = 59
[extra]
claim_id = "history-ledger-read"
status = "guaranteed"
source = "docs/claims/history-ledger-read.md"
+++
{% raw %}

## What it means

Majordomus has always appended events to `state/ledger.jsonl`. `majordomus history` is the command that reads them back: what happened, when, for which task, at which commit, what verification ran, and what outcome was accepted.

It is operational reconstruction, not a conversation. Nothing in the ledger records what anyone said, because nothing writes that.

## How it works

Filters are `--task`, `--event`, `--since` (a duration like `90m`, `2h`, `7d`, or an ISO timestamp), and `--limit` (default 20, newest) or `--all`. Output is oldest line first, so a filtered run reads as a narrative rather than in reverse.

Each event renders a detail column suited to its kind: a started task shows its profile and scope, a finished task its outcome and the verify command's exit code, a checkpoint or handover its record's filename, a resolved question its answer.

`--json` emits the matching ledger lines verbatim — the same bytes, not a re-serialisation, so a consumer sees exactly what was written.

The reader skips a malformed line rather than crashing on it, so a corrupted ledger stays readable while it is being reported.

## How to see it

```bash
majordomus history --task t-20260903193012-a4f1
# 2026-09-03T19:30:12Z  task.started       t-20260903193012-a4f1  3f2a9c1  profile=debugging scope=lib/auth
# 2026-09-03T19:45:00Z  task.checkpoint    t-20260903193012-a4f1  3f2a9c1  20260903T194500Z--main--3f2a9c1--8c1d0e4a.md
# 2026-09-03T19:52:31Z  decision.recorded  t-20260903193012-a4f1  3f2a9c1  Normalise the callback URI before comparing state
# 2026-09-03T20:14:08Z  task.finished      t-20260903193012-a4f1  b71e0c9  outcome=completed verify_exit=0

majordomus history --event task.finished --all --json | jq -r '.outcome'
```

## What it does not cover

It reports what was recorded, not what happened. A worker that never checkpoints leaves a sparse history, and the ledger cannot know that.

It does not aggregate. There is no report of tasks per week or average duration, because nothing yet needs one and a statistic in a tool that cannot measure cost invites the reader to infer cost.

It is one repository's ledger. Nothing merges histories across repositories or worktrees.

## Why it exists

The events were being written from the first version and read by nothing but a `grep` inside `watch`. A write-only log is a cost with no benefit: it accumulates, it is capped, and it answers no question anyone can ask of it. Either something reads it or it should not be written.
{% endraw %}
