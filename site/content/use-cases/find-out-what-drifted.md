+++
title = "Find out what has drifted since anyone last looked"
description = "Ask what has moved, rather than whether anything is wrong."
weight = 6
[extra]
id = "find-out-what-drifted"
source = ".ai/repo/use-cases/find-out-what-drifted.md"
category = "drift"
maturity = "guaranteed"
+++

## Situation

The policy changed but the projections did not. A generated file was hand-edited. A task record describes a commit the branch no longer contains. Each is invisible until something downstream behaves oddly.

## What you run

- `watch`: reports drift across policy, projections, state, scope, records and retention
- `update`: --dry-run shows what regeneration would change before it changes it

## Outcome

Every finding carries the command that reproduces it, and the same rules produce it as produce the failures — one vocabulary, one registry, a different question.
