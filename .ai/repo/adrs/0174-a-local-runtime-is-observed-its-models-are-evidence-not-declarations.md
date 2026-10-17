---
schema: adr/v1
id: adr-0174
kind: adr
title: A local inference runtime is observed, and what its models can do is evidence, not a declaration
status: proposed
date: 2026-10-17
tags:
  - models
  - inference
  - routing
  - evidence
related:
  - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - file:.ai/repo/adrs/0049-the-model-catalogue-is-declarative-data-and-routing-is-an-ex.md
  - file:.ai/repo/adrs/0098-reasoning-is-provider-independent-advisors-are-optional-and-review-is-recorded.md
  - file:share/models.yaml
  - file:apps/majordomus-cli/src/models/observed.rs
  - file:scripts/lib/inference/observer.mjs
  - file:docs/MODELS.md
provenance:
  origin: authored
---

# 174. A local inference runtime is observed, and what its models can do is evidence, not a declaration

## Context

The model catalogue (ADR 0049) declares Ollama and LM Studio as local vendors with no
model, because which model a local runtime serves is the machine's, not the
distribution's. That stayed true and left local inference unusable: `models route
--local-only` could select nothing on any machine, the advisor catalogue (ADR 0098) called
a local runtime available when its executable was on `PATH` even with no model pulled, and
llama.cpp's llama-server was not known at all. The machine this was written on had Ollama
0.40.4 running with nothing pulled, LM Studio installed with one embedding model and its
server stopped, and llama-server installed with no weights: three different truths that
the repository rendered as one.

Three constraints stand. ADR 0032 keeps HTTP clients out of `majordomus-cli`; network
transports live in the Node layer under `scripts/lib/`. ADR 0049 names "an outside layer
writing files the crate reads" as a legal shape for live discovery. And a runtime's word
about a model is not proof: an OpenAI-compatible `/v1/models` says which protocol answers,
not which runtime, and a model typed as a chat model may still refuse a completion.

## Decision

**The catalogue declares how a runtime is reached, never what it serves.** A local vendor
in `share/models.yaml` carries a `runtime`: the adapter under `scripts/lib/inference/`
that speaks its protocol, the loopback origin it listens on by default, and the variable
that moves it (`OLLAMA_HOST`, `LMSTUDIO_BASE_URL`, `LLAMA_SERVER_URL`). llama.cpp enters
as a third local vendor. No consumer keeps a port or an adapter name of its own.

**The transport observes; the crate admits and stores.** `scripts/inference-observe` reads
the vendors from `models list`, reaches each declared endpoint (the loopback interface
only, unless `--allow-remote` admits another host — nothing is scanned), identifies each
runtime by a signature only that runtime gives (Ollama's `/api/version`, LM Studio's typed
`/api/v0/models`, llama-server's `/props`), lists its models with digest, quantization,
load state and context window, and hands the report to `models.observe`. That capability
refuses a vendor that is not a declared local runtime, an endpoint that carries a
credential or leaves the loopback interface unadmitted, and a malformed field; it redacts
secret shapes, assigns deterministic identities (a runtime instance is its vendor at its
endpoint; a model instance is a model at that runtime, bound to its digest when known) and
replaces the checkout's one observation under `.ai/local/state/inference/`. It is checkout
state: never committed, never published, so no private endpoint can reach the site.

**A capability's standing is never higher than its evidence.** Each word is `inferred`
(the runtime's kind of model usually can), `advertised` (the runtime's own metadata says
so) or `probed` (a bounded request just succeeded). A probe that failed refutes the word,
whatever was advertised. Passive observation loads nothing and generates nothing; `--probe`
asks loaded models for a handful of tokens or one embedding, `--probe-all` asks every
model and may load each.

**Routing overlays the observation; it never rewrites the catalogue.** `models.route
--observed` considers observed models after every declared one, loaded before unloaded,
and holds them to their own checks in the declared order: freshness (an observation older
than its time to live, 900 s by default, is stale and selects nothing), the runtime's
locality for `local_only`, each required word's presence, refutation and, with
`--min-evidence`, its standing. The observation and its age are arguments of a pure
function, so the decision is still the same for the same inputs and still says why.

## Alternatives rejected

- **An HTTP client in the crate.** It would undo ADR 0032 for one feature, and the crate's
  hand-written HTTP/1.1 client reads no chunked or streamed reply, which every one of these
  runtimes streams by default. The Node layer already reaches two of them for the advisors.
- **Writing observed models into `share/models.yaml`.** A machine's runtime state in a
  shipped declaration is the manually synchronised reality this repository refuses: it
  would be wrong on every other machine and stale on this one by the next pull.
- **One registry per runtime.** Three inventories of the same facts. One observation
  document, one adapter per protocol, one admission, one routing overlay.
- **Trusting a protocol answer as an identity.** LM Studio, llama-server and Ollama all
  answer an OpenAI-compatible route; only a native signature tells them apart.

## Consequences

- `models route --local-only --observed --require embeddings` can select a model on a
  machine that serves one, and says what the choice rests on.
- The three runtimes are told apart by identity, not by protocol, and a compatible server
  that is none of them is reported `unidentified` rather than guessed.
- The surfaces are projections of two new capability declarations, `models.observe` and
  `models.observed`; the CLI, HTTP, OpenAPI, MCP and generated reference follow from them.
- Not decided here, and left to the issues of the local-inference milestone: executing
  work on an observed model through the execution plane, recording its tokens and latency
  in the economics model, advertising inference as a mesh feature, host and resource
  accounting, the advisor catalogue reading the observation instead of `PATH`, and the
  Cockpit and Observatory views.
