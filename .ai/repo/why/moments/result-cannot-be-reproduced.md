---
schema: moment/v1
id: result-cannot-be-reproduced
kind: moment
title: 'A result nobody can reproduce, including its author'
short_title: 'Irreproducible result'
hook: 'could not reproduce a result because nothing recorded the state it came from'
summary: 'The result is recorded and the conditions that produced it are not, so it is an anecdote rather than a measurement.'
status: stable
severity: high
frequency: occasional
weight: 370
audiences: [research-team, enterprise, platform-team, engineering-lead]
areas: [observability, verification]
lifecycle: [operations, review]
tags: [reproducibility, evidence, benchmarks, provenance]
signals:
  - id: number-without-conditions
    text: 'A measurement is quoted without the conditions that produced it.'
  - id: cannot-repeat
    text: 'A result could not be repeated and nobody can say what differed.'
  - id: baseline-by-hand
    text: 'A performance baseline was written by hand rather than produced by a run.'
examples:
  - id: the-benchmark-number
    audience: platform-team
    title: 'The number in the slide'
    before: 'A latency figure is quoted for a year; nobody can say which machine, which build or which input produced it.'
    after: 'A baseline is a tracked file a run wrote, per platform, with the sampling policy beside it; a threshold is set from a run rather than guessed.'
  - id: experiment-conditions
    audience: research-team
    title: 'The conditions were the experiment'
    before: 'A promising result is reported; the prompt, the model and the repository state that produced it were not captured.'
    after: 'The session records what it ran under, and evidence names the command that produced it and the commit it applied to.'
  - id: audit-a-measurement
    audience: enterprise
    title: 'A measurement used in a decision'
    before: 'A capacity decision rests on a figure whose provenance is a message.'
    after: 'Evidence is refused unless it names a command or an artifact, and the recorded result carries the exit code and the commit.'
commands: [bench, plan, session]
capabilities: [perf.counters, health.report]
responsibilities: [watch, plan]
claims: [evidence-gates-done, rust-hot-path-benchmarks, benchmark-coverage-derived, session-records, reproduce-command]
doctrines: [project.performance-evidence, project.benchmarkable-commands, majordomus.session-records, majordomus.verify-outcomes]
use_cases: [prove-performance-with-benchmarks, complete-an-issue-only-with-its-evidence, open-and-close-a-session]
related: [failure-disappears-between-sessions, no-record-why-this-model, issue-says-done-tests-disagree]
aliases: ['irreproducible benchmark', 'no provenance for a number', 'conditions not captured']
---

## The moment

The figure has been quoted in three places for a year. Somebody finally tries to reproduce
it and gets something 40% different. Nobody can say whether the system changed, the machine
changed, or the original measurement was taken differently — because none of that was
recorded with the number.

## Why it happens

A number is easy to write down and its conditions are not, so the number travels and the
conditions stay behind. The moment it appears in a second document it has lost the last of
its context, and from then on it is quoted rather than measured.

## Why a better model does not fix it

Nothing about the measurement was a reasoning task. The conditions existed and were not
captured; no later analysis can recover them.

## What it costs

Decisions made on a number that means something different from what everyone thinks. And
the specific waste of re-measuring, which is only discovered to be necessary when a result
finally contradicts itself in public.

## What Majordomus does

A performance claim names a measurement, and the measurement is compared against a baseline
that is a tracked file a run wrote — one per platform, because a figure from a laptop cannot
refute a figure from a runner. The sampling and the regression thresholds are data beside
it, so a threshold is changed deliberately rather than tuned until a gate passes. Evidence
elsewhere follows the same discipline: `plan evidence` refuses narrative and needs a command
or an artifact, and records the result with the commit. An execution episode records what it
ran under.

## Before and after

```text
before   "about 40ms"        (which machine? which build? which input?)

after    .ai/repo/benchmarks/rust/baseline.macos-aarch64-debug.json
         written by: majordomus bench baseline update  (refuses a dirty tree)
         compared by: majordomus bench --check         (policy.yaml thresholds)
```

## What it does not do

It does not make a measurement meaningful, and a baseline recorded on a noisy machine is a
noisy baseline. It refuses a number that no run produced, and refuses to record one from a
dirty tree.
