---
schema: knowledge/v1
id: the-intent-loop-was-built-on-branches-that-never-landed
kind: knowledge
class: fact
title: "Most of the intent loop's 2026-10-07..10-10 implementation and two owner-pack plans exist only on unmerged branches"
description: "Before planning intent-loop work, read the rescue refs and these branches; reviving keeps their bytes."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/project/intents/intent-planning.yaml
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/a-plan-on-an-unlanded-branch-is-planned-again.md
  - type: documents
    target: file:.ai/repo/project/issues/I3259.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3261.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3260.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3255.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3246.yaml
---

# Most of the intent loop's 2026-10-07..10-10 implementation and two owner-pack plans exist only on unmerged branches

The accepted 2026-10-07 order A-G (binding, opposition, satisfaction engine, completion, GitHub reconciliation, Cockpit view, outcome-to-plan) was implemented on feature/opposition-is-executed, feature/satisfaction-is-explained, feature/the-cockpit-shows-the-intent-loop and rescue/d1/feature/{what-remains-of-an-intent-is-derived,a-finish-reads-what-its-work-serves,github-agrees-with-the-intent}; only A (intent-bound-work) reached master. Planning records for drafting, command/deployment criteria and the e2e proof sit on origin/chore/the-owner-packs-are-planned (2026-10-10) and origin/chore/seven-owner-packs-are-tracked-and-planned (2026-10-15), never merged. On 2026-10-18 they were 281 to 1,129 commits behind master. Local revival leaves I2820-I2824 (2026-10-17) cover opposition, satisfaction explained, what remains and GitHub agreement, but not finish-reads-what-its-work-serves or the owner-pack plans. A branch that holds a plan is not a plan the plan engine sees.

## Sources

- `chatgpt-majordomus:c189#1`
