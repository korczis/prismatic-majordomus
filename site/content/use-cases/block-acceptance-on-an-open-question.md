+++
title = "Block acceptance on a question nobody has answered"
description = "Open a question against the task, watch finish refuse completion while it is unresolved, resolve it with the answer, and only then complete."
weight = 17
[extra]
id = "block-acceptance-on-an-open-question"
source = ".ai/repo/use-cases/block-acceptance-on-an-open-question.md"
category = "completion"
maturity = "guaranteed"
+++

## Situation

Somebody asked a question that decides how the work is done, nobody answered, and the work carried on. If the task can be accepted anyway, the question was never really a blocker, and the next person discovers the assumption when it is already merged.

## What you run

- `question list`: the unresolved entries for the active task, numbered
- `finish --outcome completed`: refused while an entry is unresolved, with the entry named
- `check`: the same gate as a failing line, so it is visible before anyone tries to finish
- `question resolve <n> --answer`: rewrites that one line as resolved, with the answer beside the question

## Outcome

An open question is a blocker the tool enforces, not a note somebody may read. The record of what was asked and what was decided stays in the repository, and finishing over it is impossible rather than discouraged.
