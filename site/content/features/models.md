+++
title = "The models the world offers, declared once and routed with reasons"
description = "share/models.yaml declares vendors and models in one place, in routing's preference order, with provenance stated and pricing deliberately absent; models.list and models.route project the catalogue and its explainable routing to the CLI, HTTP, OpenAPI, MCP and the Cockpit, credential presence is reported without any value being read, and the crate calls no model — its no-network boundary stands. What local runtimes (Ollama, LM Studio, llama.cpp) actually serve is observed from outside the crate, admitted with each capability's evidence, and routed over on request."
weight = 46
[extra]
id = "models"
status = "stable"
source = ".ai/repo/features/models.md"
+++
{% raw %}

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

A local vendor's `runtime` says how it is reached. `scripts/inference-observe` reaches
each declared runtime on the loopback interface, identifies it by its own signature,
and records what it serves through `models observe`; `models observed` reads the
observation back with every capability word's evidence — inferred, advertised, probed,
or refuted by a probe — and `models route --observed --local-only --require embeddings
--min-evidence probed` chooses from it, saying what the choice rests on (ADR 0174).

## What it does not do

The crate calls no model and reads no secret; the observer's probes ask a loaded local
model for a handful of tokens or one embedding, and only when asked: credential facts are the *presence* of a named
environment variable, never a value, and the catalogue's schema has no field a secret
could hide in (a test holds it). It states no price and no unverified vendor — what
cannot carry provenance stays absent. And it does not yet record which model actually
executed a session's work; that belongs to the capture adapters, whose schema already
declares the fields (ADR 0049 names it as the follow-up). Executing work on an observed
model, recording its tokens and latency, and advertising inference to the mesh are the
open issues of the local-inference milestone.
{% endraw %}
