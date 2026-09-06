+++
title = "A public page promising something nothing tests"
description = "Public material is written by a different act from the code, so it drifts ahead of the behaviour and nothing brings it back."
weight = 330
[extra]
id = "site-claims-nothing-proves"
status = "stable"
source = ".ai/repo/why/moments/site-claims-nothing-proves.md"
+++
{% raw %}

## The moment

The page says the system does something. It nearly does. The gap is the kind that a
knowledgeable reader would call a difference of emphasis and a user would call a bug, and
it exists because the page was written from the design and the code was written from the
constraints.

## Why it happens

Public material is written once, early, when the intended behaviour is the only behaviour
there is. Shipping narrows the design; the narrowing is recorded in commits and tests, and
in nothing that anyone reconciles against the page. Nothing on the page marks which
sentences are load-bearing.

## Why a better model does not fix it

A worker asked to write the page from the design writes an accurate description of the
design. The failure is that the page is not derived from anything that changes when the
behaviour changes.

## What it costs

Trust, which is expensive to regain and is spent by exactly the readers who mattered most —
the ones who relied on the sentence. Internally it costs the same as any stale document,
plus an argument about whether the page was wrong or the implementation was.

## What Majordomus does

Every capability sentence is a claim object with a status — guaranteed, advisory, planned or
rejected — the file that implements it and the behavioural case that proves it. The public
pages render those objects rather than restating them, so a claim cannot appear on a page
with a status it does not have. A guaranteed claim without a real implementation and a real
test fails the check. Rejected is a published status with its reasoning, so an idea that was
considered and refused stays refused instead of being proposed again.

## Before and after

```text
before   page: "Majordomus verifies completion."      code: it verifies what you name.

after    claim finish-contract   guaranteed  lib/finish.sh  test/cases/06_finish.sh
         claim cost-per-outcome  planned     -              -
         (the page renders the status; a guaranteed row with '-' fails the build)
```

## What it does not do

It does not review the wording of a page, and a badly worded true claim is still badly
worded. It refuses the pairing of a strong status with no evidence.
{% endraw %}
