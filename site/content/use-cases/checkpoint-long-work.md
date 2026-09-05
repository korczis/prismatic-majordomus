+++
title = "Checkpoint long work so a stop costs minutes, not the day"
description = "Record compact progress inside the active task at the profile interval, so a session that ends without warning leaves the next one a place to start."
weight = 10
[extra]
id = "checkpoint-long-work"
source = ".ai/repo/use-cases/checkpoint-long-work.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

A task runs for hours. The session that holds it can be cut off by a context limit, a crash or a person closing the laptop, and everything the worker learned since the last durable record goes with it. A transcript is not a record; it is not resumable and it is not the tool's to keep.

## What you run

- `checkpoint`: a short body on stdin becomes a capped record under the local half, with identity computed from git
- `check`: reports whether the checkpoint is fresh against the profile's interval, beside scope and blockers
- `context`: the next worker reads the newest checkpoint inside the assembled context, not a pasted log

## Outcome

Progress exists as a file the tool can find, capped so that it stays a summary, and the profile decides how stale is too stale. A session that stops mid-task loses at most one interval.
