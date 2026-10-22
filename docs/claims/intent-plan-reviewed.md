# A gap that leaves a criterion unanswered is refused, and an issue serving an intent that started without a critique or while a blocking finding is open fails intent validation

## What it means

Planning starts from two records a worker writes: a gap, which answers every criterion of the intent with what was observed at a commit, and a critique, the adversarial pass over the plan with each finding resolved. A gap that skips a criterion is refused. Work under an intent that is `ACTIVE`, `VERIFY` or `DONE` while the intent has no critique is `executing_without_critique`, and while a blocking finding is open, `executing_with_open_blocker`.

## How it works

`apps/majordomus-cli/src/intent_review.rs` validates both records against the intent and the plan, and the findings join `majordomus intent validate`, which the `intent-check` gate runs. This judgment is made after the start: an issue that `plan start` has already moved is what fails. Whether the start itself is refused is the policy's choice, and a different claim.

## How to see it

```bash
majordomus plan start I0001     # succeeds where the policy does not require binding
majordomus intent validate      # FAIL executing_without_critique, exit 10
```

## What it does not cover

This claim is the judgment after the start. Refusing the start itself is [`intent-refused-at-plan-start`](intent-refused-at-plan-start.md), which holds where the policy says `intent.binding: required`; with the key absent, `off` or `advisory` the start succeeds and this is what names the work. Majordomus writes neither the gap nor the critique and runs no opposition pass of its own.

## Why it exists

What a worker observed and what a review found are lost with the session that produced them unless they are records. Holding started work to them is what makes the review a precondition someone can check rather than a habit.
