---
schema: knowledge/v1
id: a-wrong-claim-is-corrected-in-the-open
kind: knowledge
class: convention
title: "A claim you made and found wrong is corrected explicitly, where it was made, with the evidence that refutes it"
description: "When a session finds that something it reported, recorded or told a peer or the owner was wrong: a cause, a state, an attribution or a count"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - reporting
  - evidence
  - reasoning
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/review-is-recorded-not-claimed.v1.md
    - file:.ai/repo/knowledge/curated/s-20261031234907-e660-1d367279d2d5.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/review-is-recorded-not-claimed.v1.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/s-20261031234907-e660-1d367279d2d5.md
---

# A claim you made and found wrong is corrected explicitly, where it was made, with the evidence that refutes it

A wrong claim that is silently dropped keeps working: a reader who saw it acts on it, and
another session copies it into its own record. Overwriting it with the right answer is no
better, because the reader cannot tell that anything changed. A correction names the claim,
says it was wrong, gives the evidence, and says what follows from the correction.

Observed on 2026-11-25:

- An analysis of a live incident in an adopter repository ran three passes. Pass 1 used
  independent lenses. In pass 2 each lens refuted the others, with one external model. Pass 3
  synthesised only the confirmed findings.
- Pass 2 caught confident first-pass claims that were misattributions, among them "the
  approvals list was empty" and "auto-approve was a bug". Each read plausibly and was wrong,
  and the refutation pass, not the author, found it.
- The earlier record `s-20261031234907-e660-1d367279d2d5` shows the same shape inside this
  repository's ledger. A correction is a new observation, and the first record is rejected with
  a reason that names its replacement.

What to do:

- Write "Correction: I said X; that was wrong. Y shows Z. So W changes." Put it in the same
  channel as the claim: the report, the pull request, the board, the record.
- Tag every claim of an analysis as observed (with a locator), derived (with the formula) or
  inferred (with what would confirm it). Wrong claims are then found by checking, not by luck.
- Never edit a published record or report in place to hide that it was wrong.
