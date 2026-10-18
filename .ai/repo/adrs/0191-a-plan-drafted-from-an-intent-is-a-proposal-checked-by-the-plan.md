---
schema: adr/v1
id: adr-0191
kind: adr
title: "A plan drafted from an intent is a proposal, checked by the plan's own validators before any record is written"
status: proposed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0144-an-intent-is-proposed-by-a-capability-and-its-stage-stays-derived.md
  - file:.ai/repo/adrs/0073-an-issue-names-the-intent-criterion-it-serves-and-coverage-is-derived.md
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:apps/majordomus-cli/src/intent_review.rs
  - file:apps/majordomus-cli/src/intent_plan.rs
  - rule:project.no-new-nouns
  - file:.ai/repo/project/issues/I3251.yaml
  - file:.ai/repo/project/issues/I3252.yaml
  - file:.ai/repo/project/issues/I3253.yaml
  - file:.ai/repo/project/issues/I3254.yaml
  - file:.ai/repo/project/issues/I3246.yaml
  - file:.ai/repo/project/issues/I3247.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0144-an-intent-is-proposed-by-a-capability-and-its-stage-stays-derived.md
    - file:.ai/repo/adrs/0073-an-issue-names-the-intent-criterion-it-serves-and-coverage-is-derived.md
    - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
    - file:apps/majordomus-cli/src/intent_review.rs
    - file:apps/majordomus-cli/src/intent_plan.rs
    - file:.ai/repo/project/issues/I3251.yaml
    - file:.ai/repo/project/issues/I3252.yaml
    - file:.ai/repo/project/issues/I3253.yaml
    - file:.ai/repo/project/issues/I3254.yaml
    - file:.ai/repo/project/issues/I3246.yaml
    - file:.ai/repo/project/issues/I3247.yaml
---

# 191. A plan drafted from an intent is a proposal, checked by the plan's own validators before any record is written

## Context

intent-planning's invariant says Majordomus judges records and does not generate a plan; ADR 0144 lets intents.propose write only a bare intent. The owner asked repeatedly (2026-10-04, 10-09, 10-11, 10-16) that the gap and the plan be derived. The drafting design was planned on 2026-10-10 (owner packs) and never landed.

## Decision

Majordomus drafts gaps and plans read-only from the intent, the evidence and the plan; a draft is judged in memory by the same plan and intent validators; only a checked draft accepted by a person (or by a policy the owner declares) is written, as one repository_mutation capability. A model may author a draft; it never decides.

## Alternatives rejected

- Keep generation a non-goal and hand-write every plan
- Let an agent write and execute its own plan
- A separate planner or plan store

## Consequences

intent-planning's non-goal changes; ADR 0144 is extended (propose stays bare); drafting gets an identity scheme shared with replanning.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c146#5`
- `chatgpt-majordomus:c185#4`
- `chatgpt-majordomus:c205#2`
