+++
title = "Keep every provider bootstrap thin, generated and within budget"
description = "Regenerate AGENTS.md, CLAUDE.md and the other provider files from one policy and prove they are stamped, point at the layer, stay under the line budget and resolve every reference."
weight = 15
[extra]
id = "keep-the-bootstrap-thin-and-within-budget"
source = ".ai/repo/use-cases/keep-the-bootstrap-thin-and-within-budget.md"
category = "drift"
maturity = "guaranteed"
+++

## Situation

Every AI client loads one instruction file before it does anything. If that file grows, restates rules, or drifts from the policy, every session starts from something stale, and the cost is paid on every turn. If the tool overwrote it silently, somebody’s hand edit would vanish.

## What you run

- `update`: renders every provider bootstrap the policy declares, from the policy, with a stamp
- `doctor`: the projection matches its stamp, is a bootstrap and not a rulebook, is within the line budget, references resolve, no counts are hardcoded
- `watch`: drift since the last generation, if any

## Outcome

The bootstrap is a thin pointer at the layer, generated and checked, and the layer is where the rules live. A hand edit is detected rather than overwritten.
