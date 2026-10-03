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
