---
schema: adr/v1
id: adr-0216
kind: adr
title: "Every live surface follows one channel with named follow streams, and nothing polls where a stream exists"
status: proposed
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0133-everything-is-followed-in-one-stream.md
  - file:.ai/repo/adrs/0142-no-screen-waits-for-a-source.md
  - file:.ai/repo/adrs/0158-the-observatory-draws-only-what-was-observed.md
  - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
  - file:apps/majordomus-cli/src/http/events.rs
  - file:docs/MONITOR.md
  - file:.ai/repo/project/milestones/every-surface-follows-what-changed.yaml
  - file:.ai/repo/project/issues/I3218.yaml
  - file:.ai/repo/project/issues/I3219.yaml
  - file:.ai/repo/project/issues/I3220.yaml
  - file:.ai/repo/project/issues/I3221.yaml
  - file:.ai/repo/project/issues/I3222.yaml
  - file:.ai/repo/project/issues/I3181.yaml
  - file:.ai/repo/project/issues/I3231.yaml
  - file:.ai/repo/project/intents/every-surface-follows-what-changed.yaml
  - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0133-everything-is-followed-in-one-stream.md
    - file:.ai/repo/adrs/0142-no-screen-waits-for-a-source.md
    - file:.ai/repo/adrs/0158-the-observatory-draws-only-what-was-observed.md
    - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
    - file:apps/majordomus-cli/src/http/events.rs
    - file:docs/MONITOR.md
    - file:.ai/repo/project/milestones/every-surface-follows-what-changed.yaml
    - file:.ai/repo/project/issues/I3218.yaml
    - file:.ai/repo/project/issues/I3219.yaml
    - file:.ai/repo/project/issues/I3220.yaml
    - file:.ai/repo/project/issues/I3221.yaml
    - file:.ai/repo/project/issues/I3222.yaml
    - file:.ai/repo/project/issues/I3181.yaml
    - file:.ai/repo/project/issues/I3231.yaml
    - file:.ai/repo/project/intents/every-surface-follows-what-changed.yaml
    - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
---

# 216. Every live surface follows one channel with named follow streams, and nothing polls where a stream exists

## Context

ADR 0133 joins what happens into one monitor stream; 5a's blackboard ADR (branch, in flight) pushes the blackboard over /events?follow=blackboard. Master still has three liveness mechanisms: /events for executions, a 60 s refresh on /ui and cursor polling in the Observatory and /cockpit/monitor; the owner asked repeatedly (2026-10-10, 10-14, 10-18) for push over WebSocket everywhere. ChatGPT also proposed one DomainEvent model (2026-10-11) and an Observatory-only socket was explicitly rejected (2026-10-14).

## Decision

/events is the one live channel. Each source the server records (executions, the board and journal, the monitor's signals, plan and intent writes) is a named follow stream on it with a cursor, an epoch and resync; a surface subscribes to the streams it renders and polls only a source that cannot push, saying so; frames carry record identities, not a second event model. 5a's blackboard follow protocol (branch, in flight) is the template; this ADR generalises it beyond the blackboard.

## Alternatives rejected

- An Observatory- or UI-specific WebSocket server
- A new DomainEvent store beside the journal and the execution store
- Cursor polling as the live path

## Consequences

I3219 adds the journal as a topic of master's /events and extends PR #1178's client; I3218's pull half lands on master and its pushed half joins when I2981 lands; I3220 needs I2981's crate::change; I3222 judges the streams on master; the status words are ADR 0158's, with POLLING added by I3221. I2487's 'one stream tells an open page which readings changed' rides this channel rather than its own. If 5a prefers, this is an amendment of 5a's blackboard ADR (branch, in flight) rather than a new ADR.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c294#3`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c225#2`
- `chatgpt-majordomus:c227#2`
- `chatgpt-majordomus:c260#8`
- `chatgpt-majordomus:c294#3`
- `chatgpt-majordomus:c294#7`
