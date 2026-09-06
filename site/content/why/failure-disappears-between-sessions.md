+++
title = "A failure that was seen once and never again"
description = "A failure observed inside a session is described in that session and nowhere else, so the next one starts from the report rather than the evidence."
weight = 310
[extra]
id = "failure-disappears-between-sessions"
status = "stable"
source = ".ai/repo/why/moments/failure-disappears-between-sessions.md"
+++
{% raw %}

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
{% endraw %}
