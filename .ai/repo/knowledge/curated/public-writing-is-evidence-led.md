---
schema: knowledge/v1
id: public-writing-is-evidence-led
kind: knowledge
class: convention
title: "A public article about Majordomus takes every figure from the page that computes it, and each language edition is its own article"
description: "Applies to posts, articles and talks the owner publishes about the project."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/LAUNCH.md
    - file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3135.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3136.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0206-public-outreach-begins-only-after-a-stranger-has-finished-a-firs.md
---

# A public article about Majordomus takes every figure from the page that computes it, and each language edition is its own article

Every claim in an article links the recorded run, guarantee or generated page that backs it, and no figure is typed from memory, since figures go stale in prose. A Czech and an English piece are written separately for their readers rather than translated line by line. Drafts may be generated from the declared product copy, but nothing is published by the tool: publication is the owner's act. An article ends with something the reader can run (the challenge replay or the installer), and is published through the site build when it lives on majordomus.dev.

## Sources

- `chatgpt-majordomus:c159#0`
- `chatgpt-majordomus:c159#4`
- `chatgpt-majordomus:c137#2`
- `chatgpt-majordomus:c250#5`
