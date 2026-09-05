---
schema: context/v1
id: ai.repo.benchmarks.rust
kind: context
title: Executable baselines
description: Per-platform baselines for the Rust executable's commands, with the policy that decides a regression.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [apps/majordomus-cli/src/bench, scripts/rust-check]
---

# Executable baselines

`baseline.<platform>.json` is what the executable's commands measured on that platform:
one file per platform, because a number from an arm64 laptop cannot refute a number from
a CI runner. `policy.yaml` holds the sampling and the fractions of the baseline a run may
exceed before it is called a regression, so the threshold is data the repository can
change deliberately rather than a constant compiled into a comparison.

The files are written by the benchmark run and read by the check that compares against
them; a hand-edited baseline is a claim without a measurement. Record a new one when the
change is a real cost that the repository accepts, and say so in the commit that does it.
