---
schema: adr/v1
id: adr-0208
kind: adr
title: "The product's value is stated as the false dones its finish caught, beside the friction it cost"
status: proposed
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
related:
  - file:share/events.yaml
  - file:lib/finish.sh
  - file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
  - file:.ai/repo/project/milestones/cost-per-accepted-outcome.yaml
  - file:docs/ECONOMICS.md
  - file:.ai/repo/project/milestones/the-product-shows-what-it-caught.yaml
  - file:.ai/repo/project/issues/I3130.yaml
  - file:.ai/repo/project/issues/I3131.yaml
  - file:.ai/repo/project/issues/I3132.yaml
  - file:.ai/repo/project/issues/I3133.yaml
  - file:.ai/repo/project/intents/the-product-shows-what-it-caught.yaml
  - file:.ai/repo/project/issues/I3126.yaml
  - file:.ai/repo/knowledge/curated/criteria-prove-the-declared-outcome-not-its-value.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:share/events.yaml
    - file:lib/finish.sh
    - file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
    - file:.ai/repo/project/milestones/cost-per-accepted-outcome.yaml
    - file:docs/ECONOMICS.md
    - file:.ai/repo/project/milestones/the-product-shows-what-it-caught.yaml
    - file:.ai/repo/project/issues/I3130.yaml
    - file:.ai/repo/project/issues/I3131.yaml
    - file:.ai/repo/project/issues/I3132.yaml
    - file:.ai/repo/project/issues/I3133.yaml
    - file:.ai/repo/project/intents/the-product-shows-what-it-caught.yaml
    - file:.ai/repo/project/issues/I3126.yaml
    - file:.ai/repo/knowledge/curated/criteria-prove-the-declared-outcome-not-its-value.md
---

# 208. The product's value is stated as the false dones its finish caught, beside the friction it cost

## Context

From 2026-09-11 the owner repeatedly asked whether Majordomus is worth the time and asked for metrics. Assessments proposed many ROI measures (time-to-context, interventions, failed handovers, regressions prevented). Most need telemetry that does not exist (execution-telemetry is blocked). One needs nothing new: task.refused was added so that a refusal later satisfied counts as a caught false done.

## Decision

The primary product metric is the number of tasks a worker called finished that finish refused and later accepted after a change, shown with its denominator (accepted tasks), beside the friction metrics: unanswered refusals, time from first refusal to acceptance, and on the benchmark the false-done rate with and without the finish contract. Feature counts, session counts and event counts are never product metrics.

## Alternatives rejected

- Feature completeness as the definition of done
- Event or session counts as activity metrics
- Modelled savings (counterfactual class)

## Consequences

I3130 to I3133 implement it; a figure is per checkout until a tracked projection exists, and says so.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c119#7`
- `chatgpt-majordomus:c277#1`
- `chatgpt-majordomus:c256#2`
