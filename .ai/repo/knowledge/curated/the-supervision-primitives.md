---
schema: knowledge/v1
id: the-supervision-primitives
kind: knowledge
class: fact
title: "Supervision primitives already exist: heartbeats, lease takeover, idle exit, reaper and recovery"
description: "Before designing any watchdog or self-repair."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:scripts/reap-orphans
    - file:.ai/repo/policy.yaml
    - file:.ai/repo/adrs/0039-liveness-is-four-rules-advisory-until-a-scan-can-decide-them.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0039-liveness-is-four-rules-advisory-until-a-scan-can-decide-them.md
  - type: documents
    target: file:.ai/repo/adrs/0200-supervision-repairs-only-what-is-idempotent-owned-and-verified-a.md
  - type: documents
    target: file:.ai/repo/project/issues/I3182.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3183.yaml
  - type: documents
    target: file:.ai/repo/project/milestones/the-runtime-supervises-itself.yaml
---

# Supervision primitives already exist: heartbeats, lease takeover, idle exit, reaper and recovery

Verified on master 707a04c: mesh heartbeat 5 s with 30 s expiry (docs/MESH.md); server idle exit 900 s by default (policy server.idle_seconds, cli.rs DEFAULT_IDLE_SECONDS); a bounded lease takeover (I2126-I2129); health.report, monitor.stream and mesh doctor; majordomus recover orphans; scripts/reap-orphans, which never removes a server only because it is old or idle; scripts/liveness-check for unbounded waits. A true unix zombie can only be reaped by its parent.

## Sources

- `chatgpt-majordomus:c288#16`
