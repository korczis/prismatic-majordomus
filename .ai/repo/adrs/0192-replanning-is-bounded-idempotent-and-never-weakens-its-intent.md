---
schema: adr/v1
id: adr-0192
kind: adr
title: "Replanning is bounded, idempotent and never weakens its intent"
status: proposed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0053-an-identity-is-allocated-above-the-high-water-mark-and-a-collision-is-refused-on-the-branch.md
  - rule:project.every-wait-is-bounded
  - file:.ai/repo/policy.yaml
  - file:.ai/repo/adrs/0191-a-plan-drafted-from-an-intent-is-a-proposal-checked-by-the-plan.md
  - file:.ai/repo/project/issues/I3247.yaml
  - file:.ai/repo/project/issues/I3246.yaml
  - file:.ai/repo/project/issues/I3248.yaml
  - file:.ai/repo/project/issues/I3249.yaml
  - file:.ai/repo/project/issues/I3250.yaml
  - file:.ai/repo/knowledge/curated/what-idd-is-not.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0053-an-identity-is-allocated-above-the-high-water-mark-and-a-collision-is-refused-on-the-branch.md
    - file:.ai/repo/policy.yaml
    - file:.ai/repo/adrs/0191-a-plan-drafted-from-an-intent-is-a-proposal-checked-by-the-plan.md
    - file:.ai/repo/project/issues/I3247.yaml
    - file:.ai/repo/project/issues/I3246.yaml
    - file:.ai/repo/project/issues/I3248.yaml
    - file:.ai/repo/project/issues/I3249.yaml
    - file:.ai/repo/project/issues/I3250.yaml
    - file:.ai/repo/knowledge/curated/what-idd-is-not.md
---

# 192. Replanning is bounded, idempotent and never weakens its intent

## Context

Re-evaluating an intent must not file a new issue each time, loop forever, or quietly lower the bar (2026-10-08, 10-11, 10-16).

## Decision

A drafted work item's identity is derived from (intent, criterion, gap kind); a replan reuses done work and supersedes through existing transitions; a policy-declared budget of drafts without a newly satisfied criterion ends in human_needed; a draft narrowing a criterion is refused without a recorded decision.

## Alternatives rejected

- Work until satisfied
- An LLM noticing the same gap and filing a new issue
- Rewriting history on replan

## Consequences

One budget field in policy.yaml; no runtime or scheduler; /proceed (ADR 0177 (on an unlanded branch)) inherits the bound.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c196#3`
- `chatgpt-majordomus:c196#6`
- `chatgpt-majordomus:c205#2`
- `chatgpt-majordomus:c221#2`
- `chatgpt-majordomus:c270#8`
