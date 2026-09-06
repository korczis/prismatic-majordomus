---
schema: moment/v1
id: spend-not-tied-to-outcomes
kind: moment
title: 'Spend that cannot be tied to anything accepted'
short_title: 'Cost without outcome'
hook: 'looked at the bill and could not say which of it produced anything'
summary: 'Consumption is measured per account and outcomes are recorded per person, so the two can never be joined.'
status: stable
severity: medium
frequency: common
weight: 300
audiences: [enterprise, engineering-lead, research-team, agency]
areas: [cost, observability]
lifecycle: [operations]
tags: [cost, outcomes, telemetry, reporting]
signals:
  - id: bill-by-account
    text: 'Consumption is known per account or per month and not per piece of work.'
  - id: no-accepted-denominator
    text: 'Nobody can say what fraction of what was produced was actually accepted.'
  - id: justify-the-spend
    text: 'Justifying the spend means telling a story rather than showing a ratio.'
examples:
  - id: monthly-invoice
    audience: enterprise
    title: 'A number with no denominator'
    before: 'The invoice is a single figure; the value it bought is asserted in a slide.'
    after: 'Outcomes are typed events with verification attached, so the accepted side of the ratio is at least a real count.'
  - id: eighty-attempts
    audience: research-team
    title: 'Eighty attempts, nine merges'
    before: 'Cost is attributed to the attempts; nothing records that most of them were never integrated.'
    after: 'Started, handed over and finished-with-an-outcome are distinct events, so the gap is visible without an interview.'
  - id: per-client
    audience: agency
    title: 'Attributing to a client'
    before: 'Consumption cannot be split by engagement, so it is allocated by headcount and argued about.'
    after: 'Work is attached to tasks, and tasks to scopes and repositories; the allocation has a basis.'
commands: [history, finish, session]
capabilities: [perf.counters, health.report]
claims: [typed-outcome, history-ledger-read, session-records, cost-per-outcome, telemetry]
doctrines: [majordomus.verify-outcomes, majordomus.ledger-integrity, majordomus.session-records, project.performance-evidence]
use_cases: [read-back-what-happened, accept-or-refuse-finished-work, open-and-close-a-session]
related: [worker-output-never-integrated, no-record-why-this-model, what-the-workers-did-last-night]
aliases: ['cost per outcome', 'unattributed spend', 'ROI of agents']
---

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
