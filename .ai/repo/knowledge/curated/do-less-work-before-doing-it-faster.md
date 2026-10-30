---
schema: knowledge/v1
id: do-less-work-before-doing-it-faster
kind: knowledge
class: lesson
title: "Runners, timeouts and shards hide a structural cost; removing repeated work is what has made this repository faster"
description: "When a derive, a gate or the suite is slow and the reflex is more hardware or a higher bound"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/ci/suite.yaml
    - file:scripts/generate-site-data
    - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - type: documents
    target: file:.ai/repo/project/issues/I3233.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3235.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3239.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3241.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3240.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0185-a-derivation-is-a-typed-graph-of-the-generators-the-executable-a.md
  - type: relates_to
    target: file:.ai/repo/adrs/0186-a-pull-request-s-required-verdict-is-the-feedback-lane-a-gate-wh.md
  - type: relates_to
    target: file:.ai/repo/adrs/0187-tests-are-selected-by-the-impact-of-a-change-derived-from-what-e.md
---

# Runners, timeouts and shards hide a structural cost; removing repeated work is what has made this repository faster

The documented derive wins removed spawns and repeated passes: three site-data generations collapsed into one check plus a fingerprint, a site check of about 7000 processes became one AWK pass, and 467 inputs were hashed in one batch. The suite bound was raised from 150 to 225 min to follow runner load, which bought a verdict and no speed. ChatGPT's proposed order, a proposal and not a measurement: lanes and impact selection, long-case decomposition, supersede obsolete runs, build once, compiler cache, browser selection, content-addressed reuse, shallow fetch, runner tuning last.

## Sources

- `chatgpt-majordomus:c228#3`
- `chatgpt-majordomus:c265#8`
