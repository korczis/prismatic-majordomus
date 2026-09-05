+++
title = "Hand unfinished work to the next session"
description = "Stop mid-task and leave the next worker something to act on that is not a transcript."
weight = 2
[extra]
id = "hand-work-between-sessions"
source = ".ai/repo/use-cases/hand-work-between-sessions.md"
category = "continuity"
maturity = "verified"
+++

## Situation

A session ends with the work half done. The usual handover is a paste of the conversation, which the next worker has to read in full to find the three facts that matter, and which is stale the moment the branch moves.

## What you run

- `checkpoint`: a short progress record inside the active task, refused if it grows into a report
- `handover`: an append-only record with computed front matter and the sections the policy requires
- `context`: the next session reads this instead of the transcript, within a line budget

## Outcome

The next worker starts from durable state with the git position it was written at, and is told how far the repository has moved since.
