---
schema: knowledge/v1
id: positioning-leads-with-the-outcome
kind: knowledge
class: convention
title: "Majordomus is explained by the outcome it decides, never by its architecture or as a coding agent"
description: "Applies to any public sentence about the product: homepage, README, launch kit, posts, talks."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:site/data/marketing.toml
    - file:docs/LAUNCH.md
    - file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
  - type: relates_to
    target: file:.ai/repo/adrs/0210-the-sentence-that-says-what-majordomus-is-has-one-declaration-an.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3134.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3223.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/a-visitor-understands-and-checks-majordomus.yaml
---

# Majordomus is explained by the outcome it decides, never by its architecture or as a coding agent

The lead is the pain every agent user recognises: the agent said it was done and it was not; Majordomus decides whether it is. Coding agents (Claude Code, Codex, Gemini, local models) are interchangeable workers; Majordomus is never presented as a model, a better coding agent or an orchestration platform. The first viewport states one outcome, one action and a link to proof, not a catalogue of subsystems. Positioning was acted on in site/data/marketing.toml (2026-10-16, I2745) and is bounded by homepage.toml [promises]. Public security statements stay within what is verifiable: Majordomus itself makes no model call and sends nothing over the network; what the agent sends is the agent's configuration.

## Sources

- `chatgpt-majordomus:c114#0`
- `chatgpt-majordomus:c156#3`
- `chatgpt-majordomus:c156#4`
- `chatgpt-majordomus:c177#0`
- `chatgpt-majordomus:c217#0`
- `chatgpt-majordomus:c249#0`
- `chatgpt-majordomus:c249#1`
- `chatgpt-majordomus:c250#4`
- `chatgpt-majordomus:c241#2`
