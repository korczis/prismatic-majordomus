---
schema: knowledge/v1
id: liveness-differs-by-surface
kind: knowledge
class: fact
title: "On master 707a04c7 each surface is live by a different mechanism, and executions are not a monitor source"
description: "When building or judging anything that claims to be live; re-measure after the peer-blackboard milestone lands."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:apps/majordomus-cli/src/http/events.rs
    - file:share/ui/ui.js
    - file:docs/MONITOR.md
    - file:.ai/repo/adrs/0158-the-observatory-draws-only-what-was-observed.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0158-the-observatory-draws-only-what-was-observed.md
  - type: documents
    target: file:.ai/repo/adrs/0216-every-live-surface-follows-one-channel-with-named-follow-streams.md
  - type: documents
    target: file:.ai/repo/project/milestones/every-surface-follows-what-changed.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3218.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3219.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3220.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3221.yaml
---

# On master 707a04c7 each surface is live by a different mechanism, and executions are not a monitor source

Read on 2026-10-18: /events is a WebSocket of execution events only (http/events.rs); /ui re-reads its screen every 60 s (share/ui/ui.js REFRESH_MS); the Observatory asks /api/v1/observatory/events with a cursor (observatory.js); /cockpit/monitor polls; docs/MONITOR.md says executions are not a monitor source; the published site is a snapshot. ADR 0158 already states the Observatory's journal-follow is not a push. The peer-blackboard branch (ADR 0176 (on an unlanded branch)) adds /events?follow=blackboard.

## Sources

- `chatgpt-majordomus:c260#8`
- `chatgpt-majordomus:c288#8`
- `chatgpt-majordomus:c294#7`
