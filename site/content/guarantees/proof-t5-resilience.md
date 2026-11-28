+++
title = "A proof test will inject worker, connection and conflict faults into work in progress and record whether the work resumes to an accepted finish with no commit lost and no completion duplicated"
description = "**Planned.** Test t5-resilience of the proof plan will inject faults into work in progress: a worker killed mid-task, a connection cut while a claim or handover is in flight, and two workers taking the same work. It records whether the work resumes to an accepted finish with no commit lost and no completion duplicated."
weight = 103
[extra]
claim_id = "proof-t5-resilience"
status = "planned"
source = "docs/claims/proof-t5-resilience.md"
+++
{% raw %}

## What it means

**Planned.** Test `t5-resilience` of the proof plan will inject faults into work in progress: a worker killed mid-task, a connection cut while a claim or handover is in flight, and two workers taking the same work. It records whether the work resumes to an accepted finish with no commit lost and no completion duplicated.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. The conservation reading (I3390), the trials protocol and its fault kinds (I3391), the mesh lab's work faults (I3392), the trials in real agent sessions (I3393) and the committed trials with their verdict (I3394) build it. The typed trial record of I3353 holds its records.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

Continuity and coordination are only worth their machinery if work survives the failures they exist for. This claim names the test that will inject those failures on purpose and count what was lost.
{% endraw %}
