---
schema: knowledge/v1
id: no-vendor-api-reads-a-chatgpt-project
kind: knowledge
class: constraint
title: "No vendor API reads a person's ChatGPT Project; only the person's own session and the vendor's export reach it"
description: "Applies to any plan for reading or syncing ChatGPT workspace content."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
    - file:.ai/repo/workspaces/chatgpt-majordomus.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - type: documents
    target: file:.ai/repo/adrs/0212-a-workspace-is-read-in-the-operator-s-own-signed-in-browser-or-f.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3279.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3278.yaml
---

# No vendor API reads a person's ChatGPT Project; only the person's own session and the vendor's export reach it

Checked 2026-09-10 and again in a 2026-10-15 conversation: the Conversations API holds only conversations the API created, Sign in with ChatGPT does not expose Projects, and ChatGPT's MCP support calls servers rather than serving history. A Project is a 'gizmo' on the undocumented backend. Any sync is therefore the operator's own browser session or the vendor's export; both are read-only and neither is a contract.

## Sources

- `chatgpt-majordomus:c267#2`
