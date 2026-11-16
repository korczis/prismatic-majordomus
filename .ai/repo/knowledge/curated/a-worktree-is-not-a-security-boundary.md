---
schema: knowledge/v1
id: a-worktree-is-not-a-security-boundary
kind: knowledge
class: constraint
title: "A git worktree is not a security boundary, so permission bypass belongs only inside hard isolation"
description: "Applies to any autonomy or permission policy."
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
    - file:.ai/repo/adrs/0145-a-personal-layer-changes-nothing-shared.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0145-a-personal-layer-changes-nothing-shared.md
  - type: relates_to
    target: file:.ai/repo/adrs/0214-a-provider-s-native-extension-points-carry-the-contract-as-gener.md
---

# A git worktree is not a security boundary, so permission bypass belongs only inside hard isolation

A worktree shares the user, the credentials, the network and the rest of the disk. Removing permission prompts is defensible only inside a container or VM with scoped mounts and no production credentials; inside a worktree, deny and ask rules for irreversible and external actions stay.

## Sources

- `chatgpt-majordomus:c273#6`
