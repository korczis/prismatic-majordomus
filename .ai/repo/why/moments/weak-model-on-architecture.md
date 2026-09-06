---
schema: moment/v1
id: weak-model-on-architecture
kind: moment
title: 'A cheap worker deciding something expensive'
short_title: 'Under-powered decision'
hook: 'let a fast, cheap session make a decision the whole codebase now lives with'
summary: 'Capability is dialled down for cost and nothing distinguishes the tasks where that is prudent from the ones where it is expensive.'
status: stable
severity: high
frequency: occasional
weight: 280
audiences: [research-team, engineering-lead, solo-builder, enterprise]
areas: [cost]
lifecycle: [planning, implementation]
tags: [cost, model-selection, effort, profiles]
signals:
  - id: cheap-by-default
    text: 'Everything runs on the cheap setting, including work whose consequences outlive the session.'
  - id: no-escalation-path
    text: 'There is no defined point at which a blocked worker should escalate rather than continue.'
  - id: structural-choice-by-accident
    text: 'A structural choice was made by whichever session happened to be open.'
examples:
  - id: schema-by-a-cheap-session
    audience: solo-builder
    title: 'A schema chosen in passing'
    before: 'A quick session picks a data shape that the next six months are built on; the choice was never framed as a decision.'
    after: 'A task names a profile; `deep-work` requires a decision record before it can be finished, so the choice is at least stated.'
  - id: cost-optimised-everything
    audience: enterprise
    title: 'Optimising the wrong axis'
    before: 'Cost pressure moves all work to the cheap setting, and the saving is spent several times over on one bad interface.'
    after: 'Capability class and effort are per-profile fields, so cheap is a default for routine work rather than a policy for all work.'
  - id: blocked-and-grinding
    audience: research-team
    title: 'Grinding instead of escalating'
    before: 'An under-powered worker retries a problem it cannot solve, twenty times, at low cost each time.'
    after: 'Escalation is a recorded event after a stated number of blocked attempts, not a mood.'
commands: [start, check, decision]
capabilities: [objects.list]
claims: [profile-axes, capability-class, effort-escalation, decision-record]
doctrines: [majordomus.justified-escalation, majordomus.profile-requirements, majordomus.decision-threshold, majordomus.depth-is-not-verbosity]
use_cases: [trust-the-policy-before-reading-it, record-a-decision-before-it-is-forgotten]
related: [strongest-model-renames-a-variable, no-record-why-this-model, spend-not-tied-to-outcomes]
aliases: ['under-powered model', 'cheap model wrong task', 'no escalation']
---

## The moment

A session set to the fast, cheap configuration is asked a question that turns out to be
structural: how the records relate, where the boundary goes. It answers reasonably. The
answer is now in the schema, and it will be there in two years.

## Why it happens

The dial is set per session, not per task, and the tasks arrive mixed. Nothing marks the
moment at which a question stopped being routine, so the configuration that was correct for
the last three changes is applied to the one that matters. Cost pressure makes this the
default direction of the error.

## Why a better model does not fix it

Trivially, a better model here *would* fix this instance — which is exactly the point: the
failure is that nothing chose. The next task will be mis-dialled in whichever direction the
previous one left it, and half the time that direction is expensive.

## What it costs

Asymmetrically. A rename done at maximum effort wastes a few cents. A boundary chosen by an
under-powered session is paid for by every change that crosses it afterwards.

## What Majordomus does

A profile is a small file that sets capability class, reasoning effort, verbosity,
presentation, the context to load, the verification required and the checkpoint interval,
independently. A task names one at `start`, and the generated instructions state which is in
force. `deep-work` makes a decision record a condition of finishing, so a structural choice
made under it is written down with its reason and its rejected alternative. Where a profile
allows escalation, it is a recorded event after a stated number of blocked attempts.

## Before and after

```text
before   (cheap, because the last task was cheap)  -> the data model

after    $ majordomus start "choose the record boundary" --profile deep-work
         $ majordomus finish --outcome completed --verify-command "make test"
         FAIL profile  deep-work requires a decision record for this task
```

## What it does not do

Majordomus never selects or invokes a model, and cannot observe what a worker actually ran.
Profiles are projected into the instructions and validated as configuration; every
profile-related claim is marked advisory for that reason.
