---
schema: knowledge/v1
id: a-wiring-check-proves-a-string-not-an-effect
kind: knowledge
class: lesson
title: "A wiring verifier that greps for a dispatch string proves the string exists, not that the rule refuses anything"
description: "When judging whether a rule is enforced"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:lib/doctor.sh
    - file:test/cases/18_doctrine_wiring.sh
    - file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
  - type: documents
    target: file:.ai/repo/adrs/0188-a-rule-s-proof-of-effect-is-a-recorded-refusal-of-its-violation.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3111.yaml
---

# A wiring verifier that greps for a dispatch string proves the string exists, not that the rule refuses anything

The independent audit of 2026-10-08 found that doctor's doctrine-wiring proof checked that a validator exists, that a dispatch string is present, that an exit appears anywhere (including in comments), and that a named test file exists, without reading it. A mutation to "|| exit 0" still passed. The mutations in the wiring case had been tuned to the heuristic. Proof of effect means running the rule against a violation through the real entry point and seeing it refused.

## Sources

- `chatgpt-majordomus:c193#0`
