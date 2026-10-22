---
schema: knowledge/v1
id: a-prompt-pack-in-the-owners-word
kind: knowledge
class: convention
title: "A prompt pack is a ZIP with shared instructions and independent prompts that each rebuild the maximum context"
description: "When the owner asks for a 'prompt pack', or a campaign is added"
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:campaigns/README.md
    - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
  - type: relates_to
    target: file:.ai/repo/adrs/0180-a-prompt-pack-is-a-campaign-kept-as-received-indexed-as-a-kind-a.md
  - type: documents
    target: file:.ai/repo/project/issues/I3169.yaml
---

# A prompt pack is a ZIP with shared instructions and independent prompts that each rebuild the maximum context

The owner defined the term on 2026-09-13 and asked that it be remembered. A pack is a ZIP archive with a common INSTRUCTIONS.md and README.md and a set of prompts. Every prompt is runnable on its own and begins by reconstructing the maximum available context: repository, sessions, handovers, governance, issues and milestones, git state, runtime and the current implementation. One overgrown megaprompt is not a pack. A received pack is kept verbatim under campaigns/ (rule project.a-brief-is-kept-as-received).

## Sources

- `chatgpt-majordomus:c132#4`
