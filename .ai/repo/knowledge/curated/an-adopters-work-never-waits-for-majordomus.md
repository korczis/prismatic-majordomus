---
schema: knowledge/v1
id: an-adopters-work-never-waits-for-majordomus
kind: knowledge
class: convention
title: "Work in an adopting repository discovers what the installed Majordomus offers and never waits for a missing feature"
description: "Applies to a mission run under Majordomus in a repository other than this one."
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/project/milestones/a-second-adopter-is-served.yaml
    - file:.ai/repo/project/milestones/adopter-upgrades.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/project/issues/I3129.yaml
---

# Work in an adopting repository discovers what the installed Majordomus offers and never waits for a missing feature

The worker asks the installed tool what it serves (help, capabilities, doctor) rather than assuming a command from this repository's documentation. It never invents a command and never adds an orchestration subsystem to the product it is building. A gap in Majordomus is reported to this repository as a finding (an adopter field report becomes issues here) and the product work continues without it. This keeps the adopter's delivery independent of this repository's roadmap and turns each gap into evidence for a-second-adopter-is-served.

## Sources

- `chatgpt-majordomus:c224#12`
