---
schema: moment/v1
id: implementation-contradicts-the-decision
kind: moment
title: 'Code that quietly contradicts a written decision'
short_title: 'Code against the decision'
hook: 'found code that contradicted a decision the repository had written down'
summary: 'The decision was recorded and the implementation went the other way, because nothing relates a decision to the paths it governs.'
status: stable
severity: high
frequency: occasional
weight: 200
audiences: [open-source-maintainer, engineering-lead, ai-native-team, platform-team]
areas: [decisions, governance]
lifecycle: [implementation, review]
tags: [decisions, drift, review, traceability]
signals:
  - id: written-and-ignored
    text: 'A decision is written down somewhere and the code does the opposite.'
  - id: adr-nobody-reads
    text: 'The architecture decisions are recorded and no session has ever loaded one.'
  - id: no-link-to-code
    text: 'Nothing connects a decision to the files it constrains.'
examples:
  - id: adr-unread
    audience: ai-native-team
    title: 'An ADR nobody was given'
    before: 'The decision is in the ADR directory; the worker was given the root instruction file and the diff.'
    after: 'A decision names what it put in force as typed references, so the rule, the file and the test it governs are edges, not prose.'
  - id: review-catches-it
    audience: open-source-maintainer
    title: 'Caught in review, again'
    before: 'The reviewer is the only mechanism connecting a written decision to a contribution that breaks it.'
    after: 'The reverse index — what this file was decided by — is derived from the decision''s own references.'
  - id: two-years-later
    audience: engineering-lead
    title: 'A design abandoned by accident'
    before: 'A sequence of individually reasonable changes leaves the system doing what an ADR explicitly ruled out.'
    after: 'The decision is superseded deliberately, with the replacement naming it, or it stands and the change is refused.'
commands: [adr, context, doctor]
capabilities: [graph.get, objects.get]
claims: [adr-traceability, adr-catalogue, context-impact, pointer-integrity]
doctrines: [majordomus.adr-integrity, majordomus.context-integrity, majordomus.externalise-decisions]
use_cases: [trace-a-change-to-the-context-it-affects, keep-decisions-out-of-the-transcript]
related: [re-arguing-a-settled-decision, decision-only-in-a-transcript, rule-in-a-readme-nobody-loaded]
aliases: ['ADR ignored', 'design drift', 'decision not applied']
---

## The moment

The architecture decision is written, accepted and unambiguous. The implementation does the
thing it rejected. Nobody defied the decision; the change was made by someone who had never
seen it, and reviewed by someone who had forgotten it.

## Why it happens

A decision record is a document in a directory. The work happens in files somewhere else.
Unless something relates the two, the decision is discoverable only by a reader who already
suspects it exists — which is the reader who did not need it.

## Why a better model does not fix it

The worker was not given the decision. Nothing about capability changes which documents
reach a session; that is a retrieval question with a deterministic answer, and it should be
answered by resolution rather than by inference.

## What it costs

A design erodes one reasonable change at a time. By the time it is noticed, the system does
something nobody chose, and reversing it costs more than the original decision did — so
usually the decision is quietly rewritten to match the code.

## What Majordomus does

A decision names what it put in force, as typed references: a rule of the effective set, a
claim, a file, a behavioural test. Every reference is validated, and the knowledge graph
turns each into an edge, so the reverse index — what this rule, this file or this test was
decided by — is derived rather than authored twice. Scoped context documents attach to the
paths they govern, and `context affected` reads a change set from git and reports which
documents and scopes it touches, including a tracked source whose document is now due for
review.

## Before and after

```text
before   .ai/repo/adrs/0007-….md      (accepted, unread)
         lib/skills/registry.rs       (does the rejected thing)

after    $ majordomus context affected --staged
         lib/skills/registry.rs  tracked by adr-0007 "skills are data, not registrations"
```

## What it does not do

It does not judge whether a change contradicts a decision — that is a reading, and a person
does it. It makes sure the decision is in front of the person and the worker doing the
reading, and that superseding one is an explicit, reviewable act.
