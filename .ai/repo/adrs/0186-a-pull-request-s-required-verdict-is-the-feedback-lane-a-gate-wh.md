---
schema: adr/v1
id: adr-0186
kind: adr
title: "A pull request's required verdict is the feedback lane, a gate whose inputs did not change is reused as inputs_unchanged, and the proof lane judges the combined top before it lands"
status: proposed
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
  - file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
  - file:.ai/repo/adrs/0129-a-train-of-pull-requests-lands-in-one-run-of-the-required-check.md
  - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
  - rule:project.blocking-checks-cheap
  - file:.ai/repo/ci/gates.yaml
  - file:scripts/ci-plan
  - file:.ai/repo/adrs/0185-a-derivation-is-a-typed-graph-of-the-generators-the-executable-a.md
  - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/issues/I3240.yaml
  - file:.ai/repo/project/issues/I3242.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
    - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
    - file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
    - file:.ai/repo/adrs/0129-a-train-of-pull-requests-lands-in-one-run-of-the-required-check.md
    - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
    - file:.ai/repo/ci/gates.yaml
    - file:scripts/ci-plan
    - file:.ai/repo/adrs/0185-a-derivation-is-a-typed-graph-of-the-generators-the-executable-a.md
    - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/issues/I3240.yaml
    - file:.ai/repo/project/issues/I3242.yaml
---

# 186. A pull request's required verdict is the feedback lane, a gate whose inputs did not change is reused as inputs_unchanged, and the proof lane judges the combined top before it lands

## Context

Owner orders of 10-08, 10-10 (under 5 min) and 10-11. One validate serves three time horizons: a 10-12 min structure job, four shards of 94-148 min, and a 39-min ui-audit (c228#3, dated). ADR 0041 keeps proven and inputs_unchanged as two states. ADR 0167 lands work in combined tops.

## Decision

Each gate declares a lane. The required pull-request verdict needs every feedback-lane gate green and every other selected gate either run or reused as inputs_unchanged: a recorded passing result whose inputs, toolchain and meaning are byte-identical. The proof lane (full suite, full browser sweep, macOS, coverage, installer) is required on the combined top before landing and on release. Unknown impact selects everything. The target is 300 s p95 from validation start, queue time apart.

## Alternatives rejected

- More runners, timeouts or shards first (c228#3)
- Moving required validation to nightly or making it non-blocking (c195#2)
- Recomputing every proof on every pull request (c228#3)
- Running every gate for every intent (c130#3)

## Consequences

I2348 and I2350 become the implementation. The landing seat's combined-top run carries the proof. A proof-lane red blocks the next landing (rule project.never-reported-is-not-green applies to every skipped gate's proof).

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c130#3`
- `chatgpt-majordomus:c228#3`
