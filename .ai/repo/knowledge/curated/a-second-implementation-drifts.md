---
schema: knowledge/v1
id: a-second-implementation-drifts
kind: knowledge
class: lesson
title: "Two implementations of one verdict drift; delete one or test their parity"
description: "When shell and Rust both implement an operation"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.github/workflows/github.yml
relations:
  - type: documents
    target: file:.ai/repo/adrs/0184-every-command-name-is-implemented-by-exactly-one-executable-and.md
  - type: documents
    target: file:.ai/repo/project/issues/I3156.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I3159.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3145.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/repository-automation-is-typed.yaml
  - type: relates_to
    target: file:lib/machine_lock.sh
  - type: relates_to
    target: file:apps/majordomus-cli/src/offload/queue.rs
  - type: relates_to
    target: file:apps/majordomus-cli/src/worktree/lock.rs
---

# Two implementations of one verdict drift; delete one or test their parity

.github/workflows/github.yml records that on 2026-10-12 the shell reading of the model reported 318 drift findings where the Rust executable reported 26. The workflow now forces the Rust executable, which fixes the symptom but not the class. The class goes away only when the second path is deleted, or while both exist, when a parity case compares them. Consolidation is measured by fewer independent definitions of one operation, not by scripts rewritten.

## The same class in mechanisms, not only in verdicts (2026-11-25)

A near-copy of a mechanism drifts the same way a second implementation of a verdict does.
On 2026-11-25 one shared workstation lost most of a day to locks:

- A script crashed after `mkdir`, so a build slot had no owner file.
- A slot outlived the process that owned it.
- A derive lock waited 80 minutes at 0% CPU behind the dead slot.
- One session's 30-second poller lost the slot three times to faster pollers, because there
  was no queue.

The tree held three lock mechanisms that each solved part of this and none of the rest:

- the machine derive lock in `lib/machine_lock.sh`, an owner-checked directory;
- the offload host's FIFO queue in `apps/majordomus-cli/src/offload/queue.rs`, with tickets
  and dead-holder removal;
- the worktree lock in `apps/majordomus-cli/src/worktree/lock.rs`.

The adopter's hand-made build slots were a fourth.

- Before writing a mechanism, search for the ones that already do part of it, and name them in
  the design.
- Extend one and retire the others, or put a parity test on the shared contract: the owner
  record, the dead-owner test, queue fairness. Never add a fifth.
- A review question that catches this: which existing code already decides who holds this
  resource?

## Sources

- `chatgpt-majordomus:c244#3`
- `chatgpt-majordomus:c244#20`
