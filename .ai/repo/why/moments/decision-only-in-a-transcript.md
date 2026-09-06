---
schema: moment/v1
id: decision-only-in-a-transcript
kind: moment
title: 'The decision exists, in a conversation nobody can find'
short_title: 'Decision in a transcript'
hook: 'went looking for why we chose this and found a chat log'
summary: 'A decision that was reached in a session is stored where only that session can read it, so it is neither reviewable nor discoverable.'
status: stable
severity: high
frequency: constant
weight: 190
audiences: [solo-builder, agency, ai-native-team, enterprise]
areas: [decisions, observability]
lifecycle: [planning, implementation]
tags: [decisions, provenance, adr, transcripts]
signals:
  - id: answer-is-a-log
    text: 'The answer to "why is it built this way" is a conversation log.'
  - id: no-adr
    text: 'Significant choices are made without any durable record of the alternative that was rejected.'
  - id: cannot-find-it
    text: 'A decision is known to have been made and nobody can find where.'
examples:
  - id: architecture-in-a-window
    audience: solo-builder
    title: 'The design conversation'
    before: 'An hour of design reasoning produces a good decision and lives in a window that is closed that evening.'
    after: '`decision add` records the choice, its reason and the rejected alternative; the head and task are computed from git.'
  - id: client-handover
    audience: agency
    title: 'Handing the reasoning over'
    before: 'The engagement ends and the client inherits code whose constraints have no stated reasons.'
    after: 'Durable decisions are records in the repository, and the significant ones are promoted to architecture decisions with context and consequences.'
  - id: audit-provenance
    audience: enterprise
    title: 'Who decided, and on what basis'
    before: 'The provenance of a design choice is a screenshot of a conversation.'
    after: 'The record carries the decision, the reason, the rejected alternative, the task and the commit, and it is append-only.'
commands: [decision, adr, search]
capabilities: [objects.search, objects.get]
responsibilities: [state, layer]
claims: [decision-record, decision-attribution, adr-catalogue, adr-traceability, adr-propose, no-transcripts]
doctrines: [majordomus.decision-records, majordomus.adr-integrity, majordomus.externalise-decisions, project.never-store-transcripts, majordomus.decision-threshold]
use_cases: [keep-decisions-out-of-the-transcript, record-a-decision-before-it-is-forgotten, read-back-what-happened]
related: [re-arguing-a-settled-decision, implementation-contradicts-the-decision, discovery-never-becomes-knowledge]
aliases: ['no ADR', 'rationale lost', 'decision provenance']
---

## The moment

Someone asks why the service writes to the queue before the database rather than after. The
answer is known — it was worked out carefully, once — and the only place it exists is a
conversation that has since been closed.

## Why it happens

Recording a decision is a separate act from making one, and it comes at the moment when
everyone involved is certain they will remember. They do remember, for about a fortnight.
Meanwhile the artefact that does exist is the transcript, which is long, unindexed, and
written in the register of a conversation rather than of a record.

## Why a better model does not fix it

A model can summarise a transcript into something that reads like a decision record, but
the result is a reconstruction with no standing: it cannot distinguish what was decided from
what was merely discussed, and nothing links it to the commit it applied to.

## What it costs

The decision gets re-argued (which is its own moment) or, worse, silently contradicted. And
because the reason is unavailable, the next person cannot tell a deliberate constraint from
an accident, so they treat all constraints as accidents.

## What Majordomus does

`majordomus decision add` records what was decided and refuses to record it without `--why`.
`--rejected` records the alternative that was ruled out; `--evidence` says where to look.
The task id and the git head are computed, never typed, and entries are append-only:
`--supersedes` records a replacement and refuses to point at a decision that does not exist.
A decision durable enough to outlive the task is promoted to an architecture decision under
the layer, with its context, its alternatives and its consequences, and `adr propose` will
draft one from a local record rather than from prose.

## Before and after

```text
before   "we discussed it in the design session"     (which one?)

after    $ majordomus search "queue before database" --kind decision
         t-…a4f1  head=8c31f0e
           decided:  write to the queue before the database
           why:      a lost enqueue is recoverable; a lost row is not
           rejected: writing after, which loses the event on a crash between the two
```

## What it does not do

It does not decide what is worth recording, and it does not mine transcripts. The threshold
is a judgement the repository states as a rule; the tool refuses a decision with no reason
and never stores a conversation.
