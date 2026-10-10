+++
title = "Independent review is a recorded fact, weighed by evidence, never claimed and never counted"
description = "Independent review is a recorded fact, weighed by evidence, never claimed and never counted"
weight = 136
[extra]
kind = "rule"
slug = "project-review-is-recorded-not-claimed-1"
identity = "project.review-is-recorded-not-claimed@1"
status = "active"
source = ".ai/repo/rules/project/review-is-recorded-not-claimed.v1.md"
+++
{% raw %}

## Rationale

A review that cannot be traced to the exchange that produced it is indistinguishable from
one that never happened. And advisors that agree are evidence that they agree: models share
training data and failure modes, so a count of agreeing answers measures correlation, not
correctness. In the session that built ADR 0098, the one advisor that answered opposed the
executor's hypothesis. An experiment showed it was wrong, and the conclusion records both
the advice and why it was rejected.

## Required behaviour

1. **Evidence before opinion.** A material assessment carries the evidence gathered before
   anyone is asked; the question an advisor receives carries that evidence, and marks the
   executor's hypothesis and any earlier advice as opinions.
2. **Only planned review is review.** A consultation names an advisor the recorded plan
   selected. A failed consultation is recorded and is not review.
3. **Weigh every answer.** A conclusion cites every answer its assessment received and
   rejects in `rejected` what it does not follow.
4. **No count decides.** A conclusion waits for every disagreement on its assessment to be
   resolved by an experiment that cites evidence.
5. **Computed, not authored.** `reviewed_by` and `independent_review_count` are the
   writer's; no record field lets a caller assert them.
6. **Survives the session.** The decision, its evidence and its review reach the next
   session through context and handover; the same question is reopened only by new
   evidence.

## Failure behaviour

The writer refuses each violation when the record is made. `majordomus reasoning check`
exits 10 on a stored record that claims more than it cites, and `doctor` fails on it.
{% endraw %}
