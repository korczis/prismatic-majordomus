+++
title = "Every claim about speed is a recorded measurement"
description = "The benchmark targets are derived from the registry, so a capability nothing times cannot be merged; runs are compared with a tracked baseline per platform under a regression policy; the shell tool reports where every command's time went; and derived state is computed once per state version rather than per call."
weight = 180
[extra]
id = "benchmarks"
status = "stable"
source = ".ai/repo/features/benchmarks.md"
+++
{% raw %}

## What it does

`majordomus bench` times every operation through the real transports and reports the
slowest first; `bench coverage --check` refuses a required target nobody covers; `bench
--check` compares a run with this platform's baseline under the policy's thresholds; `bench
baseline update` records a reviewable, tracked baseline and refuses a dirty tree. The
executor's counters say whether an execution was answered from the cache, so a claim about
work done is a count rather than a clock.

## What it does not do

A baseline belongs to the machine that measured it; a value from one platform says nothing
about another, and no number in prose is trusted over the file that recorded it.
{% endraw %}
