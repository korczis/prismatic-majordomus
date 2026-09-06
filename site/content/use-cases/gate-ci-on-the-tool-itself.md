+++
title = "Gate CI on the tool itself, with an exit code that is a contract"
description = "Run the checks CI runs, read every finding with the command that reproduces it, and rely on exit codes that mean one thing each: no code means warn and continue."
weight = 16
[extra]
id = "gate-ci-on-the-tool-itself"
source = ".ai/repo/use-cases/gate-ci-on-the-tool-itself.md"
category = "policy"
maturity = "guaranteed"
+++

## Situation

A quality gate that prints warnings and exits zero is decoration. A gate whose findings cannot be reproduced locally is a conversation with a CI log. And a gate that is itself unverified may be checking nothing: the hook exists, but does it call the tool?

## What you run

- `doctor`: the tool’s own health, and every enforcement reconciled against what actually runs
- `watch`: what has drifted since the last update
- `doctrine list` and `doctrine show <id>`: what is enforced, by what, and whether it is wired
- `check`: the same contract inside a task

## Outcome

CI runs the same commands a person runs, gets the same exit codes, and every finding is a command away from being reproduced at a desk. The tool proves its own wiring before it judges anything else.
