+++
title = "Once work exists under an intent, a criterion no issue serves, an issue under its milestones that serves nothing and a link to a criterion that does not exist are each refused by name"
description = "An issue names the criterion it serves in serves (ADR 0073). From those links the coverage of every criterion is derived, and every issue under an intent's milestones answers why it exists. Once an intent has work, a criterion nothing serves is criterion_uncovered, an issue that serves nothing is issue_without_purpose, and a link to an unknown intent or criterion is refused."
weight = 112
[extra]
claim_id = "intent-criteria-covered"
status = "guaranteed"
source = "docs/claims/intent-criteria-covered.md"
+++
{% raw %}

## What it means

An issue names the criterion it serves in `serves` (ADR 0073). From those links the coverage of every criterion is derived, and every issue under an intent's milestones answers why it exists. Once an intent has work, a criterion nothing serves is `criterion_uncovered`, an issue that serves nothing is `issue_without_purpose`, and a link to an unknown intent or criterion is refused.

## How it works

`apps/majordomus-cli/src/intent_plan.rs` reads `serves` from the plan and judges it; the plan itself derives no finding from it. A criterion is `covered`, `weak`, `observed` or `uncovered`, and an issue's purpose is `intent`, `maintenance` or `unexplained`. `intent validate` and the `intent-check` gate carry the failures.

## How to see it

```bash
majordomus intent coverage   # each criterion, its strength and the issues serving it
majordomus intent validate   # exit 10 on criterion_uncovered, issue_without_purpose, serves_unknown_*
```

## What it does not cover

An intent with no work yet is `intent_not_planned`, a warning, because that is where every intent starts. Nothing derives the issues from the criteria: a worker writes them.

## Why it exists

A milestone can be finished while a criterion of the intent above it was never planned at all. Coverage makes that omission a named failure before the work closes, rather than a surprise when the intent never becomes satisfied.
{% endraw %}
