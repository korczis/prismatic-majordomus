---
schema: adr/v1
id: adr-0212
kind: adr
title: "A workspace is read in the operator's own signed-in browser or from the vendor's export, never through a launched automation profile or another tool's credential"
status: proposed
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - file:.ai/repo/workspaces/chatgpt-majordomus.yaml
  - file:scripts/lib/ui-audit.mjs
  - file:.ai/repo/project/issues/I3278.yaml
  - file:.ai/repo/project/issues/I3279.yaml
  - file:.ai/repo/project/milestones/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
  - file:.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
  - file:.ai/repo/adrs/0211-a-record-promoted-from-a-workspace-cites-the-message-it-came-fro.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
    - file:.ai/repo/workspaces/chatgpt-majordomus.yaml
    - file:scripts/lib/ui-audit.mjs
    - file:.ai/repo/project/issues/I3278.yaml
    - file:.ai/repo/project/issues/I3279.yaml
    - file:.ai/repo/project/milestones/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
    - file:.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
    - file:.ai/repo/adrs/0211-a-record-promoted-from-a-workspace-cites-the-message-it-came-fro.md
---

# 212. A workspace is read in the operator's own signed-in browser or from the vendor's export, never through a launched automation profile or another tool's credential

## Context

ADR 0032 and I1304 assume a Playwright-launched, already-authenticated profile. On 2026-09-10 Google sign-in refused that browser, and the only working path found was the Codex access token on the vendor's backend, which reads another tool's credential and which the client refused to perform. On 2026-10-18 the project was read in full by an in-page read in the owner's own signed-in browser, relayed to a loopback receiver. Conversations proposing 'no scraping, MCP app only' (10-12) and 'no token from another tool's storage' (10-15) bound the choice.

## Decision

Two transports: in-page (the operator's own browser session under a per-site permission, read-only, posting to a loopback receiver with a nonce, origin check, size bound and schema validation) and export (the vendor's data export, read offline). launchProfile is withdrawn as a workspace transport. No other tool's credential is read. The declaration's transports enumerate in-page and export.

## Alternatives rejected

- A launched automation profile (refused by the identity provider)
- The Codex token on the vendor backend (another tool's credential; ToS)
- A custom MCP app as the read path (ChatGPT calls servers; it exposes no project history)

## Consequences

I1304 is rewritten; the in-page read still uses an undocumented backend and can break, so the export transport is the fallback; the relay used on 2026-10-18 is replaced by a hardened receiver.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c239#4`
- `chatgpt-majordomus:c267#2`
