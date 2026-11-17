---
schema: adr/v1
id: adr-0187
kind: adr
title: "Tests are selected by the impact of a change, derived from what each case declares it reads, and a change of unknown impact runs every case"
status: proposed
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0026-a-capability-modules-area-is-derived-from-the-features-that-name-it.md
  - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
  - file:scripts/ci-plan
  - file:test/run.sh
  - rule:project.blocking-checks-cheap
  - file:.ai/repo/adrs/0186-a-pull-request-s-required-verdict-is-the-feedback-lane-a-gate-wh.md
  - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/issues/I3238.yaml
  - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/issues/I3240.yaml
  - file:.ai/repo/knowledge/curated/do-less-work-before-doing-it-faster.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0026-a-capability-modules-area-is-derived-from-the-features-that-name-it.md
    - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
    - file:scripts/ci-plan
    - file:test/run.sh
    - file:.ai/repo/adrs/0186-a-pull-request-s-required-verdict-is-the-feedback-lane-a-gate-wh.md
    - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/issues/I3238.yaml
    - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/issues/I3240.yaml
    - file:.ai/repo/knowledge/curated/do-less-work-before-doing-it-faster.md
---

# 187. Tests are selected by the impact of a change, derived from what each case declares it reads, and a change of unknown impact runs every case

## Context

Gates are selected by hand-kept path classes; cases are not selected at all. 431 of 527 cases carry `# majordomus-covers:` (282 'none').

## Decision

A case's reads are declared in its header or derived from the commands and features it covers through the feature-to-module map (ADR 0026). scripts/ci-plan computes the closure from changed paths to cases and lists each selection with its reason. A case that declares nothing is always selected. A mutation case proves that a case whose input changed is never skipped. Historical shard timing balances shards but never selects.

## Alternatives rejected

- Manual giant path lists (c195#2)
- Sharding by historical time as the only lever (c228#3)

## Consequences

Selection starts conservative (282 'none' cases always run) and narrows only as cases declare. The full suite stays on the proof lane.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c130#3`
- `chatgpt-majordomus:c195#2`
- `chatgpt-majordomus:c228#3`
