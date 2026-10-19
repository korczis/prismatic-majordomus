---
schema: adr/v1
id: adr-0188
kind: adr
title: "A rule's proof of effect is a recorded refusal of its violation, not the presence of its wiring"
status: proposed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0048-a-rule-declares-how-it-is-enforced-and-review-is-one-of-the-three.md
  - file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
  - rule:project.rule-is-a-doctrine
  - rule:project.blocking-checks-cheap
  - file:apps/majordomus-cli/src/rules/mod.rs
  - file:test/cases/18_doctrine_wiring.sh
  - file:.ai/repo/project/issues/I3110.yaml
  - file:.ai/repo/project/issues/I3111.yaml
  - file:.ai/repo/project/issues/I3112.yaml
  - file:.ai/repo/project/milestones/a-claim-stands-only-on-its-evidence.yaml
  - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
  - file:.ai/repo/knowledge/curated/declared-is-not-enforced.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0048-a-rule-declares-how-it-is-enforced-and-review-is-one-of-the-three.md
    - file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
    - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
    - file:apps/majordomus-cli/src/rules/mod.rs
    - file:test/cases/18_doctrine_wiring.sh
    - file:.ai/repo/project/issues/I3110.yaml
    - file:.ai/repo/project/issues/I3111.yaml
    - file:.ai/repo/project/issues/I3112.yaml
    - file:.ai/repo/project/milestones/a-claim-stands-only-on-its-evidence.yaml
    - file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
    - file:.ai/repo/knowledge/curated/declared-is-not-enforced.md
---

# 188. A rule's proof of effect is a recorded refusal of its violation, not the presence of its wiring

## Context

ADR 0048 makes a rule declare its enforcement mode, and rules/mod.rs reads proof from named, present, recorded cases. Doctor's wiring check greps for a dispatch string. The 2026-10-08 audit showed a validator changed to "|| exit 0" passing doctor, and a rule backed only by happy-path cases reading proven.

## Decision

A blocking rule is satisfied only by a passing recorded run of a case that shows its violation refused, and that case names the rule with a '# majordomus-refutes: <rule-id>' header; '# majordomus-negative:' keeps its command vocabulary and is not rule proof. A dispatched validator is shown, in CI, to refuse a declared violation fixture. Doctor keeps the cheap wiring check and says that wiring is all it checks.

## Alternatives rejected

- A separate "rules self-check" command and engine (a second engine; rule project.no-new-nouns)
- Doctor running every validator on every commit (rule project.blocking-checks-cheap)
- Counting the named test as proof regardless of what it asserts

## Consequences

Most blocking rules start in a ratcheted refusal-not-shown list; the list only shrinks. RuleProof gains one field. Case 18 remains the proof that the wiring verifier works.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c115#1`
- `chatgpt-majordomus:c115#4`
- `chatgpt-majordomus:c129#3`
- `chatgpt-majordomus:c139#1`
- `chatgpt-majordomus:c193#1`
- `chatgpt-majordomus:c207#6`
- `chatgpt-majordomus:c214#2`
- `chatgpt-majordomus:c259#6`
- `chatgpt-majordomus:c109#7`
- `chatgpt-majordomus:c130#13`
- `chatgpt-majordomus:c135#1`
- `chatgpt-majordomus:c139#5`
- `chatgpt-majordomus:c144#7`
- `chatgpt-majordomus:c217#5`
- `chatgpt-majordomus:c241#4`
- `chatgpt-majordomus:c269#5`
- `chatgpt-majordomus:c193#0`
