---
schema: knowledge/v1
id: naive-fan-out-burns-tokens
kind: knowledge
class: lesson
title: "Fan-out without narrow briefs turns an optimisation into a token burner"
description: "Applies when a session spawns subagents."
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
    - file:.ai/repo/benchmarks/economics/methodology.yaml
relations:
  - type: documents
    target: file:.ai/repo/project/issues/I3298.yaml
---

# Fan-out without narrow briefs turns an optimisation into a token burner

Each subagent reloads project instructions, repeats the same searches and returns long reports the architect reads in full. Briefs must be narrow and return structured results; whether fan-out pays is measured by the benchmark's topology arm, not argued.

## Sources

- `chatgpt-majordomus:c252#2`
