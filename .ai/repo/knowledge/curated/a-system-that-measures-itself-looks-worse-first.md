---
schema: knowledge/v1
id: a-system-that-measures-itself-looks-worse-first
kind: knowledge
class: lesson
title: "A system that starts measuring itself looks worse before it looks better"
description: "When a new gate or drift check raises the finding count."
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
    - file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3112.yaml
---

# A system that starts measuring itself looks worse before it looks better

New mechanisms that expose existing defects raise counts in the short term (drift findings, blocked issues, red gates). That is healthy only if the counts then fall; a rising count after the first window is the signal, not the first jump. Read a new finding class as an observation of old state.

## Sources

- `chatgpt-majordomus:c272#5`
