+++
title = "A proof test will compare the accepted results of the same agents with and without Majordomus over a corpus of tasks registered before its first run"
description = "**Planned.** Test t2-ab-workflow of the proof plan will compare the accepted results of the same agents, with and without Majordomus. It runs over a corpus of tasks that is registered and stratified before its first run."
weight = 100
[extra]
claim_id = "proof-t2-ab-workflow"
status = "planned"
source = "docs/claims/proof-t2-ab-workflow.md"
+++
{% raw %}

## What it means

**Planned.** Test `t2-ab-workflow` of the proof plan will compare the accepted results of the same agents, with and without Majordomus. It runs over a corpus of tasks that is registered and stratified before its first run.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. The exact paired test (I3366), the registered suite (I3367), the new tasks (I3368, I3369), the registration and the T2 protocol (I3370), the decided primary outcome (I3371), the runs in both arms (I3372), the difficulty proxy (I3374) and the pinned treatment release (I3375) build it. Acceptance is measured as I3133 defines it.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

A comparison whose corpus, outcomes and stopping rule are chosen after the runs can show whatever its authors want. This claim names the test that will decide it in advance, and its row stays not run until a registered protocol counts a record.
{% endraw %}
