---
schema: knowledge/v1
id: not-run-on-master-means-the-ledger-lacks-ci-runs
kind: knowledge
class: lesson
title: "\"Not run\" on master means the tracked ledger lacks CI's executions, not that the tests never ran"
description: "When a rule, claim or feature reads not_run on master"
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
    - file:.ai/repo/evidence/ledger.json
    - file:apps/majordomus-cli/src/rules/mod.rs
    - file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0087-evidence-is-indexed-by-subject-and-judged-at-the-presented-revision.md
  - type: documents
    target: file:.ai/repo/project/issues/I3112.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0189-junit-and-cobertura-are-formats-evidence-is-imported-from-and-ex.md
  - type: relates_to
    target: file:.ai/repo/project/intents/a-claim-stands-only-on-its-evidence.yaml
---

# "Not run" on master means the tracked ledger lacks CI's executions, not that the tests never ran

Read on master 707a04c on 2026-10-18 through majordomus_rules and majordomus_evidence: the tracked ledger held 14 executions, the newest 2026-09-17. 156 of 187 rules and 214 of 229 claims read not_run, while CI runs those cases on every push. An audit on 2026-10-08 read the same gap as "0 of 206 guarantees with evidence". Once CI runs are imported (PR 1347 in the 0.30 work), not_run falls but stale rises, because evidence is not truth until each piece is classified against the revision. Before calling a rule unproven, check the ledger's newest execution. These numbers are dated observations; measure again rather than quoting them.

## Sources

- `chatgpt-majordomus:c109#5`
- `chatgpt-majordomus:c286#10`
- `chatgpt-majordomus:c193#0`
- `chatgpt-majordomus:c256#2`
