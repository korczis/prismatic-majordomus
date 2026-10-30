---
schema: knowledge/v1
id: a-claim-is-not-a-lock
kind: knowledge
class: constraint
title: "Mesh claims converge but are not strongly consistent, so no orchestrator may use them as a global lock"
description: "When a release right, a seat or a write right is derived from a claim."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/MESH.md
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - type: documents
    target: file:.ai/repo/project/issues/I3174.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3198.yaml
  - type: relates_to
    target: file:.ai/repo/project/milestones/mesh-peers-execute-assigned-work.yaml
---

# Mesh claims converge but are not strongly consistent, so no orchestrator may use them as a global lock

docs/MESH.md lists strong consistency and linearisable claims as not guaranteed; during a partition both sides can admit one exclusive claim. A right that must be unique needs a fencing epoch or one authoritative seat (I3174), not eventual convergence alone.

## Sources

- `chatgpt-majordomus:c294#7`
