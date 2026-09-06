---
schema: moment/v1
id: failure-disappears-between-sessions
kind: moment
title: 'A failure that was seen once and never again'
short_title: 'Vanished failure'
hook: 'lost a failure a worker had already reproduced, because the session ended'
summary: 'A failure observed inside a session is described in that session and nowhere else, so the next one starts from the report rather than the evidence.'
status: stable
severity: medium
frequency: common
weight: 310
audiences: [ai-native-team, research-team, solo-builder, engineering-lead]
areas: [observability, verification]
lifecycle: [implementation, operations]
tags: [debugging, evidence, observability, continuity]
signals:
  - id: cannot-reproduce-again
    text: 'A failure was reproduced once and could not be reproduced afterwards.'
  - id: evidence-in-the-window
    text: 'The evidence for a bug exists only in a session window.'
  - id: starts-from-a-report
    text: 'Debugging restarts from somebody''s description rather than from a recorded observation.'
examples:
  - id: overnight-flake
    audience: ai-native-team
    title: 'The overnight flake'
    before: 'A worker hits an intermittent failure at 3am, describes it, and the session ends; the morning has a paragraph and no reproduction.'
    after: 'A checkpoint records what was true at that moment, capped so the next context can quote it whole, with the head it applied to.'
  - id: probe-lost
    audience: research-team
    title: 'A probe that showed something'
    before: 'An experiment produces an interesting negative result; the branch is discarded and so is the observation.'
    after: 'The observation is a knowledge record with class `observed` and the evidence it was read off.'
  - id: two-day-gap
    audience: solo-builder
    title: 'Two days later'
    before: 'Work resumes after a gap and the state of the investigation has to be rebuilt from the diff.'
    after: 'The handover names the objective, the current state and the next action, and says how far git has moved since.'
commands: [checkpoint, handover, history]
capabilities: [objects.search]
claims: [checkpoint-record, checkpoint-interval, handover-record, history-ledger-read, record-resolution]
doctrines: [majordomus.checkpoint-freshness, majordomus.handover-integrity, majordomus.handovers-carry-state, majordomus.ledger-integrity]
use_cases: [checkpoint-long-work, hand-work-between-sessions, read-back-what-happened]
related: [what-the-workers-did-last-night, discovery-never-becomes-knowledge, result-cannot-be-reproduced]
aliases: ['lost repro', 'evidence lost with the session', 'intermittent failure']
---

## The moment

At three in the morning a worker reproduced the intermittent failure and said so clearly.
By nine the session is gone. What survives is a sentence describing a failure that nobody
can now make happen.

## Why it happens

Reproduction is the expensive part of debugging and it lives in a running context: an
environment, a sequence, a piece of state. None of that is written down at the moment it
exists, because the worker is busy solving the problem, and by the time anyone wants it the
context has been discarded.

## Why a better model does not fix it

The failure was already found. What is missing is a durable artefact created at the moment
of the observation. A better worker finds it faster and loses it just as completely.

## What it costs

The investigation is repeated from the top, usually more than once, and each repetition is
paid at the cost of the original. Intermittent failures are the worst case: the second
investigation may not reproduce at all, so the bug is filed as unreproducible and returns
in production.

## What Majordomus does

Checkpointing is a first-class, cheap act with a hard length cap: `majordomus checkpoint`
records what was true a moment ago, short enough that the next context can quote it whole,
refusing an over-long body rather than truncating it. The profile sets an interval, and
`check` and `watch` report checkpoint age, so a long-running investigation that has recorded
nothing is visible. A session that ends writes a handover with objective, current state and
next action, each required, and the next one resolves it with a divergence label computed
from git.

## Before and after

```text
before   3am: "reproduced it — it's the retry loop under load"   (session, gone)

after    $ majordomus history --task t-…a4f1
         checkpoint  head=8c31f0e  "reproduced under 50 concurrent enqueues;
                                    the retry loop re-enters before the ack"
         handover    advanced      # Next Action: add the load case to test/queue
```

## What it does not do

It does not capture the environment, and it does not record a session's output. The worker
writes the checkpoint; the tool guarantees it is short, attached to a head, and readable by
whatever comes next.
