---
schema: adr/v1
id: adr-0193
kind: adr
title: "Conflicting evidence keeps the verdict vocabulary: the criterion reads unknown and cites both sides"
status: proposed
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0075-intent-realization-is-a-join-and-closed-work-does-not-outrank-evidence.md
  - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
  - file:apps/majordomus-cli/src/intent.rs
  - file:.ai/repo/project/issues/I3258.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
    - file:.ai/repo/adrs/0075-intent-realization-is-a-join-and-closed-work-does-not-outrank-evidence.md
    - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
    - file:apps/majordomus-cli/src/intent.rs
    - file:.ai/repo/project/issues/I3258.yaml
---

# 193. Conflicting evidence keeps the verdict vocabulary: the criterion reads unknown and cites both sides

## Context

Several conversations proposed a fourth state 'conflicted'; the constraint of 2026-10-08 says not to extend the public status vocabulary unless existing semantics cannot represent the state.

## Decision

The public verdict stays satisfied, unsatisfied, unknown. Conflict is a reason of unknown that lists every source with its standing; nothing is discarded or averaged.

## Alternatives rejected

- A fourth public state 'conflicted' (versioned contract, every consumer migrated)
- Latest evidence wins

## Consequences

No schema migration; the UI and explain show the reason; revisit if a consumer needs to act differently on conflict than on unknown.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c196#3`
- `chatgpt-majordomus:c196#6`
- `chatgpt-majordomus:c221#2`
