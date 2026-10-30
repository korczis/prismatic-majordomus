---
schema: adr/v1
id: adr-0207
kind: adr
title: "A new mechanism is admitted only with the measured problem it removes"
status: proposed
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
related:
  - rule:project.no-new-nouns
  - rule:project.optional-complexity
  - rule:project.derived-once
  - file:.ai/repo/project/critiques/README.md
  - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/project/critiques/README.md
    - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
---

# 207. A new mechanism is admitted only with the measured problem it removes

## Context

Self-assessments on 2026-09-11, 10-02, 10-08, 10-13 and 10-16 found the same pattern: a local problem answered by a registry, doctrine, schema, API, MCP tool, Cockpit panel, gate and ADR at once. Rules project.no-new-nouns and project.optional-complexity bound the vocabulary and the ordinary path, but neither asks a new subsystem what it removes, so the question is argued per review.

## Decision

A milestone or issue that adds a subsystem, capability family, record kind or surface names, in its why, the measured problem it removes: a manual step, a regression class, context or token cost, or a lower probability of reaching a verified merge, with the evidence of that problem. A proposal without one is recorded as an open question, not planned.

## Alternatives rejected

- A permanent freeze
- Leaving it to reviewer judgement case by case

## Consequences

The critique of an intent gains one question. Lumen, notifications, federation and similar proposals start from their measured problem. The rule is advisory until a plan validator can read the why for a cited measurement.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c119#3`
- `chatgpt-majordomus:c277#1`
- `chatgpt-majordomus:c256#0`
- `chatgpt-majordomus:c256#2`
- `chatgpt-majordomus:c269#2`
