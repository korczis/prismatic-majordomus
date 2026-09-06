<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `graph`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Module `graph` — Graphs

The graphs derived from the registry and the index: the executable's own capability registry, the shape of the layer, the rule dependencies, the decisions and what they put in force, and the use cases and what they exercise. Canonical nodes and edges; a rendering library is a consumer, never the shape.

Stability: behaviorally_verified. Capabilities: 2.

## `graph.get` — Derive one graph

One graph by id: its nodes and edges with the vocabularies that say what each kind means, the file every node was derived from, and whether the result is acyclic. Deterministic for a given tree and executable.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_graph` |
| HTTP | `GET /api/v1/graph` |
| cache | process, 16 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::graph |
| tags | graph |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The graph's id, as `graph.list` gives it (`registry`, `layer`, `rules`, `adrs`,
`use-cases`). |

Output: `Graph`.

## `graph.list` — List graphs

Every graph this executable derives: its id, what it shows, and what it is derived from.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_graphs` |
| MCP resource | `majordomus://graphs` |
| HTTP | `GET /api/v1/graphs` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::graph |
| tags | graph, introspection |

Input: none.

Output: `GraphList`.

