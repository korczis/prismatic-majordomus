+++
title = "Reasoning that works with or without advisors, and review that is recorded rather than claimed"
description = "share/advisors.yaml declares advisory roles by reference to the provider table and the model catalogue; availability is derived from presence and recorded outcomes, with no network; a pure policy plans review by capability and treats no advisor as a structured local review; one writer records assessments, consultations, disagreements settled by experiment, conclusions with computed review counts, and validations; and the state reaches the command line, HTTP, MCP, the Cockpit, the environment banner, doctor, the context briefing and the derived handover."
weight = 47
[extra]
id = "reasoning"
status = "stable"
source = ".ai/repo/features/reasoning.md"
+++
{% raw %}

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
{% endraw %}
