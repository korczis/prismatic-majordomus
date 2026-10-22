---
schema: knowledge/v1
id: criteria-prove-the-declared-outcome-not-its-value
kind: knowledge
class: constraint
title: "Satisfied criteria prove the declared outcome, not its business value"
description: "When a satisfied intent is presented as product value."
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
    - file:.ai/repo/project/milestones/cost-per-accepted-outcome.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/evidence-machinery-must-end-in-a-shipped-result.md
  - type: relates_to
    target: file:.ai/repo/project/intents/the-product-shows-what-it-caught.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3276.yaml
---

# Satisfied criteria prove the declared outcome, not its business value

Machine-checked criteria show that what was declared holds. Whether that helped a user, saved time or was worth its cost is a separate measurement (cost-per-accepted-outcome, ADR 0082). A page or report must not present a satisfied verdict as evidence of value.

## Sources

- `chatgpt-majordomus:c198#1`
