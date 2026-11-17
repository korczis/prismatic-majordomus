---
schema: knowledge/v1
id: claude-code-mods-are-unsandboxed-hooks-in-plugins
kind: knowledge
class: fact
title: "A Claude Code mod is a TypeScript hook shipped inside a plugin, and it runs unsandboxed"
description: "Applies when generating or trusting client extensions."
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
    - file:.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md
    - file:share/mods/claude-code/majordomus-brief/README.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md
  - type: documents
    target: file:.ai/repo/project/issues/I3302.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3300.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0214-a-provider-s-native-extension-points-carry-the-contract-as-gener.md
---

# A Claude Code mod is a TypeScript hook shipped inside a plugin, and it runs unsandboxed

Mods run before, after, instead of or around client events and can rewrite prompts, block tool calls and change the UI (vendor news, 2026-10-02). This repository ships two: share/mods/claude-code/majordomus-brief and majordomus-handover, each with hooks/register.ts and a .claude-plugin. Because they are unsandboxed, provenance of an installed mod is the thing to check.

## Sources

- `chatgpt-majordomus:c174#4`
