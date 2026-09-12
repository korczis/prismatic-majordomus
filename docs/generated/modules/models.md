<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `models` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `models` — Models

The model catalogue the distribution declares — vendors, canonical model references, typed capabilities, context windows, lifecycle — and the explainable routing over it: what a stated need selects, what stands behind it, and why every excluded model fell out. Declared data; nothing here calls a model or reads a credential's value.

Stability: experimental. Capabilities: 2.

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

Output: `RoutingDecision`.

