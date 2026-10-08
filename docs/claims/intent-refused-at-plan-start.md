# Starting an issue that serves an intent with no critique, or with a blocking finding open, will be refused when it is started rather than reported afterwards

## What it means

**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered.

The intended shape is that `majordomus plan start` refuses an issue whose intent has no critique, or has a blocking finding still open, so the review of a plan is a precondition of the work rather than a finding about it.

## How it works today

`plan start` succeeds. Once the issue is `ACTIVE`, `majordomus intent validate` and the `intent-check` gate report `executing_without_critique` or `executing_with_open_blocker` and exit 10. That half is `intent-plan-reviewed`, and `test/cases/386_a_plan_is_held_to_its_intent.sh` asserts both: the start goes through, the validation fails.

## How to see it

```bash
majordomus plan start I0001   # exit 0
majordomus intent validate    # FAIL executing_without_critique
```

## What it does not cover

Refusing the start would not write the critique, and would not judge whether the critique is any good: a worker still writes it.

## Why it exists

A failure reported after the work started is cheaper than none, and dearer than a refusal before it. The gap between the two is named here so the homepage does not call the start refused.
