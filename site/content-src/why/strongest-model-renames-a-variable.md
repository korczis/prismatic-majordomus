+++
title = "The strongest model, at maximum effort, renaming a variable"
description = "Why effort and capability drift to maximum by default, and how named profiles make them a decision instead of a habit."
weight = 3
[extra]
hook = "watched the strongest model, at maximum effort, rename a variable"
responsibilities = ["profiles"]
commands = ["start"]
claims = ["profile-axes", "capability-class", "effort-escalation", "minimum-context"]
+++
## The moment

A one-line rename. The session is on the most capable model available, reasoning effort at
its ceiling, context loaded with half the repository, and it writes three paragraphs
explaining the rename.

## Why it happens

Nothing chose that configuration; it was left over from the last hard task. Capability,
reasoning depth, context size and output verbosity are four separate decisions, and when
nothing names them they collapse into one habit — usually "everything on". The routing
documents this tool was distilled from fused effort into the model name and routed by the
worker's job title, with performance figures nobody had measured.

## What Majordomus does

A profile is a small file that sets each axis independently: capability class, reasoning
effort, output verbosity, presentation, which context to load, what verification is
required, and how often to checkpoint. Four ship with the tool — `routine`,
`implementation`, `debugging`, `deep-work` — and a task names one at `start`. The generated
instruction file tells the worker which profile is in force, so the rename runs under
`routine`: fast capability class, low effort, terse output, task and current state only.
Escalation, when a profile allows it, is a recorded event after a stated number of blocked
attempts, not a mood.

## What it does not do

This is the honest part. Profiles are projected into the worker's instructions and validated
as configuration. Whether the worker honours them is not observable from outside the worker,
and Majordomus never selects or invokes a model. Every profile-related row in the claims
matrix is marked advisory for exactly that reason. Measuring what a session actually
consumed is on the roadmap and is not claimed today.

## Try it

```bash
majordomus start "rename SessionStore to SessionRepo" --scope lib/session --profile routine
majordomus check --explain     # the effective axes for this task
```
