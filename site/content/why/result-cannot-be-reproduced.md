+++
title = "A result nobody can reproduce, including its author"
description = "The result is recorded and the conditions that produced it are not, so it is an anecdote rather than a measurement."
weight = 370
[extra]
id = "result-cannot-be-reproduced"
status = "stable"
source = ".ai/repo/why/moments/result-cannot-be-reproduced.md"
+++
{% raw %}

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
{% endraw %}
