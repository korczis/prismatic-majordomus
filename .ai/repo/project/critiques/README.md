---
schema: context/v1
id: ai.repo.project.critiques
kind: context
title: Plan critiques
description: The adversarial pass over an intent's plan, as structured findings with their resolutions.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Plan critiques

One record per intent, named by it: what a review of the plan found — a missed requirement, an
unproven assumption, insufficient or unnecessary work, a regression risk, a surface that must
also change, a deployment or runtime verification that is required — each finding blocking or
not, and each resolved `open`, `planned` into an issue that serves the intent, or `rejected`
with a reason.

Work serving an intent does not start while the intent has no critique, or while a blocking
finding is open: `majordomus intent validate` refuses it (ADR 0073).

A reviewer writes the findings; the tool runs the structural half of the review and stamps
the record with the plan it was run over (ADR 0112):

```bash
majordomus-cli intent oppose <intent>   # the brief, the findings of both halves, the disposition
majordomus-cli intent stamp <intent>    # reviewed_revision, reviewed_at, reviewed_with: three lines
```

`reviewed_revision` and `reviewed_with` are written by `intent stamp` and never by hand. A
finding may name its `source`, and a resolution who made it (`resolved_by`); classes
`invariant_conflict` and `dependency_order` exist beside the seven above. A critique stamped
against a plan that has since changed reviewed another plan, and refuses the work it once
authorised.
