---
schema: knowledge/v1
id: a-plan-on-an-unlanded-branch-is-planned-again
kind: knowledge
class: lesson
title: "A plan that lives only on an unlanded branch or a rescue ref is invisible to master-reading planners, and gets planned again"
description: "Before planning a subsystem, and when a records branch stalls"
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
    - file:scripts/collision-check
relations:
  - type: relates_to
    target: file:.ai/repo/project/milestones/a-derive-answers-within-a-second.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0185-a-derivation-is-a-typed-graph-of-the-generators-the-executable-a.md
---

# A plan that lives only on an unlanded branch or a rescue ref is invisible to master-reading planners, and gets planned again

The incremental-derive plan (I2521-I2533) was pushed on 2026-10-13 and never landed. An earlier delivery-packs plan (I2344-I2353) and the CI critical-path milestone survive only on rescue refs. On 10-15 and 10-17 the owner asked for the same derive work again. Meanwhile two sessions built separate stage-B skips (I2490 on 10-12, I2880 on 10-17). Before planning, ask the branches and refs/rescue at their tips whether they carry the subsystem's milestone and issue files (scripts/collision-check --new <paths>, which reads every branch tip once; never an unbounded git log --all over the shared machine, see s-20261028215542-dcf9-30333fac611c), and land a records-only branch quickly or close it.

## Sources

- `chatgpt-majordomus:c265#0`
- `chatgpt-majordomus:c292#0`
