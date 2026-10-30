---
schema: knowledge/v1
id: integration-capacity-bounds-the-backlog
kind: knowledge
class: lesson
title: "The backlog grows whenever finished work arrives faster than it can be integrated, however fast it is written"
description: "When planning parallel sessions, choosing batch sizes, or reading a growing open pull request count."
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
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
    - file:.ai/repo/adrs/0101-pull-requests-are-integrated-one-at-a-time-against-the-current-master.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - type: relates_to
    target: file:.ai/repo/adrs/0101-pull-requests-are-integrated-one-at-a-time-against-the-current-master.md
  - type: relates_to
    target: file:.ai/repo/adrs/0197-the-backlog-is-bounded-by-a-declared-bound-that-every-session-re.md
  - type: relates_to
    target: file:.ai/repo/adrs/0198-delivery-is-judged-by-verified-outcomes-landed-per-day-with-lead.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3204.yaml
  - type: documents
    target: file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
---

# The backlog grows whenever finished work arrives faster than it can be integrated, however fast it is written

Between 2026-10-10 and 2026-10-18 the open pull request count climbed from tens to near ninety while sessions kept finishing work. Days of creation and days of integration alternated: on 2026-10-15 a reading counted 74 opened and none merged, on 2026-10-17 69 merged against 17 opened. Each new implementation pull request feeds the queue; a control plane that models the flow better than the one-at-a-time validate pool can process it stalls before merge. Collaboration is measured by safely integrated results, not by open pull requests or finished sessions. The remedies that worked were small green batches through one landing seat (ADR 0167) and stopping new scope (Operation Convergence), not more authors.

## Sources

- `chatgpt-majordomus:c142#1`
- `chatgpt-majordomus:c220#2`
- `chatgpt-majordomus:c227#2`
- `chatgpt-majordomus:c262#5`
- `chatgpt-majordomus:c295#7`
