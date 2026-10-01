+++
title = "An advisor that times out, is rate limited, refuses its credential or answers malformed is recorded as exactly that, never blocks the work, opens a circuit that closes again after its cooldown, and never fails the session"
description = "A failing advisor costs the session one bounded wait, then the work continues on local"
weight = 214
[extra]
claim_id = "advisor-failure-is-recorded-and-recovers"
status = "guaranteed"
source = "docs/claims/advisor-failure-is-recorded-and-recovers.md"
+++
{% raw %}

## What it means

A failing advisor costs the session one bounded wait, then the work continues on local
evidence and completes. The failure is a record with its class and diagnostic. Repeated
failures take the advisor out of later plans for a cooldown, and it comes back on its own.

## How it works

`consultOne` bounds every exchange with a deadline. Each outcome is recorded by the
writer. Availability (`apps/majordomus-cli/src/reasoning/availability.rs`) reads the
outcomes newest first: two consecutive transient failures open the circuit for 10 minutes,
a rate limit for its retry-after, an authentication failure for an hour; past the cooldown
the advisor is `available` and `recovering`, and a completed answer closes the circuit. A
malformed or empty answer does not count against the advisor.

## How to see it

```
majordomus reasoning advisors     # temporarily_failed … until <time>, then (recovering)
```

`test/cases/733_advisor_failure_and_recovery.sh` times out, completes the task locally,
opens and recovers the circuit, and records a rate limit, an authentication failure and a
malformed answer for what each is.

## What it does not cover

It does not retry within a consultation; a failed exchange is a result, and the policy
decides about the next one. It does not detect a fixed credential by itself; the cooldown
or an explicit override does.

## Why it exists

An optional advisor that can hang or fail a session is a dependency in everything but
name. Bounded, recorded, self-healing failure is what keeps it optional (ADR 0098).
{% endraw %}
