+++
title = "Code that quietly contradicts a written decision"
description = "The decision was recorded and the implementation went the other way, because nothing relates a decision to the paths it governs."
weight = 200
[extra]
id = "implementation-contradicts-the-decision"
status = "stable"
source = ".ai/repo/why/moments/implementation-contradicts-the-decision.md"
+++
{% raw %}

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
{% endraw %}
