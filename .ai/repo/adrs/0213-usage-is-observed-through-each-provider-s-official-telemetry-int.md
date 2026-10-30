---
schema: adr/v1
id: adr-0213
kind: adr
title: "Usage is observed through each provider's official telemetry into one provider-neutral record, and reported, estimated and billed amounts are never merged"
status: proposed
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
  - file:.ai/repo/adrs/0009-prompt-capture-happens-below-the-model.md
  - file:.ai/repo/adrs/0103-every-client-gets-an-episode-and-every-provider-capability-cites-its-evidence.md
  - rule:project.never-store-transcripts
  - rule:project.providers-are-data
  - file:apps/majordomus-cli/src/economics/usage.rs
  - file:docs/ECONOMICS.md
  - file:.ai/repo/project/issues/I3287.yaml
  - file:.ai/repo/project/issues/I3288.yaml
  - file:.ai/repo/project/issues/I3289.yaml
  - file:.ai/repo/project/issues/I3290.yaml
  - file:.ai/repo/project/issues/I3291.yaml
  - file:.ai/repo/project/issues/I3292.yaml
  - file:.ai/repo/project/issues/I3293.yaml
  - file:.ai/repo/project/issues/I3294.yaml
  - file:.ai/repo/project/issues/I3295.yaml
  - file:.ai/repo/project/issues/I3296.yaml
  - file:.ai/repo/project/issues/I3297.yaml
  - file:.ai/repo/project/milestones/every-sessions-usage-is-observed.yaml
  - file:.ai/repo/project/intents/every-sessions-usage-is-observed.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md
    - file:.ai/repo/adrs/0009-prompt-capture-happens-below-the-model.md
    - file:.ai/repo/adrs/0103-every-client-gets-an-episode-and-every-provider-capability-cites-its-evidence.md
    - file:apps/majordomus-cli/src/economics/usage.rs
    - file:docs/ECONOMICS.md
    - file:.ai/repo/project/issues/I3287.yaml
    - file:.ai/repo/project/issues/I3288.yaml
    - file:.ai/repo/project/issues/I3289.yaml
    - file:.ai/repo/project/issues/I3290.yaml
    - file:.ai/repo/project/issues/I3291.yaml
    - file:.ai/repo/project/issues/I3292.yaml
    - file:.ai/repo/project/issues/I3293.yaml
    - file:.ai/repo/project/issues/I3294.yaml
    - file:.ai/repo/project/issues/I3295.yaml
    - file:.ai/repo/project/issues/I3296.yaml
    - file:.ai/repo/project/issues/I3297.yaml
    - file:.ai/repo/project/milestones/every-sessions-usage-is-observed.yaml
    - file:.ai/repo/project/intents/every-sessions-usage-is-observed.yaml
---

# 213. Usage is observed through each provider's official telemetry into one provider-neutral record, and reported, estimated and billed amounts are never merged

## Context

The owner ordered automatic token and model tracking (10-12) and then a generic fabric rather than three provider subsystems (10-13). docs/ECONOMICS.md already classes every number, and economics::usage already normalises Anthropic and OpenAI usage for benchmark runs. Telemetry events are runtime volume.

## Decision

Official seams only (Claude Code OpenTelemetry, Codex app-server notifications, Gemini CLI OpenTelemetry), declared per provider in share/providers.yaml. One usage record, distinct from capability executions, and one normalisation. Events are runtime state, never .ai/repo; .ai holds policy, schema and budgets. Reported tokens, estimates and billed amounts are distinct fields; a missing value is absent, never zero; cumulative snapshots are never summed; provider metadata is observed evidence, never canonical state; no transcript or body is read or kept.

## Alternatives rejected

- Transcript parsing
- A MITM proxy or client patch
- Per-provider telemetry subsystems
- One 'cost' field
- Storing events under .ai/repo

## Consequences

execution-telemetry loses its dependency on runtime-adapters; the server gains a loopback OTLP/HTTP-JSON receiver with no new dependency; subscription users see estimates labelled as such.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c174#18`
- `chatgpt-majordomus:c234#0`
- `chatgpt-majordomus:c236#4`
- `chatgpt-majordomus:c236#7`
- `chatgpt-majordomus:c253#2`
- `chatgpt-majordomus:c253#3`
- `chatgpt-majordomus:c253#4`
