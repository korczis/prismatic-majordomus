+++
title = "Decide whether finished work is actually finished"
description = "Evaluate a contract line by line instead of accepting a sentence that says the work is done."
weight = 5
[extra]
id = "accept-or-refuse-finished-work"
source = ".ai/repo/use-cases/accept-or-refuse-finished-work.md"
category = "completion"
maturity = "guaranteed"
+++

## Situation

A worker reports success. The report is a paragraph, the evidence is the paragraph, and accepting it is a matter of trust rather than of checking.

## What you run

- `question`: anything unresolved is recorded as state, and refuses completion while it stands
- `finish`: --verify-command runs the project's own verification and records its exit code and duration

## Outcome

A refusal names the rules that refused. An outcome is typed — completed, partial, blocked, no_match, failed — so "the thing does not exist" and "I could not do it" stay different facts.
