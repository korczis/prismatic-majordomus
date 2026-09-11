+++
title = "A routing decision names why the selected model was selected and, for every excluded model, the first check it failed — the same answer for the same question, with no hidden state"
description = "Ask models.route which model satisfies a need — required capability words, a minimum"
weight = 169
[extra]
claim_id = "models-routing-explains-itself"
status = "guaranteed"
source = "docs/claims/models-routing-explains-itself.md"
+++
{% raw %}

## What it means

Ask `models.route` which model satisfies a need — required capability words, a minimum
context window, a vendor, local-only inference, or a model named outright — and the
answer carries its own explanation: the selected model with the reason, the qualifying
rest in order as the fallback chain, and every excluded model with the first check it
failed, in a fixed check order. There is no health guess, no load estimate, no hidden
score: routing is a pure function over the declared catalogue, so the same question
gets the same answer, and "why did this pick X" or "why was Z excluded" is read off
the decision instead of asked of a black box.

## How it works

`models::route` (`apps/majordomus-cli/src/models/mod.rs`) walks the catalogue in
declaration order; `disqualify` applies the checks — named-outright, lifecycle,
vendor, locality, capabilities, context — and returns the first failure as the
exclusion reason. A model named outright is still checked, so an override that cannot
do the work is an exclusion with a reason, never a silent selection; deprecated and
retired models qualify only when named. The unit tests prove the preference order,
every reason string, the override semantics and the empty-catalogue answer;
`test/cases/131_models.sh` proves the same through the command line in text and JSON.
{% endraw %}
