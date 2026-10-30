---
schema: adr/v1
id: adr-0195
kind: adr
title: "What accepting an issue unblocks is answered by one counterfactual evaluator over the plan graph"
status: proposed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
  - file:apps/majordomus-cli/src/plan.rs
  - file:.ai/repo/project/issues/I3267.yaml
  - file:.ai/repo/project/issues/I3268.yaml
  - file:.ai/repo/project/issues/I3269.yaml
  - file:.ai/repo/project/milestones/the-plan-says-what-a-fix-unblocks.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
    - file:apps/majordomus-cli/src/plan.rs
    - file:.ai/repo/project/issues/I3267.yaml
    - file:.ai/repo/project/issues/I3268.yaml
    - file:.ai/repo/project/issues/I3269.yaml
    - file:.ai/repo/project/milestones/the-plan-says-what-a-fix-unblocks.yaml
---

# 195. What accepting an issue unblocks is answered by one counterfactual evaluator over the plan graph

## Context

The owner ordered an impact view (2026-10-17) and /proceed ranks by unlock (ADR 0177 (on an unlanded branch), I2973). Two counts would drift.

## Decision

plan.impact applies plan.rs's readiness and milestone rules to a copy of the plan with hypothetical acceptances and returns immediate unlocks, transitive reach and remaining blockers; every consumer (proceed, /ui, the site) reads it.

## Alternatives rejected

- Counting downstream dependants
- A frontend dependency inventory
- A second graph engine or library

## Consequences

I2973 consumes plan.impact rather than its own count; the site page is generated.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c284#5`
