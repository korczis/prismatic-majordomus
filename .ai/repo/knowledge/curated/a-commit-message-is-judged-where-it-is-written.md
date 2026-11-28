---
schema: knowledge/v1
id: a-commit-message-is-judged-where-it-is-written
kind: knowledge
class: lesson
title: "A commit message is judged against the repository's own grammar when it is written, not first by the forge after the push"
description: "When a repository's commit messages follow a grammar (conventional or its own) that a forge check or a reviewer enforces, and when adopting Majordomus in a repository whose history is not conventional"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - commit
  - hooks
  - adoption
provenance:
  origin: extracted
  derived_from:
    - file:docs/COMMIT.md
    - file:.ai/repo/project/issues/I4262.yaml
relations:
  - type: documents
    target: file:docs/COMMIT.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I4262.yaml
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/a-published-commit-is-superseded-not-rewritten.md
---

# A commit message is judged against the repository's own grammar when it is written, not first by the forge after the push

A message is cheap to fix before `git commit` stores it. Once it is pushed it is part of a
published history: fixing it costs a rewrite, and a protected branch refuses that, so the
branch has to be superseded. The forge check is the authority, but it is the last place
to learn about a defect, not the first.

Observed on 2026-11-25 in an adopter repository, a fork that keeps its upstream's grammar
`<Keyword>( #<issue>|<commit>, ...)?: [<Component>] <Details>`:

- `majordomus commit validate` knew only the conventional grammar. It refused every valid
  message there (`Fix #12: [YAPF] Infinite loop in pathfinder` gave `commit.not_conventional`),
  so the adopter never wired the `commit-msg` hook.
- A session committed `Test: [Component] ...`, and `Test` is not a keyword of that grammar.
  The forge's commit checker found it only after the push.
- Force-push was refused, so the pull request had to be superseded by hand. One such commit
  had already reached the adopter's trunk.
- In the same session a pre-commit gate caught a stale line citation at commit time, which
  is the moment the message also deserved.

What follows:

- A repository declares its grammar in policy (`commit.grammar`, docs/COMMIT.md), and the
  `commit-msg` hook judges each message against it before git stores it.
- A hook that refuses every valid message is worse than none, because people stop wiring it.
  Declare the grammar first, measure the history with `majordomus commit history`, and only
  then wire the hook.
- The hook can be bypassed (`--no-verify`, `git commit-tree`, tools). A pushed-range check
  before the push and the forge check after it remain the backstops.
