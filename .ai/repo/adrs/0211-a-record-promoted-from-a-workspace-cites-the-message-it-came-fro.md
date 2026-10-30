---
schema: adr/v1
id: adr-0211
kind: adr
title: "A record promoted from a workspace cites the message it came from by a typed reference that validates in every checkout and resolves only where the store is held"
status: proposed
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - file:.ai/repo/adrs/0005-one-projection-plan-canonical-owners-and-the-site-as-registry-view.md
  - file:.ai/repo/adrs/0164-a-record-names-the-session-that-wrote-it.md
  - rule:project.never-store-transcripts
  - rule:project.no-new-nouns
  - file:share/schemas/majordomus/knowledge/knowledge.v1.schema.json
  - file:.ai/repo/project/issues/I3280.yaml
  - file:.ai/repo/project/issues/I3281.yaml
  - file:.ai/repo/project/issues/I3282.yaml
  - file:.ai/repo/project/issues/I3283.yaml
  - file:.ai/repo/project/issues/I3284.yaml
  - file:.ai/repo/project/issues/I3173.yaml
  - file:.ai/repo/project/issues/I3286.yaml
  - file:.ai/repo/project/milestones/a-promoted-record-opens-its-message.yaml
  - file:.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
    - file:.ai/repo/adrs/0005-one-projection-plan-canonical-owners-and-the-site-as-registry-view.md
    - file:.ai/repo/adrs/0164-a-record-names-the-session-that-wrote-it.md
    - file:share/schemas/majordomus/knowledge/knowledge.v1.schema.json
    - file:.ai/repo/project/issues/I3280.yaml
    - file:.ai/repo/project/issues/I3281.yaml
    - file:.ai/repo/project/issues/I3282.yaml
    - file:.ai/repo/project/issues/I3283.yaml
    - file:.ai/repo/project/issues/I3284.yaml
    - file:.ai/repo/project/issues/I3173.yaml
    - file:.ai/repo/project/issues/I3286.yaml
    - file:.ai/repo/project/milestones/a-promoted-record-opens-its-message.yaml
    - file:.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
---

# 211. A record promoted from a workspace cites the message it came from by a typed reference that validates in every checkout and resolves only where the store is held

## Context

ADR 0032 makes synced content checkout state and promotion a person's act, but gives a promoted record no way to say where it came from. The 2026-10-18 import produced hundreds of records whose sources are `conversation#index`. Two existing facts constrain the answer: rule project.never-store-transcripts forbids a record summarising a conversation, and a reference resolved against checkout-local state fails every other checkout (adopter report 2026-10-18 on task: references).

## Decision

One reference grammar, declared once and inlined by majordomus generate into the ADR, knowledge, intent, milestone and issue schemas, serves all five; each kind admits its own declared subset (plan records only tracked-resolvable types, knowledge keeps task:) and keeps its own origin semantics. The grammar gains `workspace:<workspace-id>/<conversation-id>#<message-id>`. Validity is structural against the tracked workspace declaration; resolution goes through the one workspaces capability and answers resolved, not-held-here or gone-upstream. A digits-only message is a positional reference, accepted and flagged until pinned. The reference carries the upstream conversation id, which needs the owner's own sign-in to open. /ui links it; the published site prints it as text.

## Alternatives rejected

- A summary or quote of the message in the record (rule project.never-store-transcripts)
- A hashed local id only (unopenable by a person in the vendor UI without the store)
- Resolving at validation time (fails every checkout without the store)
- A new 'source' or 'conversation' noun (rule project.no-new-nouns; the workspace noun exists)

## Consequences

The plan schemas change; the import's records need a pinning pass; the drill-back works only for the operator who holds the store, by design.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c267#2`
