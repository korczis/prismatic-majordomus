+++
title = "Read back what happened, and keep the ledger within its cap"
description = "Read the append-only ledger as operational history, filtered by task and event, validate every line, and rotate the oldest lines into an archive rather than deleting them."
weight = 20
[extra]
id = "read-back-what-happened"
source = ".ai/repo/use-cases/read-back-what-happened.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

Something happened to the task last week: a checkpoint, a decision, a handover. The person asking was not there. The ledger has every event, but reading a JSON file end to end is not an answer, and a ledger with a corrupt line in it is worse than none if the tool skips it quietly.

## What you run

- `history`: the ledger as operational history, newest lines by default, `--task`, `--event`, `--since` to narrow
- `history --validate`: every line well-formed, exit 10 otherwise
- `search <text>`: durable records matched literally across kinds
- `watch`: the retention caps on the ledger, the handovers and the checkpoints

## Outcome

What happened is readable by task, event and time, a malformed line is a failure rather than a skipped record, and the ledger stays within its cap by rotating the oldest lines into an archive.
