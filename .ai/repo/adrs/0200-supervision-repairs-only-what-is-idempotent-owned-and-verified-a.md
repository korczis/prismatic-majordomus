---
schema: adr/v1
id: adr-0200
kind: adr
title: "Supervision repairs only what is idempotent, owned and verified, and never restarts blindly"
status: proposed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0039-liveness-is-four-rules-advisory-until-a-scan-can-decide-them.md
  - rule:project.recovery-is-idempotent
  - rule:project.reclaim-only-what-you-own
  - rule:project.every-wait-is-bounded
  - file:scripts/reap-orphans
  - file:.ai/repo/project/issues/I3182.yaml
  - file:.ai/repo/project/issues/I3183.yaml
  - file:.ai/repo/project/milestones/the-runtime-supervises-itself.yaml
  - file:.ai/repo/project/intents/nothing-in-the-mesh-waits-on-something-dead.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0039-liveness-is-four-rules-advisory-until-a-scan-can-decide-them.md
    - file:scripts/reap-orphans
    - file:.ai/repo/project/issues/I3182.yaml
    - file:.ai/repo/project/issues/I3183.yaml
    - file:.ai/repo/project/milestones/the-runtime-supervises-itself.yaml
    - file:.ai/repo/project/intents/nothing-in-the-mesh-waits-on-something-dead.yaml
---

# 200. Supervision repairs only what is idempotent, owned and verified, and never restarts blindly

## Context

Owner 2026-10-17 asked for a watchdog that keeps the mesh up, drains tasks and kills zombies. serve ensure exits idle servers by design, claims expire on heartbeat, some processes hold leases, and a zombie is reaped only by its parent.

## Decision

Supervision is observe, classify, decide, recover, verify inside the existing runtime. Automatic repair is limited to idempotent safe actions; resuming executions or requeueing work is verified first; live sessions, name-based kills, taking valid leases, deleting worktrees, bypassing trust and retrying uncertain merges are never automatic. Each repair has a reason, one owner, an idempotent action, a verification and a circuit breaker.

## Alternatives rejected

- A new autonomous daemon that restarts everything
- Restart=always for every process
- Forcing nodes to cooperate

## Consequences

I3182 then I3183; extends ADR 0039's liveness rules from advice to a read capability.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c288#16`
