+++
title = "The model catalogue"
description = "the model catalogue: vendors and models declared once in share/models.yaml with provenance and no fabricated facts, and the explainable routing that names why every candidate was chosen or excluded"
weight = 47
[extra]
source = "docs/MODELS.md"
+++

{% raw %}

Which AI models exist for this tool's world, what each can do, and which one a stated
need selects — as declared data. ADR 0044 is the decision; `share/models.yaml` is the
whole declaration; `apps/majordomus-cli/src/models/` is the implementation. Nothing
here calls a model, holds an SDK, or reads a credential's value: ADR 0032's no-network
boundary stands, and what cannot be verified is absent rather than guessed.

## Vocabulary

A **vendor** is the party that serves models — remote (an API) or local (a runtime on
the machine). The word "provider" is deliberately not used: ADR 0024/0032 spent it on
the AI client tools (`share/providers.yaml`), and one word for two registries is how
truths fork. A **model** is a canonical reference: the id this repository's records
and configuration name, the vendor's own `native_id` behind it, aliases that resolve
to it, typed capability words, a context window, a lifecycle status.

## The declaration

`share/models.yaml`, in the exact grain of `providers.yaml`: one file, declaration
order meaningful (it is routing's preference order), facts with provenance stated in
the file header. Three deliberate absences:

- **No pricing.** A price the tool cannot verify live is a fact it must not state.
- **No unverified vendors.** An entry enters when a named reference could verify it.
- **No secrets.** A vendor's `credential_env` names an environment variable; only its
  *presence* is ever reported, and a test holds that the catalogue's schema has no
  property a credential value could hide in.

## The surfaces

Two capabilities, one declaration each, every surface a projection:

```
majordomus models list [--vendor V] [--capability C] [--id NAME]
majordomus models route [--require a,b] [--min-context N] [--vendor V] [--local-only] [--model NAME]

GET /api/v1/models          GET /api/v1/models/route
majordomus_models           majordomus_models_route        (MCP tools)
majordomus://models         (MCP resource)
/cockpit/models             (the Cockpit's Models page)
```

`models.list` answers the catalogue — narrowable, with each vendor's
credential-presence and the catalogue's own findings (a duplicate alias, an
undeclared vendor). `models.route` answers a need.

## Routing

Routing is a pure function over the declared data. The requirements: capability words
(all required), a minimum context window, a vendor, `local_only`, or a model named
outright — which is still checked against the rest, so an override that cannot do the
work is an exclusion with a reason, not a silent selection. The decision:

- **selected** — the first model in declaration order satisfying every requirement,
  with the reason;
- **fallbacks** — the qualifying rest, in order: the chain a caller walks when the
  selected model fails *at the caller's end* (this tool has no live layer to fail);
- **excluded** — every non-qualifying model with the first check it failed, in the
  fixed check order (named-outright, lifecycle, vendor, locality, capabilities,
  context).

Deprecated and retired models are excluded unless named outright. There is no health,
no load, no cost term — nothing this tool could not defend. Ask the same question
twice, get the same answer twice; `--format json` is the same decision the HTTP route
and MCP tool serve.

## What this is not, yet

Recording which model *actually executed* a session's work belongs to the capture and
lifecycle adapters — their schema (`capture.v1`) already declares the optional
`model`, `effort` and `tokens` fields, and nothing fills them today
(`docs/ECONOMICS.md` names this as the missing half). Live model discovery over
vendor APIs, health and usage accounting would each need a network layer this crate
deliberately does not have; ADR 0044 names the two legal shapes such a thing could
take (an outside layer writing files the crate reads, or a new ADR revisiting ADR
0032's posture). Until then the catalogue is honest, versioned, reviewable data.
{% endraw %}
