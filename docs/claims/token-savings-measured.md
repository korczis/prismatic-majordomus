# A total-token change is stated only from verified, matched runs

## What it means

**Planned.** Majordomus will state how much it changes total token consumption only when matched live runs, with and without it, judged by the same hidden acceptance tests, meet the methodology's publication rule. Until then, the statement every surface shows is that no verified total-token-savings claim is available, beside whatever preliminary observation the runs support.

## How it works

The benchmark exists and runs: `majordomus economics run` executes each task of the corpus in a real coding harness, once in the fixture as a competent team keeps it and once with Majordomus installed, records the provider's usage and the gate verdicts, and the calculator pairs the runs, derives the per-pair reduction, its distribution and a seeded bootstrap interval, and checks the publication rule. What is missing is evidence: enough valid pairs, across enough task categories, with enough repetitions.

## How to see it

```bash
majordomus economics summary
majordomus economics explain effective_token_reduction
```

## What it does not cover

It is not a claim, and says so. The hypotheses the benchmark was built to test are recorded as hypotheses, never as results.

## Why it exists

A token-saving percentage is the most quotable thing this project could publish and the easiest to invent. This page is where the claim will stand once the evidence exists, and it cannot move to guaranteed before then: `economics check` refuses it.
