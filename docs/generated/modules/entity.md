<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `entity` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `entity` — Entities

Every object of the layer as an addressable, cross-linked node: its route, its outgoing references, the references that resolve to it, the surfaces that answer for it, and what can be said about its enforcement without running anything. Nothing here enumerates kinds, entities or routes — an object of the index has a route because it is an object.

Stability: implemented. Capabilities: 2.

## `entity.kinds` — Every kind, and whether every object of it is addressable

The kinds the layer holds, each with the route of its index and how many objects it carries, and every route collision there is. A collision is the only way an object of the index can fail to have an address of its own; the healthy answer is none.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_kinds` |
| HTTP | `GET /api/v1/entity/kinds` |
| CLI | `majordomus entity kinds` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::entity |
| tags | objects, governance |

Input: none.

Output: `KindList`.

## `entity.show` — Read one entity

One object of the layer as an addressable node, by URI or by the address it is served at: identity and route, provenance, front matter and content, every reference it declares and every reference that resolves to it, the surfaces that answer for it, and the state of the executable artefacts it names.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_entity` |
| HTTP | `GET /api/v1/entity` |
| CLI | `majordomus entity show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::entity |
| tags | objects, governance |

| input | type | required | description |
|---|---|---|---|
| `uri` | string or null | no | `majordomus://<kind>/<identity>`. Either this, or `kind` and `slug` together. |
| `kind` | string or null | no | The kind, when addressing by route. |
| `slug` | string or null | no | The last segment of the entity's route, when addressing by route. |

Output: `EntityView`.

