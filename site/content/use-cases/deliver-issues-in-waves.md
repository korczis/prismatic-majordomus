+++
title = "Deliver issues in waves the graph computes, and take the next one"
description = "Validate the plan, read the execution waves the dependency graph yields, see which issues are ready and which are blocked on what, and take the one issue a worker should do now."
weight = 21
[extra]
id = "deliver-issues-in-waves"
source = ".ai/repo/use-cases/deliver-issues-in-waves.md"
category = "workers"
maturity = "guaranteed"
+++

## Situation

A plan with twenty issues and a handful of dependencies has an order in it that nobody wrote down. Workers pick what looks interesting, two of them take issues that touch the same files, and the dependency somebody forgot surfaces as a rebase.

## What you run

- `plan validate`: the model, references, DAG and status consistency
- `plan waves`, `plan ready`, `plan blocked`: the order the graph implies, and what is waiting on what
- `plan next`: the one issue to take
- `start <task> --scope <paths>`: the task under the issue’s scope, with overlap reported at start

## Outcome

The order is computed, not remembered. Parallel issues that would collide are named before anyone starts them, and the issue a worker takes is the one the graph says is next.
