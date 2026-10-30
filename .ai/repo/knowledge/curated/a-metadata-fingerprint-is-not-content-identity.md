---
schema: knowledge/v1
id: a-metadata-fingerprint-is-not-content-identity
kind: knowledge
class: lesson
title: "A fingerprint of mtime and size detects possible change; it is not content identity and does not prove outputs intact"
description: "When designing any skip or cache key for derive, a gate or a build"
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:scripts/generate-site-data
relations:
  - type: documents
    target: file:.ai/repo/adrs/0185-a-derivation-is-a-typed-graph-of-the-generators-the-executable-a.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3234.yaml
---

# A fingerprint of mtime and size detects possible change; it is not content identity and does not prove outputs intact

Metadata may filter candidates fast, but a skip that claims equivalence must compare content digests of every input the producer reads, including the generator itself. I2880's own record (2026-10-17) found that generate-site-data's input list (1613 paths) omitted the script, the libraries it sources and the executable it calls, so a changed generator with unchanged inputs would have been skipped. An unchanged input key also says nothing about a deleted or tampered output, so outputs are verified separately.

## Sources

- `chatgpt-majordomus:c292#2`
