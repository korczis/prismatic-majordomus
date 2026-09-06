+++
title = "Nobody can say why that worker was chosen"
description = "Provider and model selection is made implicitly, per session, and recorded nowhere, so it cannot be reviewed or repeated."
weight = 290
[extra]
id = "no-record-why-this-model"
status = "stable"
source = ".ai/repo/why/moments/no-record-why-this-model.md"
+++
{% raw %}

## The moment

Two providers have been in use for a quarter. Somebody asks which one to standardise on.
The available evidence is that one of them "feels better for refactors", and nobody can
point at a single piece of work and say which produced it.

## Why it happens

The choice is made at the moment a window is opened, which is not a moment anybody thinks of
as a decision. It leaves no artefact: the code looks the same whoever wrote it, and the
commit records a human author.

## Why a better model does not fix it

This failure is about the absence of a record, not the quality of the work. Every model
involved may have performed well; the point is that nobody can demonstrate it, so the next
choice is made on the same basis as the last one — impression.

## What it costs

A recurring, unresolvable argument, and a spend allocation that cannot be defended. Where a
policy exists about which work may go to which provider, its application cannot be shown at
all, which makes the policy decorative.

## What Majordomus does

An execution episode is a record: when it opened, when it closed, what it was for, and what
happened under it. A task names the profile it ran under, and that profile — capability
class, effort, verbosity, context — is projected into the instructions the worker reads, so
the intended configuration is a written fact rather than a habit. The ledger carries the
outcome, the verification command, its exit code and its duration, so work done under
different configurations can be compared on something.

## Before and after

```text
before   commit author: a person.   worker: unknown.   profile: unknown.

after    $ majordomus session list
         s-20260905T2214Z  closed  profile=deep-work  tasks=2  outcome=completed,partial
```

## What it does not do

It does not select, invoke or measure a model, and it cannot see what a provider actually
ran. It records what this repository decided and what happened, which is the half that is
missing today.
{% endraw %}
