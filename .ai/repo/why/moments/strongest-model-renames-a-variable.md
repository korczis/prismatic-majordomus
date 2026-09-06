---
schema: moment/v1
id: strongest-model-renames-a-variable
kind: moment
title: 'The strongest model, at maximum effort, renaming a variable'
short_title: 'Maximum effort by default'
hook: 'watched the strongest model, at maximum effort, rename a variable'
summary: 'Capability, reasoning depth, context size and verbosity collapse into one habit — everything on — because nothing ever named them separately.'
status: stable
severity: medium
frequency: constant
weight: 30
featured: true
audiences: [solo-builder, research-team, engineering-lead, enterprise]
areas: [cost]
lifecycle: [implementation]
tags: [cost, effort, model-selection, profiles]
signals:
  - id: never-chose-the-model
    text: 'Nobody chose the model or the effort for the last task; it was whatever the previous task left set.'
  - id: three-paragraphs-for-a-rename
    text: 'A trivial change came back with several paragraphs of explanation nobody needed.'
  - id: no-cheap-mode
    text: 'There is no agreed cheap mode for routine work, so everything runs at the expensive one.'
examples:
  - id: leftover-configuration
    audience: solo-builder
    title: 'Left over from the last hard task'
    before: 'Yesterday''s refactor needed the ceiling; today''s rename inherits it, and so does everything after it.'
    after: 'The rename runs under the `routine` profile: fast capability class, low effort, terse output, task and current state only.'
  - id: probe-at-full-price
    audience: research-team
    title: 'Fifty probes at the ceiling'
    before: 'A sweep of fifty small experiments runs at maximum capability and effort because that is what the window was set to.'
    after: 'The sweep names a profile; the axes are set once, deliberately, and the expensive setting is the exception that had a reason.'
  - id: budget-without-a-dial
    audience: enterprise
    title: 'A budget with no dial to turn'
    before: 'Spend rises and the only lever anyone can describe is "use it less".'
    after: 'Capability class, effort, verbosity and context are four named fields in a profile a task is started under, and the profile in force is stated to the worker.'
commands: [start, check]
capabilities: [objects.list]
responsibilities: [profiles]
claims: [profile-axes, capability-class, effort-escalation, minimum-context]
doctrines: [majordomus.justified-escalation, majordomus.depth-is-not-verbosity, majordomus.minimum-sufficient-context, majordomus.profile-requirements]
use_cases: [read-only-the-context-that-fits, trust-the-policy-before-reading-it]
related: [weak-model-on-architecture, no-record-why-this-model, spend-not-tied-to-outcomes]
aliases: ['over-powered model', 'effort escalation', 'wrong model for the task']
---

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
