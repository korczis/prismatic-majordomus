+++
title = "A cheap worker deciding something expensive"
description = "Capability is dialled down for cost and nothing distinguishes the tasks where that is prudent from the ones where it is expensive."
weight = 280
[extra]
id = "weak-model-on-architecture"
status = "stable"
source = ".ai/repo/why/moments/weak-model-on-architecture.md"
+++
{% raw %}

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
{% endraw %}
