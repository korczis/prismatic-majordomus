+++
title = "Majordomus reasons, decides, implements and validates with no advisor at all: absent advisors are reported as ordinary state, material uncertainty gets a structured local review, and the conclusion claims no independent review"
description = "With none of the declared advisors installed, configured or reachable, a session still"
weight = 227
[extra]
claim_id = "reasoning-works-with-no-advisor"
status = "guaranteed"
source = "docs/claims/reasoning-works-with-no-advisor.md"
+++
{% raw %}

## What it means

With none of the declared advisors installed, configured or reachable, a session still
runs the whole workflow: it starts a task, states a material uncertainty with its evidence,
plans, concludes on local evidence, implements, validates and hands over. Every advisor is
reported with why it is absent (`executable_not_found`, `credential_absent`), reasoning
reports itself operational, and the conclusion's independent review count is zero rather
than a review that never happened.

## How it works

Availability is a pure function of presence (an executable on `PATH`, a credential
variable set), the reasoning mode and recorded outcomes
(`apps/majordomus-cli/src/reasoning/availability.rs`). The policy
(`apps/majordomus-cli/src/reasoning/policy.rs`) treats zero available advisors as a plan:
`decide_locally`, with the structured local review spelled out. The transport
(`scripts/advisor-consult`) consults nobody and exits 0. The writer computes the review
count from the consultations a conclusion cites, so a local conclusion counts zero.

## How to see it

```
env -i PATH=/usr/bin:/bin majordomus reasoning advisors   # 0 available · N optional unavailable
majordomus reasoning plan --materiality high              # outcome decide_locally, local review steps
```

`test/cases/730_reasoning_works_with_no_advisor.sh` runs the workflow end to end, through
the derived handover, and in the `ci` and `offline` modes.

## What it does not cover

It does not make local reasoning as good as review; it makes its absence explicit. It
does not check that the executor actually performed the local review steps — the record
says what was decided and on what evidence, and the evidence is what a reviewer reads.

## Why it exists

Optional providers are optional only if the work survives their absence. A workflow
that merely avoids crashing without an advisor, then asks a person what to do, is not
provider-independent (ADR 0098).
{% endraw %}
