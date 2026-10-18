---
schema: adr/v1
id: adr-0197
kind: adr
title: "The backlog is bounded by a declared bound that every session reads at start, and the bound reports rather than refuses"
status: proposed
date: 2026-10-18
tags:
  - integration-release
  - chatgpt-project
related:
  - rule:project.accumulation-is-measured
  - rule:project.blocking-checks-cheap
  - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - file:scripts/ci/backlog-check
  - file:.ai/repo/policy.yaml
  - file:.ai/repo/project/issues/I3204.yaml
  - file:.ai/repo/project/issues/I3212.yaml
  - file:.ai/repo/project/milestones/delivery-flow-is-measured.yaml
  - file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
    - file:scripts/ci/backlog-check
    - file:.ai/repo/policy.yaml
    - file:.ai/repo/project/issues/I3204.yaml
    - file:.ai/repo/project/issues/I3212.yaml
    - file:.ai/repo/project/milestones/delivery-flow-is-measured.yaml
    - file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
---

# 197. The backlog is bounded by a declared bound that every session reads at start, and the bound reports rather than refuses

## Context

Pull requests outran integration repeatedly (52 open on 2026-10-10, 82 on 2026-10-11, 79 on 2026-10-18 by the audits' readings). Proposals of 2026-10-10 and 2026-10-11 asked for admission control that refuses new independent work past a WIP bound. Rule project.accumulation-is-measured asks for a threshold that makes growth loud; rule project.blocking-checks-cheap says work in progress is reported, never blocked. The owner's Operation Convergence (2026-10-17) stopped new scope by decision, not by a gate.

## Decision

The landable backlog and the starving age are declared once in .ai/repo/policy.yaml. The session brief and backlog-check read them, and a session that starts past the bound is told so with the backlog's state. The bound does not refuse a session or a task; whether it ever should is the owner's decision, recorded separately if taken.

## Alternatives rejected

- Refusing new tasks past the bound by default (conflicts with rule project.blocking-checks-cheap; emergency work would need an exemption mechanism)
- A bound per script in environment defaults (today's state)
- No bound, relying on the owner's orders

## Consequences

I3204 moves the bound into policy and the brief; I3212 names duplicate candidates; a later refusal would amend rule project.blocking-checks-cheap explicitly.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c207#6`
- `chatgpt-majordomus:c209#3`
- `chatgpt-majordomus:c227#2`
