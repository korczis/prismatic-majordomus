+++
title = "Hand work over with a question still open, and keep it blocking"
description = "Close a task into a handover while a question is unresolved, start the follow-up task, and see the same question still refuse acceptance there."
weight = 12
[extra]
id = "carry-a-blocker-across-a-handover"
source = ".ai/repo/use-cases/carry-a-blocker-across-a-handover.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

A worker opened a question, ran out of session and handed the work over. The next worker starts a fresh task. If the question belonged to the old task, the new task would be acceptable with the question still unanswered, and the handover would have laundered a blocker into a footnote.

## What you run

- `handover --close`: the continuation record, and the task marked handed over
- `start <task> --scope <paths>`: the follow-up; the prior handover is named
- `check`: the unresolved question is a failing line on this branch
- `question list`: the entry, numbered, with the task that opened it
- `finish --outcome completed`: refused by the same entry

## Outcome

A blocking question keeps blocking after the work is handed to a new task. The record of who asked and under which task survives, and only an answer clears it.
