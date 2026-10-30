---
schema: knowledge/v1
id: a-second-implementation-drifts
kind: knowledge
class: lesson
title: "Two implementations of one verdict drift; delete one or test their parity"
description: "When shell and Rust both implement an operation"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.github/workflows/github.yml
relations:
  - type: documents
    target: file:.ai/repo/adrs/0184-every-command-name-is-implemented-by-exactly-one-executable-and.md
  - type: documents
    target: file:.ai/repo/project/issues/I3156.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3159.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3145.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/repository-automation-is-typed.yaml
---

# Two implementations of one verdict drift; delete one or test their parity

.github/workflows/github.yml records that on 2026-10-12 the shell reading of the model reported 318 drift findings where the Rust executable reported 26. The workflow now forces the Rust executable, which fixes the symptom but not the class. The class goes away only when the second path is deleted, or while both exist, when a parity case compares them. Consolidation is measured by fewer independent definitions of one operation, not by scripts rewritten.

## Sources

- `chatgpt-majordomus:c244#3`
- `chatgpt-majordomus:c244#20`
