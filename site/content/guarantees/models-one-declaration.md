+++
title = "Vendors and models are declared once in share/models.yaml, and the CLI, HTTP, OpenAPI, MCP and Cockpit render that one declaration with no model name of their own"
description = "Which models exist, what each can do, how large its context is, and which name is an"
weight = 180
[extra]
claim_id = "models-one-declaration"
status = "guaranteed"
source = "docs/claims/models-one-declaration.md"
+++
{% raw %}

## What it means

Which models exist, what each can do, how large its context is, and which name is an
alias of which are facts stated exactly once, in `share/models.yaml`. `majordomus
models list`, `GET /api/v1/models`, the `majordomus_models` MCP tool, the
`majordomus://models` resource, the OpenAPI document and the Cockpit's Models page are
projections of that file — none carries a model name of its own, so adding or
retiring a model is one edit and every surface follows. The catalogue's own findings —
a duplicate alias, a model naming an undeclared vendor — ride along in every listing,
reported at first read rather than discovered downstream.

## How it works

`models.list` and `models.route` are `capability!` declarations
(`apps/majordomus-cli/src/capability/builtin/models.rs`), so every transport
projection is derived (ADR 0004); the catalogue type and its loader live in
`apps/majordomus-cli/src/models/mod.rs`, reading the share directory through the same
resolution everything else uses. Declaration order is routing's preference order, a
property a person verifies by reading the file. `test/cases/131_models.sh` proves the
operator path — one declaration rendered, presence-only credentials, an empty
catalogue as an answer. ADR 0049 records the decision and why the word "provider"
stays spent.

## How to see it

```
majordomus models list                   # the whole catalogue, one declaration
curl http://127.0.0.1:8741/api/v1/models
grep -c "id:" share/models.yaml          # the same count the surfaces show
bash test/run.sh 131_models
```

## What it does not cover

The catalogue does not verify that a declared model still exists at its vendor — it
is reviewed data with stated provenance, not a live probe (ADR 0032 keeps network
clients out of the crate). Staleness is visible in the file's provenance header,
where it can be judged.

## Why it exists

Model names scattered through prompts, configs and workers' heads are a
hand-maintained truth per consumer — the exact shape this repository refuses
everywhere else. One declared file, projected everywhere, makes adding or retiring
a model one reviewable edit (ADR 0049).
{% endraw %}
