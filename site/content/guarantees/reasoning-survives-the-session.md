+++
title = "A decision and its provenance survive the session: the next session finds them in its context and handover, reuses a standing conclusion instead of consulting again, and reopens it only on new evidence"
description = "Consultation reduces repeated reasoning instead of multiplying it. A later session reads"
weight = 221
[extra]
claim_id = "reasoning-survives-the-session"
status = "guaranteed"
source = "docs/claims/reasoning-survives-the-session.md"
+++
{% raw %}

## What it means

Consultation reduces repeated reasoning instead of multiplying it. A later session reads
what was decided, on what evidence and who reviewed it, and does not ask the same question
again unless something it did not know before has turned up.

## How it works

Records live in the checkout's reasoning store. `reasoning.status`
(`apps/majordomus-cli/src/reasoning/state.rs`) derives the report that `majordomus context`
prints under REASONING and `majordomus handover --derive` writes under Reasoning. A plan for
an assessment whose subject matches one with a standing conclusion returns `reuse_prior`
and selects nobody, unless the assessment declares `new_evidence`; the new conclusion then
names the old one in `supersedes`.

## How to see it

```
majordomus context            # ## REASONING — Decision …, reviewed by …
majordomus handover --resolve # # Reasoning
```

`test/cases/734_reasoning_survives_the_session.sh` hands over from session A to session B,
proves B consults nobody, continues and validates, and reopens on new evidence.

## What it does not cover

Subjects are matched by their normalised text, not by meaning; two phrasings of one
question are two subjects. Records are checkout state: another machine sees them through
a handover published over the mesh, not by reading the store.

## Why it exists

A decision whose reasons live only in a finished conversation is rediscovered by every
session that meets it, at the cost of another round of consultations (ADR 0098).
{% endraw %}
