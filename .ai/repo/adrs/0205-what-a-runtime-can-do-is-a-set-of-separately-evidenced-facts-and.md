---
schema: adr/v1
id: adr-0205
kind: adr
title: "What a runtime can do is a set of separately evidenced facts, and detection never authorises"
status: proposed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0049-the-model-catalogue-is-declarative-data-and-routing-is-an-ex.md
  - rule:project.mesh-is-observation-not-authority
  - file:.ai/repo/project/issues/I3199.yaml
  - file:.ai/repo/project/issues/I3198.yaml
  - file:.ai/repo/project/issues/I3177.yaml
  - file:.ai/repo/project/issues/I3185.yaml
  - file:.ai/repo/project/milestones/mesh-peers-execute-assigned-work.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0049-the-model-catalogue-is-declarative-data-and-routing-is-an-ex.md
    - file:.ai/repo/project/issues/I3199.yaml
    - file:.ai/repo/project/issues/I3198.yaml
    - file:.ai/repo/project/issues/I3177.yaml
    - file:.ai/repo/project/issues/I3185.yaml
    - file:.ai/repo/project/milestones/mesh-peers-execute-assigned-work.yaml
---

# 205. What a runtime can do is a set of separately evidenced facts, and detection never authorises

## Context

The 10-16 browser design and the 10-17 local-inference design both met a single optimistic 'supported' flag: a browser able to run a task is not allowed to; a model name does not prove a context window.

## Decision

A capability of a runtime is declared, compiled, detected, authorised, verified and available as separate facts, each with provenance and time; routing reads the facts it needs and states which; inference never replaces authorisation.

## Alternatives rejected

- One boolean per capability
- Capabilities guessed from a runtime or model name

## Consequences

Generalises ADR 0174 (on an unlanded branch)'s evidence levels (in flight) and ADR 0170 (on an unlanded branch)'s untrusted browser; I3177 and I3199 read it.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c275#4`
