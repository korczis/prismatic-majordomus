+++
title = "A proof test will compare the wall time of coordinated workers on the mesh with that of one agent on the same tasks, with the accepted share held to a margin registered in advance"
description = "**Planned.** Test t4-mesh-value of the proof plan will compare the wall time of coordinated workers on the mesh with the wall time of one agent on the same tasks. The accepted share is held to a margin registered in advance."
weight = 102
[extra]
claim_id = "proof-t4-mesh-value"
status = "planned"
source = "docs/claims/proof-t4-mesh-value.md"
+++
{% raw %}

## What it means

**Planned.** Test `t4-mesh-value` of the proof plan will compare the wall time of coordinated workers on the mesh with the wall time of one agent on the same tasks. The accepted share is held to a margin registered in advance.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. The non-inferiority guard (I3385), the coordinated-workers runner arm (I3386, declared as the topology arm of I3298), the batch tasks (I3387), the registered mesh suite and the T4 protocol (I3388) and the runs in both arms (I3389) build it.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

Several workers can finish sooner by doing worse work. This claim names the test that will measure time only together with the accepted share, so that a quicker run of worse work cannot count in the mesh's favour.
{% endraw %}
