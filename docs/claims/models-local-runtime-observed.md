# A local inference runtime the catalogue declares is reached, identified by its own signature and its models recorded typed; a compatible server that is none of them is reported unidentified

## What it means

Ollama, LM Studio and llama.cpp's llama-server are declared in `share/models.yaml` with the
endpoint each listens on by default and the variable that moves it. `scripts/inference-observe`
reaches each one, and an answer counts only when it carries a signature that runtime alone
gives: Ollama's `/api/version`, LM Studio's typed `/api/v0/models` with a load state,
llama-server's `/props` with slots and generation settings. What each serves is recorded with
its digest, quantization, format, load state, and the advertised and configured context
windows as separate, possibly unknown, facts. A server that answers only the OpenAI-compatible
`/v1/models` is `unidentified`, with that diagnostic, rather than mistaken for one of them.

## How it works

One adapter per protocol under `scripts/lib/inference/` (`ollama.mjs`, `lmstudio.mjs`,
`llamacpp.mjs`), driven by `observer.mjs` concurrently and reported in the plan's order, not the
order answers arrived. The crate's `models.observe` (`apps/majordomus-cli/src/models/observed.rs`)
admits the report against the catalogue, assigns deterministic identities — a runtime instance
is its vendor at its endpoint, a model instance is that model at that runtime bound to its
digest — and replaces the checkout's observation. ADR 0174 is the decision.

## How to see it

```
scripts/inference-observe
majordomus models observed
bash test/run.sh 3211_a_local_runtime_is_observed
```

## What it does not cover

It does not install, pull or load a model, and passive observation generates nothing. It does
not establish that a model is good at anything; that is the evidence a capability carries and
what benchmarks would add. A runtime whose API changes shape is reported unidentified or
failed until its adapter learns the new shape. CI proves the adapters against fakes of the
documented responses; a live run against real runtimes is recorded evidence, not repeated by CI.

## Why it exists

The catalogue declared Ollama and LM Studio with no model, so nothing could tell which local
model can do what on this machine, and every OpenAI-compatible server looked alike. Identity
first, then inventory, is what lets routing and the advisors trust a local runtime for what it
is.
