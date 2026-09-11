---
id: project.session-judgement-cites-its-evidence
version: 1
kind: rule
title: A session judgement cites its evidence
description: Every judgement the continuity subsystem reports must be answerable by a provenance capability that names the file, ledger line or policy key it came from, and must report what it does not know rather than a likely-looking answer.
statement: Where the continuity subsystem reports a judgement — which episode is current, which record resolved, how old a record is, how an episode closed — a provenance capability answers why, citing a file, a ledger line or a policy key for every claim. A judgement the process cannot establish is reported as unknown with the reason it could not be established; an answer is never inferred to fill the gap.
status: active
class: blocking
depends_on: []
tags: [continuity, session, provenance, evidence]
---

# Rationale

Between 2026-09-05 and 2026-09-11 this repository told every worker who opened an episode to
continue work that had already been finished. The record it handed them was labelled
`advanced` — a true statement about git topology, and one that says nothing whatever about
age. The subsystem was not broken. It was confidently wrong, for six days, on every surface
it has, and nobody caught it because nothing could be asked *why*.

That is this subsystem's characteristic failure, and it is worse than an outage. An outage
announces itself. A briefing that is quietly about last week is acted on.

Each of these judgements is the output of a resolution rule with tiers, thresholds and
tie-breaks, and each of them was already computing the reason it decided as it did:
`mj_resolve_latest` sets `MJ_RES_MATCH` and `MJ_RES_SKIPPED`, `Thresholds::judge` returns a
sentence beside its verdict. None of it left the function. The workings existed and were
discarded at the door.

A worker who could have asked *why is this the record you are showing me* would have been
told "tier 0, the newest of eighteen, asserted six days ago" and would have stopped reading
at the third clause.

# Required behaviour

A judgement the continuity subsystem reports is answerable by a provenance capability —
`continuity.explain` today — and each answer carries:

* **the verdict**, or nothing at all when the process could not establish one;
* **a reason**, never empty, naming the field, tier or threshold that decided it;
* **evidence**: at least one file, symlink, ledger line or policy key a reader can open. An
  absence counts, and is recorded as an absence naming where it was looked for, so that "we
  looked and found nothing" stays distinguishable from "we did not look";
* **what was passed over**, with the reason each candidate lost, wherever the judgement
  chose between candidates.

Two things follow from this and are not negotiable:

**The reason is produced by the rule, not beside it.** A second function that re-derives why
a record won is a second account of one rule, and it will drift from the rule it explains —
at which point the explanation is another confident assertion, which is the thing being
fixed. The trace comes out of the selection itself.

**A threshold is cited with the declaration that owns it.** "Stale past 48 hours" is an
assertion; `.ai/repo/policy.yaml#session.freshness.stale_minutes says 2880` is a fact. A
number compiled into a reader is forbidden here for the ordinary reason it is forbidden
everywhere in this repository, and for one more: the answer would cite a policy key while
having judged against something else.

# Failure behaviour

Where a judgement cannot be established — a legacy prompt record carrying no episode, an
episode with no closed record in this clone, a policy that predates `session.freshness` —
the answer reports it as unknown, with the cause, and answers nothing. It does not fall back
to a default, to the current episode, or to the likely one.

Absence is an answer. Invention is not, and a plausible explanation is worse than none.

# Verification

`test/cases/270_a_session_explains_itself.sh`: every question carries a reason and at least
one piece of evidence of a declared kind; an unknown answer is empty; a question or an
episode that does not exist is refused rather than silently answered about something else;
the 2026-09-05 record is explained as selected *and* stale, with the threshold traced to the
file that declares it. The unit tests in
`apps/majordomus-cli/src/capability/builtin/continuity.rs` hold the same invariants against
the handler directly.
