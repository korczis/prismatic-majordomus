---
schema: adr/v1
id: adr-0181
kind: adr
title: "An episode records the governance and the briefing it was handed, each by one digest"
status: proposed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0066-entry-reports-a-verdict-per-claim-and-the-evidence-behind-it.md
  - file:.ai/repo/adrs/0043-entering-the-repository-brings-its-runtime-up.md
  - rule:project.entry-reports-only-evidence
  - file:docs/PREFLIGHT.md
  - file:.ai/repo/adrs/0111-a-task-names-what-it-serves-and-the-binding-is-derived.md
  - file:.ai/repo/project/issues/I3147.yaml
  - file:.ai/repo/project/issues/I3148.yaml
  - file:.ai/repo/project/issues/I3149.yaml
  - file:.ai/repo/project/issues/I3150.yaml
  - file:.ai/repo/project/issues/I3151.yaml
  - file:.ai/repo/project/milestones/entry-proves-what-it-handed.yaml
  - file:.ai/repo/project/intents/entry-proves-what-it-handed.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0066-entry-reports-a-verdict-per-claim-and-the-evidence-behind-it.md
    - file:.ai/repo/adrs/0043-entering-the-repository-brings-its-runtime-up.md
    - file:docs/PREFLIGHT.md
    - file:.ai/repo/adrs/0111-a-task-names-what-it-serves-and-the-binding-is-derived.md
    - file:.ai/repo/project/issues/I3147.yaml
    - file:.ai/repo/project/issues/I3148.yaml
    - file:.ai/repo/project/issues/I3149.yaml
    - file:.ai/repo/project/issues/I3150.yaml
    - file:.ai/repo/project/issues/I3151.yaml
    - file:.ai/repo/project/milestones/entry-proves-what-it-handed.yaml
    - file:.ai/repo/project/intents/entry-proves-what-it-handed.yaml
---

# 181. An episode records the governance and the briefing it was handed, each by one digest

## Context

The preflight reports governance per claim (ADR 0066) but no value identifies the effective governance; docs/PREFLIGHT.md states no compiled context revision exists in a checkout. The owner asked to see at every start that governance is in force (2026-09-10).

## Decision

One function computes a governance digest over a declared input set with volatile fields excluded; the preflight, the brief, session.started and handovers carry it; session.started also carries the digest of the briefing it injected; a differing digest at the next interaction is told, never silently re-resolved.

## Alternatives rejected

- Filesystem watchers
- Validation only at start
- One fingerprint over the whole repository (moves on every commit)

## Consequences

Every entry surface must report the same digest, which becomes a parity case; a handover resumed after governance moved says so.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c103#2`
