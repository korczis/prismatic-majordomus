---
schema: knowledge/v1
id: rhai-is-not-an-os-sandbox
kind: knowledge
class: constraint
title: "Rhai is not an operating-system sandbox, and native Rust plugins have no stable ABI"
description: "When extension from outside the tool's tree is proposed"
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
  - type: documents
    target: file:.ai/repo/project/issues/I3306.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3305.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/repository-automation-is-typed.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3157.yaml
---

# Rhai is not an operating-system sandbox, and native Rust plugins have no stable ABI

Rhai gives a restricted engine with no file, process or network function unless the host registers one; that is a language boundary, not process isolation. Installing arbitrary external Rust libraries as plugins would need its own trust and isolation model, and dynamic libraries cannot be assumed ABI-stable across compiler or dependency versions. External extension therefore goes through scripted capabilities under ADR 0069's effect ceiling, not native plugins.

## Sources

- `chatgpt-majordomus:c244#8`
