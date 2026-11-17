---
schema: adr/v1
id: adr-0183
kind: adr
title: "Evolution is scheduled at the episode boundary and carried out by a session; nothing promotes itself"
status: proposed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0118-a-session-leaves-what-it-observed-and-one-candidate-per-defect-is-routed.md
  - file:.ai/repo/adrs/0091-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md
  - rule:project.a-personal-layer-changes-nothing-shared
  - file:.ai/repo/project/issues/I3163.yaml
  - file:.ai/repo/project/issues/I3164.yaml
  - file:.ai/repo/project/issues/I3165.yaml
  - file:.ai/repo/project/milestones/knowledge-acts-on-what-it-learns.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0118-a-session-leaves-what-it-observed-and-one-candidate-per-defect-is-routed.md
    - file:.ai/repo/adrs/0091-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md
    - file:.ai/repo/project/issues/I3163.yaml
    - file:.ai/repo/project/issues/I3164.yaml
    - file:.ai/repo/project/issues/I3165.yaml
    - file:.ai/repo/project/milestones/knowledge-acts-on-what-it-learns.yaml
---

# 183. Evolution is scheduled at the episode boundary and carried out by a session; nothing promotes itself

## Context

ADR 0118 derives routed candidates without a model and leaves acting on a route to a later milestone; the owner asked (2026-10-16) that /evolve run at the right time without being called. ADR 0091 makes promotion a person's act.

## Decision

The boundary derives a readiness verdict (skip, defer, run) from recurrence, routes, a cooldown and a budget declared in policy, records it even when skipped, and the next session is told; a 'run' is executed by a session through the evolve skill; changes it makes are not sources for the deriver; promotion and merging remain acts.

## Alternatives rejected

- A session-end hook that runs evolve itself
- A manual 'review this session' prompt as the mechanism
- Promoting candidates automatically
- Another orchestration system

## Consequences

Amends ADR 0118's non-scope; four sessions meeting one snag yield one correction to the mechanism rather than four notes.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c274#0`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c197#8`
- `chatgpt-majordomus:c274#4`
