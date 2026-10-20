---
schema: knowledge/v1
id: a-partial-green-is-not-the-verdict
kind: knowledge
class: constraint
title: "A green light workflow, a green release workflow or a merge is not the trunk's verdict and not delivery"
description: "When reading CI state, release state or deciding that work is delivered."
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
    - file:.ai/repo/adrs/0131-a-release-publishes-its-metadata-before-its-record-lands.md
    - file:.github/workflows/validate.yml
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0131-a-release-publishes-its-metadata-before-its-record-lands.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3200.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3113.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3208.yaml
  - type: relates_to
    target: file:.ai/repo/project/milestones/a-release-is-proven-end-to-end.yaml
  - type: documents
    target: file:.ai/repo/project/intents/finished-work-reaches-a-verified-release.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0198-delivery-is-judged-by-verified-outcomes-landed-per-day-with-lead.md
---

# A green light workflow, a green release workflow or a merge is not the trunk's verdict and not delivery

On 2026-10-13 the github-projection and version workflows were green on a head whose validate and pages runs were still queued. On 2026-10-17 the release workflow was green at a commit whose UI and Pages jobs had failed. If generate, deploy or verify fails after a merge, master is merged but not delivered. Only the aggregate ci check of validate.yml is the trunk's verdict, and only an observed publication is delivery; a partial success must be read as partial.

## Sources

- `chatgpt-majordomus:c112#3`
- `chatgpt-majordomus:c199#21`
- `chatgpt-majordomus:c255#3`
