---
schema: adr/v1
id: adr-0190
kind: adr
title: "Mutation testing is scheduled evidence over declared critical predicates, never a gate on every change"
status: proposed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
  - file:docs/COVERAGE.md
  - file:.ai/repo/adrs/0188-a-rule-s-proof-of-effect-is-a-recorded-refusal-of-its-violation.md
  - file:.ai/repo/project/issues/I3120.yaml
  - file:.ai/repo/project/milestones/a-test-fails-when-the-code-is-wrong.yaml
  - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
    - file:docs/COVERAGE.md
    - file:.ai/repo/adrs/0188-a-rule-s-proof-of-effect-is-a-recorded-refusal-of-its-violation.md
    - file:.ai/repo/project/issues/I3120.yaml
    - file:.ai/repo/project/milestones/a-test-fails-when-the-code-is-wrong.yaml
    - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
---

# 190. Mutation testing is scheduled evidence over declared critical predicates, never a gate on every change

## Context

Coverage proves a line ran, not that a test would fail if it were wrong (docs/COVERAGE.md). The owner asked for mutation testing several times, and the CI pool judges one pull request at a time.

## Decision

Critical predicates are declared once in .ai/repo/ci. A scheduled or offloaded mutation run records its survivors in the ledger, and they are published. A survivor is a finding with an owner, never a red pull request.

## Alternatives rejected

- A blocking mutation-score gate per pull request (CI cost)
- Mutation over the whole crate

## Consequences

Mutation evidence is periodic and revision-stamped; claims about adversarial strength cite it.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c113#16`
- `chatgpt-majordomus:c139#5`
- `chatgpt-majordomus:c158#10`
- `chatgpt-majordomus:c158#20`
- `chatgpt-majordomus:c293#2`
