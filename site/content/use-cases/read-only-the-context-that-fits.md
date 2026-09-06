+++
title = "Read only the context that fits the budget, and know what was dropped"
description = "Assemble the context a worker needs now in authority order, cut it to a line budget, and see every dropped section named with its reason."
weight = 11
[extra]
id = "read-only-the-context-that-fits"
source = ".ai/repo/use-cases/read-only-the-context-that-fits.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

A worker that reads everything reads nothing well. The state is spread over the task record, the layer’s scoped documents, the questions store, the decisions and the checkpoints, and a fresh session cannot know which of it matters now. Pasting all of it into a prompt is expensive and wrong in a different way each time.

## What you run

- `context`: assembled from durable state in a fixed authority order, shaped by the task’s profile
- `context --budget-lines <n>`: the same, within a budget; dropped sections are listed by name and reason
- `context --json`: the same as one document
- `check --explain`: the policy and profile that shaped it

## Outcome

The next worker reads a short, ordered, complete-enough context and knows exactly what was left out. Nothing is in it that the profile excludes, and nothing is invented.
