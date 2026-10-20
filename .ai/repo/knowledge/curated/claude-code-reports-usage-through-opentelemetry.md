---
schema: knowledge/v1
id: claude-code-reports-usage-through-opentelemetry
kind: knowledge
class: fact
title: "Each declared agent provider reports per-request usage through an official seam"
description: "Applies when choosing where usage numbers come from."
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:share/providers.yaml
    - file:apps/majordomus-cli/src/economics/usage.rs
relations:
  - type: documents
    target: file:.ai/repo/adrs/0213-usage-is-observed-through-each-provider-s-official-telemetry-int.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3287.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3288.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3290.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3291.yaml
---

# Each declared agent provider reports per-request usage through an official seam

Reported from vendor documentation on 2026-10-12/13, not yet verified here: Claude Code exports OpenTelemetry metrics and api_request events with model and token categories, and its status line exposes session fields; Codex's app-server emits thread/tokenUsage/updated with last and total usage; Gemini CLI emits gemini_cli.api_response with input, output, cached and thought tokens. Each adapter verifies its fields against the installed client before relying on them. The declared providers, with the lifecycle evidence each one carries, are listed in docs/generated/providers.md; a seam named here becomes data there only with dated vendor evidence.

## Sources

- `chatgpt-majordomus:c234#2`
- `chatgpt-majordomus:c236#4`
- `chatgpt-majordomus:c253#2`
