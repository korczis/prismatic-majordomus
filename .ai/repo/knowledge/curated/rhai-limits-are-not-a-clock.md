---
schema: knowledge/v1
id: rhai-limits-are-not-a-clock
kind: knowledge
class: constraint
title: "Rhai's operation limits are not a wall-clock timeout; the capability enforces deadlines"
description: "When a scripted capability calls a slow capability or needs a timeout"
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
    - file:apps/majordomus-cli/src/script/mod.rs
    - file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
  - type: documents
    target: file:.ai/repo/project/issues/I3157.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3306.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/repository-automation-is-typed.yaml
---

# Rhai's operation limits are not a wall-clock timeout; the capability enforces deadlines

The engine in apps/majordomus-cli/src/script/mod.rs bounds operations, call depth and sizes and checks a cancel flag on every operation, and is built with no_time. A script that makes one call into an expensive Rust capability spends few operations and can still run for minutes. Deadlines, process-tree termination and cancellation of the callee are therefore the executor's and the capability's job (rule every-wait-is-bounded), never the script's limits.

## Sources

- `chatgpt-majordomus:c230#2`
