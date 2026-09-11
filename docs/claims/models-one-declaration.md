# Vendors and models are declared once in share/models.yaml, and the CLI, HTTP, OpenAPI, MCP and Cockpit render that one declaration with no model name of their own

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
catalogue as an answer. ADR 0044 records the decision and why the word "provider"
stays spent.
