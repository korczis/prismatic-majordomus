+++
title = "The decision exists, in a conversation nobody can find"
description = "A decision that was reached in a session is stored where only that session can read it, so it is neither reviewable nor discoverable."
weight = 190
[extra]
id = "decision-only-in-a-transcript"
status = "stable"
source = ".ai/repo/why/moments/decision-only-in-a-transcript.md"
+++
{% raw %}

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
{% endraw %}
