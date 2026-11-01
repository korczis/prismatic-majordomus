---
schema: knowledge/v1
id: chatgpt-prompts-assumed-phoenix
kind: knowledge
class: lesson
title: "Early ChatGPT prompts assumed an Elixir/Phoenix Cockpit; translate them to the Rust server"
description: "When importing a September prompt pack that mentions LiveView, mix or check_origin."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
    - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - type: relates_to
    target: file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - type: documents
    target: file:.ai/repo/project/issues/I3196.yaml
  - type: relates_to
    target: file:.ai/repo/project/milestones/the-hub-is-reached-from-any-device.yaml
---

# Early ChatGPT prompts assumed an Elixir/Phoenix Cockpit; translate them to the Rust server

The 09-11 hosting conversation assumed Phoenix (mix phx.server, LiveView, check_origin). Majordomus is one Rust executable serving /ui, /cockpit, the API and MCP. Items such as the LiveView reconnect or origin check map to the server's WebSocket/SSE and host checks; the Elixir terms must not enter the plan.

## Sources

- `chatgpt-majordomus:c118#9`
