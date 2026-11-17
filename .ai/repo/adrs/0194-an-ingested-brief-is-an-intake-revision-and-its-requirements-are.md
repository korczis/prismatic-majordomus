---
schema: adr/v1
id: adr-0194
kind: adr
title: "An ingested brief is an intake revision, and its requirements are reconciled into the existing plan"
status: proposed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
  - file:.ai/repo/adrs/0009-prompt-capture-happens-below-the-model.md
  - rule:project.a-brief-is-kept-as-received
  - file:.ai/repo/adrs/0180-a-prompt-pack-is-a-campaign-kept-as-received-indexed-as-a-kind-a.md
  - file:.ai/repo/project/issues/I3272.yaml
  - file:.ai/repo/project/issues/I3273.yaml
  - file:.ai/repo/project/issues/I3274.yaml
  - file:.ai/repo/project/issues/I3275.yaml
  - file:.ai/repo/project/milestones/intent-intake.yaml
  - file:.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
  - file:.ai/repo/adrs/0191-a-plan-drafted-from-an-intent-is-a-proposal-checked-by-the-plan.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
    - file:.ai/repo/adrs/0009-prompt-capture-happens-below-the-model.md
    - file:.ai/repo/adrs/0180-a-prompt-pack-is-a-campaign-kept-as-received-indexed-as-a-kind-a.md
    - file:.ai/repo/project/issues/I3272.yaml
    - file:.ai/repo/project/issues/I3273.yaml
    - file:.ai/repo/project/issues/I3274.yaml
    - file:.ai/repo/project/issues/I3275.yaml
    - file:.ai/repo/project/milestones/intent-intake.yaml
    - file:.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
    - file:.ai/repo/adrs/0191-a-plan-drafted-from-an-intent-is-a-proposal-checked-by-the-plan.md
---

# 194. An ingested brief is an intake revision, and its requirements are reconciled into the existing plan

## Context

ADR 0124 requires a follow-up ADR choosing the noun before any ingestion code. Owner standing orders (2026-10-10, 10-12, 10-13) ask that packs, uploads and conversations become tracked, reconciled work; 2026-10-12 separated capture (frequent, cheap) from execution (the governance boundary).

## Decision

The noun is `intake`. An intake revision is digest-pinned and immutable; its requirements are addressed and reconciled into drafts on the drafting path; capture never executes; content is untrusted; autonomy per work class, if ever, is a policy field, not a document's claim.

## Alternatives rejected

- `pack` (reserved)
- `artifact` (overloaded by CI and release artifacts)
- Pack-scoped milestones and issues

## Consequences

One record kind and schema; intake depends on drafting; this ChatGPT import is its reference case.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c239#4`
- `chatgpt-majordomus:c245#11`
