---
id: project.review-is-recorded-not-claimed
version: 1
kind: rule
title: Independent review is a recorded fact, weighed by evidence, never claimed and never counted
description: A session states that an advisor reviewed a decision only through records the writer admitted — a consultation of an advisor its plan selected — and the review count of a conclusion is computed from them. Advice is evidence the executor weighs; a disagreement is settled by an experiment, never by the number of advisors on each side.
statement: Gather evidence before asking, record every exchange, settle disagreement with evidence, weigh every answer received, and let the writer compute who reviewed a decision.
status: active
class: blocking
depends_on: [project.advisors-are-optional@1]
tags: [reasoning, advisors, evidence, sessions]

x-majordomus:
  tests: [test/cases/730_reasoning_works_with_no_advisor.sh, test/cases/732_disagreement_is_settled_by_evidence.sh, test/cases/734_reasoning_survives_the_session.sh, test/cases/736_reasoning_check_refuses_drift.sh]
---

# Rationale

A review that cannot be traced to the exchange that produced it is indistinguishable from
one that never happened. And advisors that agree are evidence that they agree: models share
training data and failure modes, so a count of agreeing answers measures correlation, not
correctness. In the session that built ADR 0098, the one advisor that answered opposed the
executor's hypothesis. An experiment showed it was wrong, and the conclusion records both
the advice and why it was rejected.

# Required behaviour

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

# Failure behaviour

The writer refuses each violation when the record is made. `majordomus reasoning check`
exits 10 on a stored record that claims more than it cites, and `doctor` fails on it.
