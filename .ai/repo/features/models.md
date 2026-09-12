---
schema: feature/v1
id: models
kind: feature
title: The models the world offers, declared once and routed with reasons
short_title: Models
headline: One declared catalogue of vendors and models — capabilities, context, lifecycle, no fabricated facts — and a routing answer that names why every candidate was chosen or excluded.
summary: share/models.yaml declares vendors and models in one place, in routing's preference order, with provenance stated and pricing deliberately absent; models.list and models.route project the catalogue and its explainable routing to the CLI, HTTP, OpenAPI, MCP and the Cockpit, credential presence is reported without any value being read, and nothing anywhere calls a model — the crate's no-network boundary stands.
status: stable
weight: 46
featured: true
areas: [coordination]
modules: [models]
rules: []
docs: [docs/MODELS.md]
adrs: [adr-0049]
claims: [models-one-declaration, models-routing-explains-itself, models-no-secret-fields]
use_cases: []
cockpit: [models]
related: [mesh]
tags: [models, catalogue, routing]
---

## What it does

`share/models.yaml` declares the vendors and models this tool's world can name:
canonical ids, the vendors' native ids, aliases, typed capability words, context
windows and lifecycle standings, in an order that is also routing's preference.
`majordomus models list` renders the catalogue with each vendor's
credential-presence; `majordomus models route --require vision,tools --min-context
500000` answers which model qualifies first, which stand behind it as the fallback
chain, and why every other model fell out — the same answer over HTTP, OpenAPI, the
MCP tools and the Cockpit's Models page, because all of them project the same two
capabilities. The catalogue's own findings — a duplicate alias, an undeclared
vendor — ride along in every listing.

## What it does not do

It calls no model and reads no secret: credential facts are the *presence* of a named
environment variable, never a value, and the catalogue's schema has no field a secret
could hide in (a test holds it). It states no price and no unverified vendor — what
cannot carry provenance stays absent. And it does not yet record which model actually
executed a session's work; that belongs to the capture adapters, whose schema already
declares the fields (ADR 0049 names it as the follow-up).
