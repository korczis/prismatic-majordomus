+++
title = "Advisors that disagree never decide by count: a conclusion is refused while a disagreement on its assessment is unsettled, a resolution needs evidence, and every answer received must be weighed"
description = "When advice diverges, the divergence is a record — the positions, the assumptions they"
weight = 234
[extra]
claim_id = "advice-is-weighed-not-counted"
status = "guaranteed"
source = "docs/claims/advice-is-weighed-not-counted.md"
+++
{% raw %}

## What it means

When advice diverges, the divergence is a record — the positions, the assumptions they
divide on, the question that decides — and the decision waits for evidence. Two advisors
agreeing is evidence about agreement; it settles nothing on its own.

## How it works

The writer (`apps/majordomus-cli/src/reasoning/store.rs`) refuses a conclusion on an
assessment with an unresolved disagreement, refuses a resolution that cites no evidence,
and refuses a conclusion that leaves out an answer its assessment received (it may reject
the answer in `rejected`, not ignore it). The review count is computed from the cited
consultations; there is no field to author it.

## How to see it

```
majordomus reasoning record --file disagreement.json
majordomus reasoning explain <conclusion-id>   # the experiment is in the chain
```

`test/cases/732_disagreement_is_settled_by_evidence.sh` consults two scripted advisors that
disagree, is refused while two of three positions agree, and concludes the minority's
position after an experiment.

## What it does not cover

It cannot judge whether an experiment was a good one. It makes the experiment and its
evidence part of the record, where a reviewer can judge it.

## Why it exists

Majority vote over models is the failure mode of naive multi-model review: it rewards
correlated errors. Making the count unable to decide anything is the repair (ADR 0098).
{% endraw %}
