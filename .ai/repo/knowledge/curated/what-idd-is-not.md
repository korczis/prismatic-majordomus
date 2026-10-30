---
schema: knowledge/v1
id: what-idd-is-not
kind: knowledge
class: convention
title: "Intent-driven development is not prompt engineering, issue generation, agent orchestration, a workflow engine or TDD with an LLM"
description: "When explaining IDD to another model or a reader."
status: verified
epistemics: inferred
date: 2026-10-18
tags:
  - idd-plan
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/CONCEPTS.md
    - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - type: relates_to
    target: file:.ai/repo/adrs/0191-a-plan-drafted-from-an-intent-is-a-proposal-checked-by-the-plan.md
  - type: relates_to
    target: file:.ai/repo/adrs/0192-replanning-is-bounded-idempotent-and-never-weakens-its-intent.md
---

# Intent-driven development is not prompt engineering, issue generation, agent orchestration, a workflow engine or TDD with an LLM

A prompt may create an intent but is not its lifecycle; an issue is an execution artifact of an intent; IDD works with one agent or a human implementer; TDD, BDD and specs remain subordinate mechanisms that answer whether code works, not whether the intended outcome was reached. Evidence strength varies (test, measurement, observation, judgement) and is represented as such. IDD does not promise optimal solutions, only that success is not self-declared.

## Sources

- `chatgpt-majordomus:c196#3`
- `chatgpt-majordomus:c270#1`
