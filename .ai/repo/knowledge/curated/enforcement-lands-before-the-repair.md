---
schema: knowledge/v1
id: enforcement-lands-before-the-repair
kind: knowledge
class: lesson
title: "Enforcement lands before the repair, because a peer running yesterday's instructions overwrites today's"
description: "When a defect in process (versioning, publication, merging) is found while other sessions keep working."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - integration-release
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0155-an-expedited-landing-is-the-owners-word.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0155-an-expedited-landing-is-the-owners-word.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3113.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3200.yaml
---

# Enforcement lands before the repair, because a peer running yesterday's instructions overwrites today's

On 2026-09-12 the version gate and Pages publication both existed but nothing turned their verdicts into forced work, and a long reconstruction would have been overwritten by peers still carrying older AGENTS.md, hooks and prompts. Updating instructions cannot stop a worker that has not read them yet; only the canonical state it pushes to can (branch protection, a red ci verdict, a refused claim). Land the small enforcing change first, then repair under it.

## Sources

- `chatgpt-majordomus:c130#8`
- `chatgpt-majordomus:c130#10`
