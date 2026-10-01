---
schema: feature/v1
id: reasoning
kind: feature
title: Reasoning that works with or without advisors, and review that is recorded rather than claimed
short_title: Reasoning
headline: Material uncertainty becomes an evidence-backed decision — reviewed by whichever optional advisors are available, decided locally when none are, and carried to the next session as a record.
summary: share/advisors.yaml declares advisory roles by reference to the provider table and the model catalogue; availability is derived from presence and recorded outcomes, with no network; a pure policy plans review by capability and treats no advisor as a structured local review; one writer records assessments, consultations, disagreements settled by experiment, conclusions with computed review counts, and validations; and the state reaches the command line, HTTP, MCP, the Cockpit, the environment banner, doctor, the context briefing and the derived handover.
status: stable
weight: 47
featured: true
areas: [coordination]
modules: [reasoning]
rules: [project.advisors-are-optional, project.review-is-recorded-not-claimed]
docs: [docs/REASONING.md]
adrs: [adr-0098]
claims: [reasoning-works-with-no-advisor, advisor-transports-share-one-contract, advice-is-weighed-not-counted, advisor-failure-is-recorded-and-recovers, reasoning-survives-the-session, advisors-are-declared-once, review-is-never-fabricated]
use_cases: []
cockpit: [reasoning]
related: [models, mesh]
tags: [reasoning, advisors, review]
---

## What it does

A session states what is uncertain, how much it matters and the evidence it has, and
`majordomus reasoning` plans: decide locally, reuse a standing conclusion, or consult the
available advisors that offer the review needed, in the preference order the catalogue
declares. `scripts/advisor-consult` asks them through one transport contract and records
every outcome; disagreement is settled by an experiment with evidence; the conclusion
weighs every answer and its review count is computed. The decision, who reviewed it and
what remains unresolved appear in `majordomus context` and every derived handover, and a
later session reuses the decision instead of asking again.

## What it does not do

It does not need an advisor. With none installed — or in CI, or offline — the same
workflow runs with a structured local review, and says so. It does not let advice decide:
a count of agreeing answers settles nothing. It does not keep transcripts, call a model
from the executable, or read a credential's value.
