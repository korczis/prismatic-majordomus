---
schema: knowledge/v1
id: declared-is-not-enforced
kind: knowledge
class: lesson
title: "Declaring, linking or repeatedly re-requesting an invariant does not enforce it; only a gate that refuses its violation does"
description: "When a demand recurs across sessions or a subsystem is \"connected\" but nothing refuses work that skips it"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0048-a-rule-declares-how-it-is-enforced-and-review-is-one-of-the-three.md
    - file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0048-a-rule-declares-how-it-is-enforced-and-review-is-one-of-the-three.md
  - type: relates_to
    target: file:.ai/repo/adrs/0061-a-gate-s-verdict-is-evidence-and-a-rule-the-repository-enfor.md
  - type: relates_to
    target: file:.ai/repo/adrs/0188-a-rule-s-proof-of-effect-is-a-recorded-refusal-of-its-violation.md
  - type: relates_to
    target: file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3110.yaml
---

# Declaring, linking or repeatedly re-requesting an invariant does not enforce it; only a gate that refuses its violation does

From 2026-09-10 the owner re-requested the same invariants (rules enforced, sessions recorded, versioning automatic, work deployed) in new prompt packs for weeks. Each time the cause was the same: the invariant was written in a rule or prompt but not on the chain declaration, validator, dispatch, non-zero exit, test, CI. On 2026-10-10 versioning was the example: finish, doctor and generate --check all existed, but no merge-implies-bump invariant did. Subsystems that link but do not condition each other leave sediment, and a system can look perfect while it only synchronises wrong claims. What repeats must become structure, then validation, then enforcement.

## Sources

- `chatgpt-majordomus:c098#1`
- `chatgpt-majordomus:c115#1`
- `chatgpt-majordomus:c119#3`
- `chatgpt-majordomus:c129#3`
- `chatgpt-majordomus:c135#1`
- `chatgpt-majordomus:c139#1`
- `chatgpt-majordomus:c147#2`
- `chatgpt-majordomus:c149#1`
- `chatgpt-majordomus:c179#1`
- `chatgpt-majordomus:c179#3`
- `chatgpt-majordomus:c214#2`
- `chatgpt-majordomus:c231#2`
