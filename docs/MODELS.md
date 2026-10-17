# The model catalogue

Which AI models exist for this tool's world, what each can do, and which one a stated
need selects — as declared data. ADR 0049 is the decision; `share/models.yaml` is the
whole declaration; `apps/majordomus-cli/src/models/` is the implementation. Nothing
in the crate calls a model, holds an SDK, or reads a credential's value: ADR 0032's
no-network boundary stands, and what cannot be verified is absent rather than guessed.
What local runtimes actually serve is observed from outside the crate and admitted as
evidence (ADR 0174, [below](#observed-local-runtimes)).

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

Four capabilities, one declaration each, every surface a projection:

```
majordomus models list [--vendor V] [--capability C] [--id NAME]
majordomus models route [--require a,b] [--min-context N] [--vendor V] [--local-only] [--model NAME]
                        [--observed] [--min-evidence inferred|advertised|probed] [--max-age S]
majordomus models observed [--max-age S]
majordomus models observe          (the observation, as JSON on stdin; scripts/inference-observe calls it)

GET /api/v1/models          GET /api/v1/models/route
GET /api/v1/models/observations                POST /api/v1/models/observations
majordomus_models  majordomus_models_route  majordomus_models_observed  majordomus_models_observe
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

A model's `status` is optional. An entry that declares none has an undeclared standing,
which is not the same fact as `available`: the field is then absent from every output
(`ModelEntry.status` carries no default), routing still considers the model, and no
surface may render it as available on the catalogue's behalf. A reader that relied on
`status` always being present treats its absence as undeclared.

Deprecated and retired models are excluded unless named outright. There is no health,
no load, no cost term — nothing this tool could not defend. Ask the same question
twice, get the same answer twice; `--format json` is the same decision the HTTP route
and MCP tool serve.

## Observed local runtimes

A local vendor declares how it is reached, never what it serves: its `runtime` names the
adapter under `scripts/lib/inference/`, the loopback origin it listens on by default and
the variable that moves it.

| Vendor | Adapter | Default endpoint | Variable | Identified by |
| --- | --- | --- | --- | --- |
| Ollama | `ollama` | `http://127.0.0.1:11434` | `OLLAMA_HOST` | `GET /api/version` |
| LM Studio | `lmstudio` | `http://127.0.0.1:1234` | `LMSTUDIO_BASE_URL` | `GET /api/v0/models` (typed, with a load state) |
| llama.cpp | `llamacpp` | `http://127.0.0.1:8080` | `LLAMA_SERVER_URL` | `GET /props` (slots and generation settings) |

The table restates `share/models.yaml`; the declaration is what the observer reads.

```
scripts/inference-observe                 # metadata only: nothing loaded, nothing generated
scripts/inference-observe --probe         # also probe loaded models: a few tokens, one embedding
scripts/inference-observe --probe-all     # probe every model (may load each)
scripts/inference-observe --endpoint llamacpp=http://127.0.0.1:8081 --dry-run
majordomus models observed
majordomus models route --observed --local-only --require embeddings --min-evidence probed
```

- **Reach.** Only declared endpoints, a declared variable's value, or `--endpoint` are
  reached, and only on the loopback interface unless `--allow-remote` admits another host.
  Nothing is scanned. Each discovery request has a 1.5 s deadline and each probe 60 s; a
  runtime that does not answer is `unreachable`, never a hang.
- **Identity.** A runtime is identified by a signature only it gives. A server answering the
  OpenAI-compatible `/v1/models` and nothing native is `unidentified`, with that diagnostic.
  A runtime instance is its vendor at its endpoint; a model instance is a model at that
  runtime, bound to its digest when the runtime reports one. Both identities are
  deterministic.
- **Evidence.** Every capability word carries its standing: `inferred` (the runtime's kind of
  model usually can — llama-server says nothing of a model's task, so `text` is inferred
  there), `advertised` (Ollama's `/api/show` capabilities, LM Studio's model type), or
  `probed` (a bounded request just succeeded). A failed probe refutes the word, whatever was
  advertised. A model's advertised context window and the one the runtime is configured to
  serve are separate fields, and either may be absent.
- **Storage.** `models observe` admits the report and replaces
  `.ai/local/state/inference/observation.json`: checkout state, never committed and never
  published, so no endpoint or model of a machine reaches the site.
- **Routing.** `--observed` adds observed models after every declared one, loaded first. They
  are held to freshness (900 s by default; a stale observation selects nothing and says to
  observe again), to the runtime's locality for `--local-only`, and to each required word's
  presence, refutation and, with `--min-evidence`, standing. The answer carries the
  observation it used (`observed.recorded_at`, `age_seconds`, `stale`) and the selected
  model's evidence in its reason.

## What this is not, yet

Recording which model *actually executed* a session's work belongs to the capture and
lifecycle adapters — their schema (`capture.v1`) already declares the optional
`model`, `effort` and `tokens` fields, and nothing fills them today
(`docs/ECONOMICS.md` names this as the missing half). Live model discovery over
vendor APIs, health and usage accounting would each need a network layer this crate
deliberately does not have; ADR 0049 names the two legal shapes such a thing could
take (an outside layer writing files the crate reads, or a new ADR revisiting ADR
0032's posture). Local runtimes took the first shape (ADR 0174); remote vendors have
not. Executing work on an observed model, recording its tokens and latency, advertising
inference to the mesh, host and resource accounting, and the Cockpit and Observatory
views of it are the open issues of the `local-inference` milestone.
