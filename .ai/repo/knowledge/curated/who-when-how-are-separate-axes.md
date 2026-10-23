---
schema: knowledge/v1
id: who-when-how-are-separate-axes
kind: knowledge
class: convention
title: "The mesh decides who, the wave decides when, the profile decides how"
description: "When designing assignment or execution policy."
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
    - file:.ai/repo/profiles/README.md
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - type: documents
    target: file:.ai/repo/project/issues/I3199.yaml
  - type: relates_to
    target: file:.ai/repo/project/milestones/mesh-peers-execute-assigned-work.yaml
---

# The mesh decides who, the wave decides when, the profile decides how

Who works is a mesh peer holding a claim; when is the execution wave, a topological level derived from the dependency graph; how is the execution profile (.ai/repo/profiles); what is the issue or intent; why is context plus ADRs and rules; proof is evidence. Keeping these axes separate avoids a combined mode enum and lets each be derived from its own source. Proposed by the assistant on 2026-09-20; not an owner decision.

## Sources

- `chatgpt-majordomus:c160#3`
