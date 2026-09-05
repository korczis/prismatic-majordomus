+++
title = "Complete an issue only when its evidence exists"
description = "See an issue refuse completion while a required piece of evidence is missing, read the roadmap the milestone state derives, and keep the GitHub projection a projection."
weight = 18
[extra]
id = "complete-an-issue-only-with-its-evidence"
source = ".ai/repo/use-cases/complete-an-issue-only-with-its-evidence.md"
category = "completion"
maturity = "guaranteed"
+++

## Situation

An issue is marked done because the person doing it was done. The test it required was never written, the document it promised never appeared, and the milestone advances on a claim. A roadmap maintained by hand says something else again.

## What you run

- `plan status`, `plan show <id>`: derived status, and the evidence an issue requires
- `plan roadmap`: milestone order derived from state
- `plan body <id>`: the neutral projection body the GitHub sync renders from

## Outcome

An issue completes only when its evidence exists in the repository, status is derived and stored nowhere, and the roadmap and the GitHub issues are projections of the one model rather than second authorities.
