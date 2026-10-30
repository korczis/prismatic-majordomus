---
schema: knowledge/v1
id: overview-first-evidence-one-click-away
kind: knowledge
class: convention
title: "Every surface leads with the overview, then relationships, then detail on demand, with evidence one click away"
description: "When laying out a screen, a homepage section or a panel."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0088-a-dashboard-is-a-projection-of-capabilities-and-every-card-names-its-source.md
    - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0088-a-dashboard-is-a-projection-of-capabilities-and-every-card-names-its-source.md
  - type: relates_to
    target: file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/a-generator-that-can-list-everything-shows-everything.md
  - type: documents
    target: file:.ai/repo/project/issues/I3230.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3224.yaml
---

# Every surface leads with the overview, then relationships, then detail on demand, with evidence one click away

Agreed in conversation on 2026-10-03 and 2026-10-04: density is good, compression on the first screen is the defect. A screen answers first what is happening and what needs attention, in task words, then the relations, then entity detail; every significant status can say why it is shown. Evidence is never more than one click from the claim it supports.

## Sources

- `chatgpt-majordomus:c180#2`
- `chatgpt-majordomus:c186#5`
