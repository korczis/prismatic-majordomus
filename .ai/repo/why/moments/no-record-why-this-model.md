---
schema: moment/v1
id: no-record-why-this-model
kind: moment
title: 'Nobody can say why that worker was chosen'
short_title: 'Unrecorded routing'
hook: 'could not say why one provider was used for that work and another for this'
summary: 'Provider and model selection is made implicitly, per session, and recorded nowhere, so it cannot be reviewed or repeated.'
status: stable
severity: medium
frequency: common
weight: 290
audiences: [research-team, enterprise, agency, engineering-lead]
areas: [cost, observability]
lifecycle: [planning, operations]
tags: [model-selection, provenance, routing, cost]
signals:
  - id: chosen-by-habit
    text: 'Which assistant is used for which work is decided by habit, not by a stated rule.'
  - id: no-record-of-choice
    text: 'Nothing records which model or provider produced a given piece of work.'
  - id: cannot-compare
    text: 'Two providers have been used for months and nobody can compare how they did.'
examples:
  - id: ab-without-a-record
    audience: research-team
    title: 'A comparison nobody can settle'
    before: 'Two providers are used side by side for a quarter, and the question of which was better is answered by preference.'
    after: 'Each task records the profile it ran under; the outcome, the verification and the duration are events, so the comparison has data behind it.'
  - id: client-question
    audience: agency
    title: 'The client asks what wrote this'
    before: 'The honest answer is a shrug and a guess based on the date.'
    after: 'The tool the session ran under is identifiable, and the task record and ledger carry what happened under it.'
  - id: policy-requires-it
    audience: enterprise
    title: 'A routing policy with no evidence of application'
    before: 'A policy says which class of work may use which provider; nothing records whether it was followed.'
    after: 'The profile in force is projected into the instructions and recorded with the task, so the policy has an observable trace.'
commands: [start, history, session]
capabilities: [repository.info, peers.list]
claims: [profile-axes, capability-class, session-records, history-ledger-read, tool-location-independent]
doctrines: [majordomus.session-records, majordomus.justified-escalation, majordomus.ledger-integrity, project.never-author-identity]
use_cases: [open-and-close-a-session, know-which-tool-is-running, read-back-what-happened]
related: [strongest-model-renames-a-variable, weak-model-on-architecture, spend-not-tied-to-outcomes]
aliases: ['model provenance', 'which model wrote this', 'routing not recorded']
---

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
