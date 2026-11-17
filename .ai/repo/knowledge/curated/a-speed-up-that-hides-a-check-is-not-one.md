---
schema: knowledge/v1
id: a-speed-up-that-hides-a-check-is-not-one
kind: knowledge
class: constraint
title: "A CI speed-up that removes, defers, hides or skips a required check without impact proof is not an optimisation"
description: "When judging any change that claims to shorten CI"
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
  - type: documents
    target: file:.ai/repo/adrs/0187-tests-are-selected-by-the-impact-of-a-change-derived-from-what-e.md
  - type: relates_to
    target: file:.ai/repo/adrs/0186-a-pull-request-s-required-verdict-is-the-feedback-lane-a-gate-wh.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3238.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3242.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
---

# A CI speed-up that removes, defers, hides or skips a required check without impact proof is not an optimisation

Each of these counts as cheating, not as speed: a required test made non-blocking, validation moved to nightly only, a test skipped without proof that nothing it reads changed, a job hidden from branch protection, local timings presented as CI timings, or a rebuild after the measurement window. Warm, partial and cold results are reported separately, from GitHub's own timings. This restates rules project.never-reported-is-not-green and project.empty-is-not-failure for the speed domain. The 45/60/90 s tiers in the source prompt are ChatGPT's and not owner-accepted.

## Sources

- `chatgpt-majordomus:c195#2`
