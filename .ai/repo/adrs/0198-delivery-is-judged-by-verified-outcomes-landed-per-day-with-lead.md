---
schema: adr/v1
id: adr-0198
kind: adr
title: "Delivery is judged by verified outcomes landed per day, with lead time p95 as its control, never by commits, pull requests or sessions"
status: proposed
date: 2026-10-18
tags:
  - integration-release
  - chatgpt-project
related:
  - rule:project.no-counts-in-prose
  - file:.ai/repo/adrs/0071-a-feature-exists-only-when-delivery-is-computed-and-every-dimension-passes.md
  - file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
  - file:.ai/repo/knowledge/curated/evidence-machinery-must-end-in-a-shipped-result.md
  - file:.ai/repo/project/issues/I3203.yaml
  - file:.ai/repo/project/milestones/delivery-flow-is-measured.yaml
  - file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
  - file:.ai/repo/project/issues/I3276.yaml
  - file:.ai/repo/adrs/0196-delivery-latency-targets-are-budgets-of-benchmark-policy-measure.md
  - file:.ai/repo/knowledge/curated/integration-capacity-bounds-the-backlog.md
  - file:.ai/repo/knowledge/curated/a-partial-green-is-not-the-verdict.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0071-a-feature-exists-only-when-delivery-is-computed-and-every-dimension-passes.md
    - file:.ai/repo/adrs/0166-the-plan-surfaces-are-inferred-from-delivery.md
    - file:.ai/repo/knowledge/curated/evidence-machinery-must-end-in-a-shipped-result.md
    - file:.ai/repo/project/issues/I3203.yaml
    - file:.ai/repo/project/milestones/delivery-flow-is-measured.yaml
    - file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
    - file:.ai/repo/project/issues/I3276.yaml
    - file:.ai/repo/adrs/0196-delivery-latency-targets-are-budgets-of-benchmark-policy-measure.md
    - file:.ai/repo/knowledge/curated/integration-capacity-bounds-the-backlog.md
    - file:.ai/repo/knowledge/curated/a-partial-green-is-not-the-verdict.md
---

# 198. Delivery is judged by verified outcomes landed per day, with lead time p95 as its control, never by commits, pull requests or sessions

## Context

Audits of 2026-10-14..18 found that 52 percent of the last 100 commits were merges, that four agents opening four pull requests did not do four times the work, and proposed several primary metrics: verified deliveries merged per 24 hours, lead time p95, autonomous intents completed, time from finished intent to user-available function without regression. The milestone cost-per-accepted-outcome measures cost only on measured data.

## Decision

The repository's delivery figure is the count of verified outcomes (an issue DONE with its evidence, or a release published and observed) per rolling day, read with the per-phase lead time p95 of I3203 as its control. Commit, pull request and session counts are activity, may be shown as such, and are never called throughput.

## Alternatives rejected

- Commits or merged pull requests per day as throughput
- One composite score (opaque, unfalsifiable)
- Forecast maturity percentages

## Consequences

No issue in this plan implements the figure yet: I3203 measures lead time only, and an issue that counts verified outcomes is planned once the owner accepts the definition. The drain and dashboard label activity as activity; an outcome without evidence does not count.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c259#6`
- `chatgpt-majordomus:c271#9`
- `chatgpt-majordomus:c287#2`
- `chatgpt-majordomus:c295#7`
- `chatgpt-majordomus:c295#9`
- `chatgpt-majordomus:c297#6`
