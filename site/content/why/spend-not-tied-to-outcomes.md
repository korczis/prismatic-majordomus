+++
title = "Spend that cannot be tied to anything accepted"
description = "Consumption is measured per account and outcomes are recorded per person, so the two can never be joined."
weight = 300
[extra]
id = "spend-not-tied-to-outcomes"
status = "stable"
source = ".ai/repo/why/moments/spend-not-tied-to-outcomes.md"
+++
{% raw %}

## The moment

The monthly figure is large and growing. The question is not whether it is worth it — it
probably is — but which part of it was. There is no way to answer, because consumption is
measured per account and outcomes live in people's heads.

## Why it happens

The two halves of the ratio are recorded by different systems with no shared key.
Consumption belongs to a provider account; acceptance belongs to a review conversation. The
unit that would join them — a piece of work, with an identity, a cost and an outcome — does
not exist anywhere.

## Why a better model does not fix it

A cheaper or better model changes the numerator and leaves the join missing. The question
"was this worth it" stays unanswerable however good the workers get.

## What it costs

Budget decisions made on anecdote, in both directions: over-spending that nobody can
challenge, and cuts that fall on the work that was actually paying for itself.

## What Majordomus does

The accepted side of the ratio becomes real first. Work is a task with an identity computed
from git; finishing it records a typed outcome, the verification command that ran, its exit
code and its duration. An execution episode records when it opened and closed and what
happened under it. `history` reads back what was started, handed over and accepted, so the
denominator of any cost question is a count rather than an impression.

Attaching consumption itself to those units is on the roadmap and is not claimed today; the
claims matrix carries it as planned, with no implementation and no test, rather than as a
capability.

## Before and after

```text
before   invoice: one number.      accepted work: an anecdote.

after    $ majordomus history --since 30d --event task_finished --json | \
             jq -r '.outcome' | sort | uniq -c
           41 completed
           18 partial
            7 failed
```

## What it does not do

It does not measure tokens, money or model consumption, and it does not integrate with a
provider's billing. It makes the outcome side countable, which is the side that was missing
in every environment this tool came from.
{% endraw %}
