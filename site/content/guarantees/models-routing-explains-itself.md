+++
title = "A routing decision names why the selected model was selected and, for every excluded model, the first check it failed — the same answer for the same question, with no hidden state"
description = "Ask models.route which model satisfies a need — required capability words, a minimum"
weight = 185
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

## How to see it

```
majordomus models route --require vision --min-context 500000
majordomus models route --model haiku --require vision      # a refused override, with the reason
curl 'http://127.0.0.1:8741/api/v1/models/route?require=vision'
bash test/run.sh 131_models
```

## What it does not cover

The decision does not consider health, load or price — facts this tool cannot verify
have no seat at the table, and a caller who knows better simply names the model
outright (and is still checked). Recording which model *actually executed* a
session's work is the capture adapters' follow-up, not this function's promise.

## Why it exists

An unexplainable router is a black box somebody eventually distrusts and routes
around, at which point there are two routers. A pure function whose answer carries
its own reasons can be read, tested and argued with — which is what keeps it the
only one (ADR 0049).
{% endraw %}
