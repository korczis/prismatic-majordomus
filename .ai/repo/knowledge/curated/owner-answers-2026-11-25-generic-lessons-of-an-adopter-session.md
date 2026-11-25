---
schema: knowledge/v1
id: owner-answers-2026-11-25-generic-lessons-of-an-adopter-session
kind: knowledge
class: fact
title: "Owner answers 2026-11-25: implement the generic lessons of an adopter session in Majordomus (all four selected, decided after several passes with other models), and evolve Majordomus from everything that session learned"
description: "When a session asks whether the owner ordered build-slot and lock work, owner answers recorded in the repository, plan reconciliation with merged pull requests, or baseline-relative gates as generic Majordomus work on 2026-11-25, or how the designs were to be reached (several passes with other models)"
status: verified
epistemics: decided
date: 2026-11-25
tags:
  - decision
  - owner-answer
  - plan
  - evolve
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/adrs/0238-the-plan-is-reconciled-with-merged-work-and-a-claim-is-refused-where-it-collides.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0238-the-plan-is-reconciled-with-merged-work-and-a-claim-is-refused-where-it-collides.md
---

# Owner answers 2026-11-25: the generic lessons of an adopter session become Majordomus work

On 2026-11-25 the owner answered two questions about work in an adopter repository. That
repository ran many concurrent sessions on one shared workstation, a live game simulation and a
plan record that fell behind its merges. Both answers are reproduced verbatim below, with their
translation. Neither names the adopter, and nothing in this repository depends on it.

## Answer 1: which lessons to implement

- Session: Claude Code `7709ab47-7786-494d-bbcc-679b293569a7`.
- Asked through AskUserQuestion at about 06:50 CET on 2026-11-25.
- Question, translated from Czech: "Yes. From this session I've distilled these generic lessons for
  prismatic-majordomus (nothing project-specific). Which should I implement there? (Each one first as
  a design for review, then implementation with tests in that repo's own workflow.)"
- Options:
  - "Build slots and locks"
  - "Owner answers in the repo"
  - "Plan ↔ merged PRs"
  - "Baseline-relative gates"
- Answer: all four selected, plus this free text, verbatim: "porad se s ostatnimi ai/llm, na vice
  pruchodu a vymslete nejlepsi reseni". Translation: "consult the other AIs/LLMs, in several passes,
  and work out the best solution".

## Answer 2: evolve Majordomus from the whole session

- Same session, about 07:40 CET.
- Verbatim: "muzes na zaklade vseho z teto sesion, z lessons learned, obecne / dynamicky zlepsi
  ~/dev/prismatic-majordomus /evolve it? proste pouzit to co jsme se naucili zde, v tomto projektu
  pouzit ke zlepseni generickeho majordomus?"
- Translation: "Based on everything from this session and its lessons learned, can you generically
  and dynamically improve ~/dev/prismatic-majordomus, evolve it? Simply use what we learned here, in
  this project, to improve the generic Majordomus."

## What follows from them

- **Each lesson is generic Majordomus work, split across sessions.**
  - The plan-reconciliation lesson is ADR 0238 with issues I4773, I4775 and I4777. Its design
    records the advisors' positions: what was taken, what was rejected, and why.
  - An addendum to the same order adds that a plan issue must not be executed twice. In the adopter,
    the second session's picker ran before the first session's pull request existed. The fix is a
    check, right before `start` and before a pull request, that no open pull request, branch or
    session already claims the issue. That is I4775.
- **"Several passes with other models" is a procedure, not a vote.**
  1. A written design.
  2. Independent critique by at least two advisors that were given the problem and the code, not the
     conclusion.
  3. A revision, then a second round of critique, then the decision. The advisors and their
     positions are kept with the design.

  An advisor that cannot be reached is recorded as such (rule `project.advisors-are-optional`).
