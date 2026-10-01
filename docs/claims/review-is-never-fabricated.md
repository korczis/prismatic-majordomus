# A session cannot claim review it did not get: a consultation must name an advisor its recorded plan selected, a conclusion's review is computed, and reasoning check and doctor refuse a forged record, provider coupling, a model credential in CI and a refused claim in a document

## What it means

"Reviewed by" is a fact about records, never a sentence someone wrote. The design's other
promises — provider-independent code, no live model in CI, no claim that consensus decides
— are checked, not trusted.

## How it works

The writer admits a consultation only for an advisor the plan's own snapshot selected,
and computes `reviewed_by`. `reasoning check` (`apps/majordomus-cli/src/reasoning/check.rs`)
re-reads every stored record — including ones written past the writer — and also checks the
catalogue's references, the adapters, the provider-independent sources, CI workflows and
gates, and the documents. `doctor` fails on its findings and reports absent advisors as
information only.

## How to see it

```
majordomus reasoning check     # exit 10 on a finding
majordomus doctor              # FAIL reasoning … / INFO advisor … — optional
```

`test/cases/736_reasoning_check_refuses_drift.sh` plants each defect and expects it named.

## What it does not cover

It cannot stop a person from deleting records; it can only refuse records that claim
more than they cite. Claim drift is matched against declared phrases, not understood.

## Why it exists

A review count anyone can type is a review anyone can invent, and an unchecked rule is
decoration (ADR 0098).
