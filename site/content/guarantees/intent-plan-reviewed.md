+++
title = "A gap that leaves a criterion unanswered is refused, and an issue serving an intent that started without a critique or while a blocking finding is open fails intent validation"
description = "Planning starts from two records a worker writes: a gap, which answers every criterion of the intent with what was observed at a commit, and a critique, the adversarial pass over the plan with each finding resolved. A gap that skips a criterion is refused. Work under an intent that is ACTIVE, VERIFY or DONE while the intent has no critique is executing_without_critique, and while a blocking finding is open, executing_with_open_blocker."
weight = 113
[extra]
claim_id = "intent-plan-reviewed"
status = "guaranteed"
source = "docs/claims/intent-plan-reviewed.md"
+++
{% raw %}

## What it means

Planning starts from two records a worker writes: a gap, which answers every criterion of the intent with what was observed at a commit, and a critique, the adversarial pass over the plan with each finding resolved. A gap that skips a criterion is refused. Work under an intent that is `ACTIVE`, `VERIFY` or `DONE` while the intent has no critique is `executing_without_critique`, and while a blocking finding is open, `executing_with_open_blocker`.

## How it works

`apps/majordomus-cli/src/intent_review.rs` validates both records against the intent and the plan, and the findings join `majordomus intent validate`, which the `intent-check` gate runs. The judgment is made after the start: an issue that `plan start` has already moved is what fails.

## How to see it

```bash
majordomus plan start I0001     # succeeds: the start is not refused here
majordomus intent validate      # FAIL executing_without_critique, exit 10
```

## What it does not cover

`plan start` does not refuse the start; refusing it there is `intent-refused-at-plan-start`, which is planned. Majordomus writes neither the gap nor the critique and runs no opposition pass of its own.

## Why it exists

What a worker observed and what a review found are lost with the session that produced them unless they are records. Holding started work to them is what makes the review a precondition someone can check rather than a habit.
{% endraw %}
