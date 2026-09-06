---
schema: moment/v1
id: same-dead-end-explored-twice
kind: moment
title: 'The same dead end, explored twice'
short_title: 'Repeated dead end'
hook: 'watched a worker spend a day on an approach that had already been ruled out'
summary: 'Negative results are the majority of experimental output and the part nobody records, so the search space is re-explored.'
status: stable
severity: medium
frequency: common
weight: 360
audiences: [research-team, solo-builder, ai-native-team, engineering-lead]
areas: [decisions, observability]
lifecycle: [planning, implementation]
tags: [experiments, negative-results, knowledge, waste]
signals:
  - id: tried-before
    text: 'An approach was attempted that somebody had already attempted and abandoned.'
  - id: negatives-unrecorded
    text: 'Abandoned experiments leave no record of why they were abandoned.'
  - id: branch-graveyard
    text: 'There are more discarded branches than anyone can characterise.'
examples:
  - id: two-workers-one-dead-end
    audience: ai-native-team
    title: 'Two workers, one dead end'
    before: 'A second worker independently tries the approach a first one abandoned, and abandons it for the same reason.'
    after: 'The abandonment is a knowledge record with class `lesson`, or a decision with its rejected alternative.'
  - id: six-weeks-later
    audience: research-team
    title: 'Six weeks later'
    before: 'An approach ruled out in the spring is proposed again in the summer, by the same team.'
    after: 'The record survives the branch and is searchable; `search` finds it in seconds.'
  - id: own-dead-end
    audience: solo-builder
    title: 'Your own dead end'
    before: 'You remember that something did not work and not why, so you check again to be sure.'
    after: 'The reason is on record with the evidence it was read off, so checking is reading.'
commands: [decision, knowledge, search]
capabilities: [objects.search]
claims: [decision-record, decision-attribution, record-search, no-transcripts]
doctrines: [majordomus.externalise-decisions, majordomus.decision-records, majordomus.decision-threshold, project.never-store-transcripts]
use_cases: [record-a-decision-before-it-is-forgotten, keep-decisions-out-of-the-transcript, find-an-object-without-reading-everything]
related: [discovery-never-becomes-knowledge, re-arguing-a-settled-decision, result-cannot-be-reproduced]
aliases: ['repeated experiment', 'negative results lost', 'rediscovered dead end']
---

## The moment

A worker spends a day establishing that the streaming approach cannot meet the ordering
guarantee. It is careful work and the conclusion is right. It is also the conclusion
somebody reached in April.

## Why it happens

Positive results become code, which is durable. Negative results become nothing: the branch
is deleted, the session ends, and the only trace is an absence — the approach is not in the
codebase, which is indistinguishable from nobody having tried it.

## Why a better model does not fix it

The second worker reasoned correctly from the same starting point and reached the same
answer. That is the definition of a reproducible negative result, and it is being paid for
twice because the first one was not written down.

## What it costs

In experimental work, most attempts fail, so most of the output is exactly the kind that is
being discarded. The search space is re-explored at full price, and the team's apparent
progress rate is lower than its actual understanding.

## What Majordomus does

A decision records the alternative that was rejected, not only the one chosen — `--rejected`
is the field that makes a dead end durable, and `--why` is refused when empty. A finding
that is not a decision is a knowledge record with a class: `lesson` is what an earlier
attempt cost, `constraint` is something outside the repository that limits it. Both are
searchable across handovers, checkpoints, decisions, questions and the ledger, in the order
in which to trust them. Nothing stores the conversation the finding came from.

## Before and after

```text
before   April:  branch deleted.      August: the same day, again.

after    $ majordomus search "ordering guarantee" --kind decision --kind knowledge
         decision t-…  rejected: streaming — cannot preserve per-key ordering
                       across a rebalance; evidence test/cases/58_ordering.sh
```

## What it does not do

It does not decide what is worth recording, and it will not mine an abandoned branch for
lessons. It makes recording a negative result a one-line act with a place to put it.
