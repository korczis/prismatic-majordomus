---
id: project.a-check-states-its-denominator
version: 1
kind: rule
title: A check states how much it examined, and zero is a failure unless zero was declared
description: A verdict without a count cannot be told apart from a verdict over nothing; every check names the size of what it examined, and an empty examination fails unless the check declares in advance that empty is a legitimate answer.
statement: A check reports the size of what it examined beside its verdict, and an examination of nothing fails unless the check declared that nothing was expected.
status: active
class: blocking
depends_on: [project.empty-is-not-failure@1, project.finding-carries-reproduce@1]
tags: [ci, evidence, safety]
---

# Rationale

In four hours on 2026-09-10 this repository found four checks that were reporting success over
nothing. None was a new defect; each had been passing, in that state, for as long as anyone had
been reading it.

- **A site probe reported `OK — 129 route×width measurements without horizontal overflow`** while
  215 of the pages it visited had never loaded. The base URL had stopped carrying a path, the
  derived prefix emptied, the build mounted one directory too deep, and the readiness guard asked
  for `/` — which a directory listing satisfies. The overflow line was true of the pages it
  measured. It measured none of them.
- **A prefix check vanished from its own output.** Wrapped in `if [ -n "$PREFIX" ]`, an empty
  prefix made it neither pass nor fail; it stopped existing, and the verdict still read as
  complete. That is worse than the first: a vacuous `OK` is at least a line somebody can question.
- **An internal-link crawl had never matched a single link, in either configuration.** It greped
  for root-absolute hrefs while the generator emits absolute ones. The custom domain did not break
  it; it changed which of two ways it was already empty. It had reported `every internal link
  resolves` for its whole life.
- **A published-site gate printed how far behind it was inside a pass**: `ok … (86 commit(s)
  behind its head)`. It asked whether the published commit was *on* the trunk and never whether it
  was the trunk's head.

The shape is one shape. **A verdict is a claim about a set, and without the size of that set the
claim is unfalsifiable.** `OK` over six hundred routes and `OK` over zero routes are the same three
characters. Every one of these would have been caught on the day it was written — not by a better
reviewer, but by the check saying `0`.

This is the sibling of `project.empty-is-not-failure`, and it is not the same rule. That one says
absence is a legitimate answer and must be reported as absence rather than as an error. This one
says the *reporting* has to happen: an empty result is a fact the reader is entitled to, and
silence about it is what lets an empty result masquerade as a full one. A check may legitimately
examine nothing — but then it says so, and says it was expected.

# Required behaviour

**A check reports the size of what it examined, beside its verdict.** Not only on failure: the
count belongs in the passing line, because the passing line is where a vacuous result hides.

**An examination of nothing fails**, unless the check declares in advance that nothing is a
legitimate answer for that subject — in which case it says so in the verdict, in words, rather
than passing silently.

**A check that derives its own scope states what it derived it from.** Every instance above
computed the set it would examine — a prefix, a route list, a pattern — from something that could
become empty. Where the scope is derived, the derivation is part of the verdict.

**A conditional check says when it did not run.** A check skipped by a guard reports that it was
skipped and why. Disappearing from the output is not an outcome; it is the absence of one, and a
reader cannot tell it from a check that never existed.

# Failure behaviour

The gates are held to this by review and by their own cases. A gate cannot be written that reads
every other gate's prose, and one that tried would be the second inventory this repository forbids.
What can be held mechanically is each check's *own* case: a check that claims to examine a set is
given an empty set and must fail, and given a non-empty one must report its size.

The rule is `blocking` because the cost is asymmetric. A check that over-reports its work is a
minor annoyance; a check that under-reports it is a gate everyone believes and nobody is running,
and the four instances above went unnoticed for weeks precisely because nothing about their output
invited doubt.

# Verification

Each check carries a case that proves it non-vacuous — the pattern this repository already uses
elsewhere, where a case is proved by removing the refusal and watching the case fail. Applied here:
the check is given an empty subject and must not pass. `test/cases/97_site_probe_attribution.sh`
and the site-check cases are the worked examples; the fixtures deliberately include the empty case
rather than only the interesting one.
