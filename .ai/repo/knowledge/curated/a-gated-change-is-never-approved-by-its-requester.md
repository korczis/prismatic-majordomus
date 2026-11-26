---
schema: knowledge/v1
id: a-gated-change-is-never-approved-by-its-requester
kind: knowledge
class: constraint
title: "A change held for approval is never approved by the session that asked for it"
description: "When a session meets an approval gate (an owner's queue, a review request, a pending permission) on a change it requested, and the approving call is within its reach"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - approval
  - governance
  - multi-session
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/a-standing-approval-is-written-where-permissions-are-read.v1.md
    - file:.ai/repo/rules/project/review-is-recorded-not-claimed.v1.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/a-standing-approval-is-written-where-permissions-are-read.v1.md
  - type: relates_to
    target: file:.ai/repo/rules/project/review-is-recorded-not-claimed.v1.md
---

# A change held for approval is never approved by the session that asked for it

An approval gate exists so that someone other than the requester decides. A session that can
reach the approving call (an HTTP endpoint, a CLI verb, a button) and uses it on its own request
has removed the gate while leaving it in place on paper: the record says "approved" and nobody
approved.

Observed on 2026-11-25 in an adopter repository that runs a local control panel. Changes an
AI player asks for are queued for the owner's approval behind an `approve` endpoint the session
itself could call. The owner's rule there: a session never approves its own queued request,
not even to test a settings change. The sanctioned paths are a test-only settings input that
bypasses the queue by construction and, for the owner's own subjects only, a countdown the
owner can cancel, with a default the owner chose. A blanket auto-approve is refused as well.

What to do:

- Test a gated behaviour through an input built for tests, never by approving your own request.
- When a gate blocks progress, ask whoever owns it, and record the answer in the repository.
- An automated approval is legitimate only when the owner declared it: scoped to named
  subjects, cancellable, with a visible default. It is then the owner's decision, executed.
- Treat "the call would succeed" as no evidence that it is yours to make.
