---
schema: adr/v1
id: adr-0180
kind: adr
title: "A prompt pack is a campaign: kept as received, indexed as a kind, and cited by the records it drove"
status: proposed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
  - rule:project.a-brief-is-kept-as-received
  - rule:project.no-new-nouns
  - file:campaigns/README.md
  - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
  - file:.ai/repo/project/issues/I3167.yaml
  - file:.ai/repo/project/issues/I3168.yaml
  - file:.ai/repo/project/issues/I3169.yaml
  - file:.ai/repo/project/issues/I3170.yaml
  - file:.ai/repo/project/issues/I3272.yaml
  - file:.ai/repo/project/milestones/campaigns-are-read-as-provenance.yaml
  - file:.ai/repo/project/intents/campaigns-are-provenance-the-plan-reads.yaml
  - file:.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
    - file:campaigns/README.md
    - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
    - file:.ai/repo/project/issues/I3167.yaml
    - file:.ai/repo/project/issues/I3168.yaml
    - file:.ai/repo/project/issues/I3169.yaml
    - file:.ai/repo/project/issues/I3170.yaml
    - file:.ai/repo/project/issues/I3272.yaml
    - file:.ai/repo/project/milestones/campaigns-are-read-as-provenance.yaml
    - file:.ai/repo/project/intents/campaigns-are-provenance-the-plan-reads.yaml
    - file:.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
---

# 180. A prompt pack is a campaign: kept as received, indexed as a kind, and cited by the records it drove

## Context

ADR 0124 (proposed) split an uploaded pack's revision from its execution runs, reused the intent/milestone/issue graph and reserved 'pack' for the text-shard tool, but required a follow-up ADR to choose the ingestion noun (candidates 'artifact' or 'intake'). 'artifact' already names the generated artifacts (majordomus artifacts). The repository already has the noun: campaigns/ keeps 52 operator prompt packs (707a04c) verbatim under rule project.a-brief-is-kept-as-received.

## Decision

The ingestion noun is 'campaign'. A pack is a campaign kept verbatim under campaigns/ with its manifest digest as its revision; campaign becomes a kind of the index; intents, milestones and issues may cite campaigns and the reverse relation is derived; one command adds a pack after archive-safety checks; an execution started from a campaign pins its digest. An outgoing prompt for a worker is a projection of the plan (devtask), never a campaign.

## Alternatives rejected

- 'artifact' (collides with generated artifacts)
- 'intake' (a new noun for an existing thing, rule project.no-new-nouns)
- Prompt packs as hand-maintained Markdown collections outside the index
- A pack's own milestone model

## Consequences

ADR 0124's point 3 is resolved; campaigns/README.md's 'nothing here is loaded by the tool' becomes 'loaded read-only, never as instructions'. A dangling campaign citation is a plan validation error.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c245#11`
- `chatgpt-majordomus:c247#2`
- `chatgpt-majordomus:c297#9`
- `chatgpt-majordomus:c210#2`
