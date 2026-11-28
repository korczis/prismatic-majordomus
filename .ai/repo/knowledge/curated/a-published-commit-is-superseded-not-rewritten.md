---
schema: knowledge/v1
id: a-published-commit-is-superseded-not-rewritten
kind: knowledge
class: convention
title: "A pushed commit that must change is superseded by a new branch and pull request, never force-pushed"
description: "When a commit already pushed to a shared branch or an open pull request has to be reworded, split or dropped"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - commit
  - integration
  - history
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/skills/rebaser/SKILL.md
    - file:docs/INTEGRATION.md
relations:
  - type: documents
    target: file:.ai/repo/skills/rebaser/SKILL.md
  - type: relates_to
    target: file:docs/INTEGRATION.md
  - type: relates_to
    target: file:docs/COMMIT.md
---

# A pushed commit that must change is superseded by a new branch and pull request, never force-pushed

A published branch is held by more than its author. The forge's pull request holds it, CI
runs hold it, a peer may have merged it, and the integration engine has assessed it. Rewriting
it in place invalidates every one of them silently, and a protected branch refuses the force
push anyway.

Observed on 2026-11-25 in an adopter repository. A commit message the forge's checker refused
had to be reworded after the push. The force push was refused. The repair was a new branch
carrying the same trees with the reworded messages, and a new pull request that superseded the
old one. Nothing supported that flow, so it was done by hand.

The flow (docs/COMMIT.md, "Superseding instead of rewriting"):

1. `majordomus commit reword <base>..<head> --message <rev>=<file> ...` replays the range onto
   a new branch. Each commit keeps its tree, its trunk merges and its author. Every message is
   judged against the policy first. The old ref is never moved.
2. Push the new branch. Open the successor pull request with the old body, a `Supersedes #N`
   line on its own and the old link.
3. Take the old pull request out of auto-merge and back to draft, so that nobody merges it by
   habit.
4. The integration engine holds the old one `waiting_for_dependency` while the successor is
   open. `majordomus prs cleanup --apply` closes it once the successor lands. If the successor
   is abandoned, the old one is still there.

Closing the old pull request at once was rejected: it throws away the fallback and the review
context while the successor has not yet passed CI.
