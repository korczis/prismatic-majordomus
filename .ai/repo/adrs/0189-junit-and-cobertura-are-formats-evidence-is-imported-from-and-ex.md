---
schema: adr/v1
id: adr-0189
kind: adr
title: "JUnit and Cobertura are formats evidence is imported from and exported to; the tracked ledger stays the only store"
status: proposed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
  - file:.ai/repo/adrs/0068-ci-evidence-is-kept-where-the-run-happened-and-published-with-the-commit-it-proves.md
  - file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
  - file:apps/majordomus-cli/src/evidence
  - file:.ai/repo/project/issues/I3124.yaml
  - file:.ai/repo/project/issues/I3125.yaml
  - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
    - file:.ai/repo/adrs/0068-ci-evidence-is-kept-where-the-run-happened-and-published-with-the-commit-it-proves.md
    - file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
    - file:apps/majordomus-cli/src/evidence
    - file:.ai/repo/project/issues/I3124.yaml
    - file:.ai/repo/project/issues/I3125.yaml
    - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
---

# 189. JUnit and Cobertura are formats evidence is imported from and exported to; the tracked ledger stays the only store

## Context

ADR 0041 rejected a JUnit XML pipeline as the evidence store, because proof is a recorded execution in the repository. The owner later asked for JUnit/Cobertura as derived formats, so that foreign projects can bring the results they already have, non-invasively.

## Decision

An importer normalises JUnit (and later LCOV and Cobertura) into ledger executions with commit and tree state, and an exporter may render the ledger as JUnit. Neither format is ever read as current proof without import. A foreign repository defaults to Observe: discover and import, never instrument.

## Alternatives rejected

- JUnit as the canonical store (ADR 0041)
- A new EvidenceProvider trait or plugin registry beside the evidence module (no new nouns)

## Consequences

Adopters get evidence without changing their CI. Imported executions are as weak as their source: no tree state means unknown, never clean.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c246#2`
- `chatgpt-majordomus:c246#4`
- `chatgpt-majordomus:c246#0`
- `chatgpt-majordomus:c246#3`
- `chatgpt-majordomus:c246#7`
