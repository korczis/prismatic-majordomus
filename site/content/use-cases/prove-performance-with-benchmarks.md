+++
title = "Prove the tool is fast, not assume it, and refuse a regression"
description = "Measure every public command and every capability of the executable against a committed baseline, and let the check fail on a regression rather than a reviewer noticing later."
weight = 8
[extra]
id = "prove-performance-with-benchmarks"
source = ".ai/repo/use-cases/prove-performance-with-benchmarks.md"
category = "performance"
maturity = "guaranteed"
+++

## Situation

A refactor made doctor slower and nobody measured; a hot path in the executable rebuilds the index on every request and the only evidence is a feeling that MCP calls got sluggish. Performance claims exist in the README and nowhere else.

## What you run

- `bench`: samples every public command of the shell tool, cold and warm, after warm-up runs, and writes a result document under the local half; --check refuses a regression larger than the fractions the policy declares
- `doctor`: reports its own wall time against the budget in the policy, as a warning that names the key and never as the exit code

## Outcome

Every externally callable operation is a benchmark target derived from the registry, directly and over each transport it is exposed on, with a coverage table that CI checks is complete. Hundreds of MCP or HTTP requests rebuild nothing, and the counters prove it. The hot paths of the executable carry criterion benchmarks that build on every push, and the thresholds and budgets are policy, measured and changed with a run.
