---
schema: feature/v1
id: benchmarks
kind: feature
title: Every claim about speed is a recorded measurement
short_title: Benchmarks
headline: Every externally callable operation is timed directly, over MCP and over HTTP, the denominator is generated from the registry, and a regression against the accepted baseline fails the check.
summary: The benchmark targets are derived from the registry, so a capability nothing times cannot be merged; runs are compared with a tracked baseline per platform under a regression policy; the shell tool reports where every command's time went; and derived state is computed once per state version rather than per call.
status: stable
weight: 180
featured: false
areas: [verification, cost]
modules: [perf]
commands: [bench]
rules: [project.performance-evidence, project.rust-benchmark-coverage, project.benchmarkable-commands, project.rust-hot-path, project.hot-path-reads-once, project.derived-once]
docs: [docs/PERFORMANCE.md]
adrs: [adr-0004]
claims: [benchmark-coverage-derived, hot-path-no-rebuild, execution-cache-equivalence, rust-hot-path-benchmarks, rust-evidence-gates, rust-coverage-floor]
use_cases: [prove-performance-with-benchmarks]
related: [declare-once, ci]
tags: [performance, benchmarks]
---

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
