<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `models` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.28.0 -->
# Module `models` — Models

The model catalogue the distribution declares — vendors, canonical model references, typed capabilities, context windows, lifecycle — and the explainable routing over it: what a stated need selects, what stands behind it, and why every excluded model fell out. Declared data; nothing here calls a model or reads a credential's value.

Stability: experimental. Capabilities: 4.

## `models.list` — The model catalogue

Every declared vendor and model, optionally narrowed by vendor, capability word, or one id or alias. Vendors carry whether their named credential variable is set — presence only, never a value. The order is the declaration's, which is also routing's preference order; the catalogue's own findings (a duplicate alias, an undeclared vendor) ride along.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_models` |
| MCP resource | `majordomus://models` |
| HTTP | `GET /api/v1/models` |
| CLI | `majordomus models list` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::models |
| tags | models, catalogue |

| input | type | required | description |
|---|---|---|---|
| `vendor` | string or null | no | Only this vendor. |
| `capability` | string or null | no | Only models declaring this capability word. |
| `id` | string or null | no | One model, by canonical id or alias. |

Output: `ModelsReport`.

## `models.observe` — Record an observation of local runtimes

Admit what scripts/inference-observe found at the local runtimes the catalogue declares — which endpoints answered as which runtime, what each serves, each model's capability words with their evidence (inferred, advertised, probed) and the probes made — and replace the checkout's observation under .ai/local/state/inference/. Refused, with every finding, when a vendor is not a declared local runtime, an endpoint carries a credential or leaves the loopback interface unadmitted, or a field is malformed. Secret shapes are redacted before anything is written.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_models_observe` |
| HTTP | `POST /api/v1/models/observations` |
| CLI | `majordomus models observe` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::models |
| tags | models, inference, records |

| input | type | required | description |
|---|---|---|---|
| `version` | integer | yes | The format version; only `VERSION` is admitted. |
| `observed_at` | string | yes | When the transport finished, RFC 3339 — reported, not trusted for staleness. |
| `probe` | ProbeScope | no | Which models were probed. |
| `remote_admitted` | boolean | no | Whether the observer admitted endpoints off the loopback interface. |
| `runtimes` | array | no | One report per endpoint, in the observer's plan order. |
| `refused` | array | no | The endpoints it would not reach. |

Output: `ObservationRecorded`.

## `models.observed` — The observed local runtimes

The checkout's current observation of its local inference runtimes: each endpoint with its runtime instance identity, locality and whether it answered as the runtime its vendor names; each model with its instance identity, digest, quantization, load state, context window (advertised and configured, either possibly unknown) and every capability word with the standing its evidence gives it. Says how old the observation is and whether routing still trusts it; absent and stale are answers, never guessed around.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_models_observed` |
| HTTP | `GET /api/v1/models/observations` |
| CLI | `majordomus models observed` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::models |
| tags | models, inference |

| input | type | required | description |
|---|---|---|---|
| `max_age` | integer or null | no | The age beyond which the observation is stale, seconds; 900 when omitted. |

Output: `ObservedReport`.

## `models.route` — Route a need to a model

Decide which declared model a stated need selects: the first in declaration order satisfying every requirement, the qualifying rest as the fallback chain, and every excluded model with the first check it failed. Pure over the catalogue and the input — the same question gets the same answer, and 'why' is in the answer itself.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_models_route` |
| HTTP | `GET /api/v1/models/route` |
| CLI | `majordomus models route` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::models |
| tags | models, routing |

| input | type | required | description |
|---|---|---|---|
| `require` | string or null | no | Capability words the model must declare, comma-separated: `vision,tools`. |
| `min_context` | integer or null | no | The least context window, tokens. |
| `vendor` | string or null | no | Only this vendor. |
| `local_only` | boolean or null | no | Only local inference. |
| `model` | string or null | no | A model named outright, by canonical id or alias — still checked against the
other requirements. |
| `observed` | boolean or null | no | Also choose from the checkout's observation of local runtimes (`models observed`),
after every declared model. |
| `min_evidence` | object | no | The least evidence an observed model's required capability must rest on:
`inferred`, `advertised` or `probed`. |
| `max_age` | integer or null | no | The age beyond which the observation is stale, seconds; 900 when omitted. |

Output: `RoutingDecision`.

