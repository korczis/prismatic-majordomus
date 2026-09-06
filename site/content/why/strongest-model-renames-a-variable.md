+++
title = "The strongest model, at maximum effort, renaming a variable"
description = "Capability, reasoning depth, context size and verbosity collapse into one habit — everything on — because nothing ever named them separately."
weight = 30
[extra]
id = "strongest-model-renames-a-variable"
status = "stable"
source = ".ai/repo/why/moments/strongest-model-renames-a-variable.md"
+++
{% raw %}

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

## Why a better model does not fix it

This is the one moment where a better model makes the problem worse: the stronger the
default, the more expensive the habit. The failure is that four independent dials were
never named, so no one can turn one of them down without turning all of them down.

## What it costs

Money, in a way that shows up on an invoice and nowhere else — no artefact records which
task the spend belonged to. Latency, on exactly the changes that should be instant. And
depth mistaken for verbosity: a worker at maximum effort writes more, which reads like
diligence and is mostly output.

## What Majordomus does

A profile is a small file that sets each axis independently: capability class, reasoning
effort, output verbosity, presentation, which context to load, what verification is
required, and how often to checkpoint. Four ship with the tool — `routine`,
`implementation`, `debugging`, `deep-work` — and a task names one at `start`. The generated
instruction file tells the worker which profile is in force, so the rename runs under
`routine`. Escalation, when a profile allows it, is a recorded event after a stated number
of blocked attempts, not a mood.

## Before and after

```text
before   (whatever the last task left set)          rename SessionStore -> SessionRepo

after    majordomus start "rename SessionStore" --scope lib/session --profile routine
         majordomus check --explain
           profile routine  capability fast  effort low  verbosity terse
           context task, current_state
```

## How to verify it

`majordomus check --explain` prints the effective axes for the active task. Start the same
task under two profiles and the printed axes differ; the instruction file the worker reads
states which profile is in force.

## What it does not do

This is the honest part. Profiles are projected into the worker's instructions and validated
as configuration. Whether the worker honours them is not observable from outside the worker,
and Majordomus never selects or invokes a model. Every profile-related row in the claims
matrix is marked advisory for exactly that reason. Measuring what a session actually
consumed is on the roadmap and is not claimed today.
{% endraw %}
