---
schema: moment/v1
id: discovery-never-becomes-knowledge
kind: moment
title: 'A hard-won discovery that never became knowledge'
short_title: 'Discovery lost'
hook: 'spent an hour discovering something the repository already knew, twice'
summary: 'A worker learns something expensive about the codebase, uses it once, and it dies with the session because nothing turned it into a durable record.'
status: stable
severity: medium
frequency: common
weight: 110
audiences: [solo-builder, agency, research-team, ai-native-team]
areas: [context]
lifecycle: [implementation, maintenance]
tags: [knowledge, context, memory, waste]
signals:
  - id: rediscovered
    text: 'Something non-obvious about this codebase was worked out from scratch that had been worked out before.'
  - id: no-place-to-put-it
    text: 'A worker learned something worth keeping and there was nowhere obvious to put it.'
  - id: findings-in-chat
    text: 'Useful findings from a session exist only inside that session.'
examples:
  - id: the-flaky-cause
    audience: ai-native-team
    title: 'Why that test is flaky'
    before: 'A worker spends an hour establishing that the flake is a clock dependency; the finding is stated in the session and lost with it.'
    after: 'The finding is a knowledge record with class `fact` and its evidence, and the next worker is assembled with it.'
  - id: client-quirk
    audience: agency
    title: 'The client build quirk'
    before: 'The build needs a flag nobody documented; each new person on the account rediscovers it, at a cost of an afternoon.'
    after: 'It is a knowledge record with class `constraint` — something outside the repository that limits it — found by `search` in seconds.'
  - id: dead-end-note
    audience: research-team
    title: 'The approach that cannot work'
    before: 'An approach is ruled out for a good technical reason; six weeks later it is attempted again.'
    after: 'The record carries class `lesson` and what the earlier attempt cost, so the second attempt does not start.'
commands: [knowledge, decision, search]
capabilities: [objects.search, objects.list]
responsibilities: [state, layer]
claims: [record-search, decision-record, context-assembly, minimum-context]
doctrines: [majordomus.externalise-decisions, project.never-store-transcripts, majordomus.minimum-sufficient-context]
use_cases: [classify-what-belongs-in-the-context, record-a-decision-before-it-is-forgotten, find-an-object-without-reading-everything]
related: [re-explaining-context, same-dead-end-explored-twice, decision-only-in-a-transcript]
aliases: ['knowledge capture', 'findings lost with the session', 'rediscovery']
---

## The moment

A worker spends an hour establishing why the integration test is flaky — a clock dependency
three layers down — fixes it, and says so. Six weeks later the same flake returns in a
different test and somebody spends the hour again.

## Why it happens

There was nowhere for the finding to go that anybody would look. A comment in the code says
it to whoever opens that file; a chat message says it to whoever was present; a wiki page
says it to whoever searches for the right words. None of them is loaded by the next worker,
so the default outcome for any discovery is that it evaporates.

## Why a better model does not fix it

The discovery was made correctly the first time. The failure is entirely between sessions:
nothing converted a finding into something the next worker is given. Better models make the
rediscovery faster and no less repeated.

## What it costs

The hour, each time. And a subtler cost: because rediscovery is normal, workers stop
treating existing code as evidence of a prior decision, and start treating every constraint
as something to be worked around.

## What Majordomus does

Knowledge is a declared kind, not a folder of notes. `majordomus knowledge` records one
durable statement about the repository with the class of statement it is — `fact`,
`convention`, `constraint`, `memory` or `lesson` — how it is known (`observed`, `inferred`,
`decided`) and how far it has been confirmed. A record whose status is `verified` must name
the evidence it was confirmed against; nothing is confirmed by having been written down.

Knowledge sources are declared in one file, so discovery goes through the version-control
index rather than the filesystem: an untracked scratch file is not knowledge, and two
machines see the same list in the same order. `majordomus context` draws on records with
class `memory` when it assembles what the next worker needs.

## Before and after

```text
before   finding -> session -> nothing

after    majordomus knowledge add "the integration clock is injected, not read" \
           --class fact --epistemics observed --evidence test/cases/42_clock.sh
         # class memory records are offered to the next context, within budget
```

## How to verify it

Record a finding and start a new session: `majordomus context` offers it. Mark a record
`verified` without evidence and it is refused.

## What it does not do

It does not extract knowledge from a transcript, and it will not store one. A machine
extraction produces a `candidate`, never a `verified` record — confirmation is a person's
act, against evidence that is named.
