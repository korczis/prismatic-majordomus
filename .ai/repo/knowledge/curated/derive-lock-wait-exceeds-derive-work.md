---
schema: knowledge/v1
id: derive-lock-wait-exceeds-derive-work
kind: knowledge
class: fact
title: "On 2026-10-17 derives on the shared machine waited on the derive lock longer than they worked"
description: "When prioritising derive speed work, or arguing about the lock"
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
    - file:lib/machine_lock.sh
relations:
  - type: documents
    target: file:.ai/repo/project/issues/I3234.yaml
---

# On 2026-10-17 derives on the shared machine waited on the derive lock longer than they worked

From 111 derive logs of the release seat's sessions (I2880 record, branch docs/a-derive-with-current-site-data-skips-generation): stages A-D took a median of 4.5 min, of which stage B was 3.9 min, and the derives waited 15.3 h in total on the lock against 9.5 h of work (median wait 4.0 min, p90 19.8, max 70). A dated observation, not a current figure. It means that shrinking each derive's work also shrinks everyone's wait, and that a no-op answered outside the lock helps most.

## Sources

- `chatgpt-majordomus:c292#0`
