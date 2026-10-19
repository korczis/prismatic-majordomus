---
schema: knowledge/v1
id: a-brief-is-reconciled-not-transcribed
kind: knowledge
class: convention
title: "A brief or an audit becomes outcome-shaped records after discovery, never one issue per bullet"
description: "When turning a prompt pack, an audit or a conversation into plan records."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
  - type: documents
    target: file:.ai/repo/adrs/0194-an-ingested-brief-is-an-intake-revision-and-its-requirements-are.md
  - type: documents
    target: file:.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3273.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3254.yaml
---

# A brief or an audit becomes outcome-shaped records after discovery, never one issue per bullet

Each requirement is first classified against the repository as done, partial, drifted, missing or obsolete; root causes and missing outcomes are ticketed; an audit becomes one intent with invariants, non-goals and measurable criteria. Generic acceptance blocks repeated across every issue let an agent satisfy the letter and miss the point: each issue needs its own input, output, invariants and failure semantics. Accepted by the owner on 2026-09-12.

## Sources

- `chatgpt-majordomus:c126#3`
- `chatgpt-majordomus:c193#1`
- `chatgpt-majordomus:c198#11`
