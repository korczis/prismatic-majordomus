---
schema: adr/v1
id: adr-0185
kind: adr
title: "A derivation is a typed graph of the generators the executable already plans, keyed by the content of what each node reads, and the full derive stays its oracle"
status: proposed
date: 2026-10-18
tags:
  - derive-performance
  - chatgpt-project
related:
  - rule:project.cache-is-invisible
  - rule:project.derived-once
  - rule:project.a-derive-sees-what-the-commit-will
  - rule:project.a-tool-that-cannot-match-the-tree-refuses
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
  - file:.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md
  - file:apps/majordomus-cli/src/generate.rs
  - file:scripts/derive
  - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/milestones/a-derive-answers-within-a-second.yaml
  - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
  - file:.ai/repo/project/issues/I3233.yaml
  - file:.ai/repo/project/issues/I3234.yaml
  - file:.ai/repo/project/issues/I3236.yaml
  - file:.ai/repo/knowledge/curated/do-less-work-before-doing-it-faster.md
  - file:.ai/repo/knowledge/curated/a-plan-on-an-unlanded-branch-is-planned-again.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
    - file:.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md
    - file:apps/majordomus-cli/src/generate.rs
    - file:scripts/derive
    - file:.ai/repo/project/intents/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/milestones/a-derive-answers-within-a-second.yaml
    - file:.ai/repo/project/milestones/a-change-is-judged-in-minutes.yaml
    - file:.ai/repo/project/issues/I3233.yaml
    - file:.ai/repo/project/issues/I3234.yaml
    - file:.ai/repo/project/issues/I3236.yaml
    - file:.ai/repo/knowledge/curated/do-less-work-before-doing-it-faster.md
    - file:.ai/repo/knowledge/curated/a-plan-on-an-unlanded-branch-is-planned-again.md
---

# 185. A derivation is a typed graph of the generators the executable already plans, keyed by the content of what each node reads, and the full derive stays its oracle

## Context

Owner asks on 2026-10-11 (Merkle/DAG/hash), 10-15 (ten times faster; reuse the parts that exist) and 10-17 (sub-second, parallel). generate.rs already has a typed Artifact/Target plan, generation.rs a content-based executable identity, generate-site-data a batch input fingerprint. Two stage-B skips were built (I2490 with unlanded ADR 0143 (on an unlanded branch), I2880). The 10-13 plan (I2521-I2533) never landed.

## Decision

Extend the existing plan in generate.rs with declared inputs and outputs per node, rather than add an engine beside it. A node's key is a content digest over its generator identity (including the executable's generation), canonical input bytes, index membership, schema and policy versions and build parameters. Metadata (mtime, size, watcher events) only nominates candidates. A hit must be observationally equivalent to a fresh run, and a test proves it. Invalidation follows git index semantics. scripts/derive --full and derive-check stay the oracle. A reused node is a saving in a derive and never a verdict in a gate.

## Alternatives rejected

- A new Merkle/DAG/cache framework beside generate.rs (c265#8)
- Cargo or build.rs as a second derive engine (ADR 0140)
- One global lock as the only fix for repeated work (c208#4)
- mtime plus size as identity (c292#2)
- Timestamp-keyed caches (c130#3)

## Consequences

ADR 0143 (on an unlanded branch) (unlanded, I2490) becomes the first slice of this decision or is withdrawn. Rule project.cache-is-invisible gains its first real subject. Every new generator must declare its reads, and an omitted read turns a gate red (I2527).

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c130#3`
- `chatgpt-majordomus:c216#6`
- `chatgpt-majordomus:c265#8`
- `chatgpt-majordomus:c292#2`
