---
schema: context/v1
id: ai.repo.project.gaps
kind: context
title: Gap analyses
description: What a worker observed of this repository against an intent's criteria, at one commit.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Gap analyses

One record per intent, named by it: the observations a worker made at an `observed_at` commit,
the state of every criterion of the intent against them — `satisfied`, `missing`, `conflicting`
or `unknown` — the risks, and the work it suggests. Planning starts here rather than from a
prompt, and `majordomus intent validate` refuses a gap that leaves a criterion unanswered or
asserts a state with nothing observed behind it (ADR 0073).

Majordomus does not write these. A person or a worker does, and the repository judges them.
