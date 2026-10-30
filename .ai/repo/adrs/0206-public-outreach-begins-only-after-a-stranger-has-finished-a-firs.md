---
schema: adr/v1
id: adr-0206
kind: adr
title: "Public outreach begins only after a stranger has finished a first task unaided"
status: proposed
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
related:
  - file:.ai/repo/project/milestones/a-second-adopter-is-served.yaml
  - file:.ai/repo/adrs/0169-what-an-adoption-leaves-behind-is-measured-not-remembered.md
  - file:docs/ADOPTION.md
  - file:.ai/repo/project/issues/I3127.yaml
  - file:.ai/repo/project/issues/I3128.yaml
  - file:.ai/repo/project/issues/I3129.yaml
  - file:.ai/repo/project/milestones/a-stranger-adopts-it-unaided.yaml
  - file:.ai/repo/project/intents/a-stranger-adopts-it-unaided.yaml
  - file:.ai/repo/knowledge/curated/public-writing-is-evidence-led.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/project/milestones/a-second-adopter-is-served.yaml
    - file:.ai/repo/adrs/0169-what-an-adoption-leaves-behind-is-measured-not-remembered.md
    - file:docs/ADOPTION.md
    - file:.ai/repo/project/issues/I3127.yaml
    - file:.ai/repo/project/issues/I3128.yaml
    - file:.ai/repo/project/issues/I3129.yaml
    - file:.ai/repo/project/milestones/a-stranger-adopts-it-unaided.yaml
    - file:.ai/repo/project/intents/a-stranger-adopts-it-unaided.yaml
    - file:.ai/repo/knowledge/curated/public-writing-is-evidence-led.md
---

# 206. Public outreach begins only after a stranger has finished a first task unaided

## Context

On 2026-10-12 the owner asked whether the project was ready for LinkedIn and early adopters. The answer found no independent evidence that a foreign developer can understand, install and keep using it, and the owner ordered a beta first. The definition given was narrow: a reliable, repeatable product a stranger uses without the maintainer, not the whole roadmap. Since then the alpha (v1.0.0-alpha.1) was set for 2026-10-19 under Operation Convergence, but at 707a04c7ce8 neither decision is a tracked intent or ADR on master.

## Decision

No public invitation to try Majordomus is made while intent a-stranger-adopts-it-unaided is unsatisfied. Readiness is the intent's derived verdict, not a feeling or a backlog count; what counts as a stranger's run is defined by the outside-first-run workflow; a release label (alpha, beta) does not by itself permit outreach. A private, consented invitation of named individual testers to make an outside first run is not public outreach and is permitted; only a public invitation waits on the verdict.

## Alternatives rejected

- Immediate launch on the feature inventory
- Waiting for the whole roadmap, the mesh and the intent engine
- Driving the backlog to zero as the readiness measure

## Consequences

The owner keeps the decision of when and where to announce; the repository supplies a verdict. Outreach work (posts, communities) waits; adoption work (time-to-first-refusal, init findings, outside runs) is prioritised after the alpha.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c241#3`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c241#0`
- `chatgpt-majordomus:c241#3`
- `chatgpt-majordomus:c241#4`
- `chatgpt-majordomus:c137#2`
