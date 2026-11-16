---
schema: adr/v1
id: adr-0182
kind: adr
title: "A closed session record is judged by what it can attribute, not only by its schema"
status: proposed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0046-a-working-context-is-derived-and-a-closed-record-is-composed.md
  - file:.ai/repo/adrs/0164-a-record-names-the-session-that-wrote-it.md
  - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
  - rule:project.episode-record-carries-its-episode
  - file:.ai/repo/project/issues/I3146.yaml
  - file:.ai/repo/project/milestones/the-session-domain-is-written-once.yaml
  - file:.ai/repo/project/issues/I3142.yaml
  - file:.ai/repo/project/intents/a-session-record-says-what-it-served.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0046-a-working-context-is-derived-and-a-closed-record-is-composed.md
    - file:.ai/repo/adrs/0164-a-record-names-the-session-that-wrote-it.md
    - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
    - file:.ai/repo/project/issues/I3146.yaml
    - file:.ai/repo/project/milestones/the-session-domain-is-written-once.yaml
    - file:.ai/repo/project/issues/I3142.yaml
    - file:.ai/repo/project/intents/a-session-record-says-what-it-served.yaml
---

# 182. A closed session record is judged by what it can attribute, not only by its schema

## Context

session/v1 permits empty task, issue and milestone arrays; the validator checks shape. The body is now composed (ADR 0046) but a record that changed files with no attribution still validates clean (2026-09-11).

## Decision

Every closed record carries a derived completeness verdict — complete, incomplete with named reasons, or legacy-unverifiable — computed outside the schema from the record, the ledger and git; episode closed, task completed, verified and accepted are four facts and the verdict never conflates them. The schema stays session/v1.

## Alternatives rejected

- A session/v2 schema other writers could bypass
- Filling empty arrays with placeholder text
- An LLM-written summary as the evidence of completeness

## Consequences

doctor, check and finish read the verdict; historical records are legacy-unverifiable, not rewritten.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c116#4`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c116#3`
