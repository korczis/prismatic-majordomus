---
schema: context/v1
id: ai.repo.automation
kind: context
title: Automation inventory
description: Every unit of this repository's automation, its migration disposition, and the exemption that allows a shell unit to remain shell.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Automation inventory

ADR 0069 replaces this repository's shell automation gradually with Rhai scripts over
typed capabilities. `inventory.jsonl` is the tracked record of that migration: one JSON
object per line, one line per unit, in canonical order.

| field | holds |
|---|---|
| `unit` | the file (or directory) the record is about, relative to the repository root |
| `anchor` | optional: a place inside the unit — a workflow step, a recipe, a function — when the record is about shell embedded in a file |
| `disposition` | `A` core capability, `B` orchestration, `C` bootstrap glue, `D` dead, `E` generated, `F` platform adaptor |
| `reason` | why the unit has that disposition |
| `exemption` | on a shell file under `bin/`, `lib/`, `scripts/`, `share/`, `.githooks/` or `.claude/hooks/`: `why` shell is required, and the `removal` condition |
| `facts` | the migration audit's descriptive facts: purpose, callers, external commands, platform assumptions |

Nothing mechanical is recorded — no line count, no interpreter — because
`majordomus shell check` derives those from the tree and a recorded copy would go stale
without anyone noticing. The check refuses a shell unit with no exemption, a record whose
unit is gone, an exemption missing `why` or `removal`, a disposition outside A-F, a duplicate
and a record out of order; `majordomus shell check --canonical` restores the order.

The list only shrinks. A migrated unit's record is deleted in the same change that deletes
the unit, and a new shell unit is a scripted or typed capability instead — or a new record
whose exemption a reviewer reads. `docs/SHELL.md` is the reference.
