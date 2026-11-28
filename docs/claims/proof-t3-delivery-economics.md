# A proof test will state each task's delivery economics over a declared window

## What it means

**Planned.** Test `t3-delivery-economics` of the proof plan will state each task's wall time, human interventions, CI reruns and spend over a delivery window that is declared before it opens. Every field it cannot observe is named as not observed, never as zero.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. The per-task reading (I3376), interventions by type (I3377, I3378), CI reruns (I3379), spend (I3380), the definition of accepted (I3381), the run-record shape (I3382), the window protocol (I3383) and the measured window (I3384) build it. The typed trial record of I3353 holds its record.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

What a repository's own work costs is the question an adopter asks first, and the one most easily answered with a guess. This claim names the test that will answer it from records, with every gap shown beside its denominator.
