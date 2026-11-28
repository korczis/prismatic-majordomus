+++
title = "A proof test will record whether a developer who has not contributed to Majordomus reaches a first verified intent in a repository of their own with only the public site"
description = "**Planned.** Test t6-first-intent of the proof plan will record whether a developer who has not contributed to Majordomus reaches a first verified intent in a repository of their own, using only the public site."
weight = 104
[extra]
claim_id = "proof-t6-first-intent"
status = "planned"
source = "docs/claims/proof-t6-first-intent.md"
+++
{% raw %}

## What it means

**Planned.** Test `t6-first-intent` of the proof plan will record whether a developer who has not contributed to Majordomus reaches a first verified intent in a repository of their own, using only the public site.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. A criterion that cites the repository's own check (I3395), the documented criterion writer and the getting-started path (I3396), the scripted first-intent stage (I3397), the protocol that says who counts as a newcomer (I3398) and the newcomer trials (I3399) build it. The typed trial record of I3353 holds its records.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

A tool its builders can drive is not yet a tool anyone else can. This claim names the test that will put it in front of someone who has never seen the internal doctrine and record what happened.
{% endraw %}
