<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `mcp` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.20.0 -->
# Module `mcp` — MCP projection

The MCP projection of this registry, described as one typed value: who answers `initialize` and with which protocol versions, the transports and how many sessions each holds, what of the protocol is served and what is not, every tool with the effect and the hints its annotations are derived from, the tools that write the repository, the resources, and where each declared client's configuration stands in this repository. Nothing in it is a list of its own: the tools are the registry's, the effect is the capability's classification, the clients are the distribution's providers.

Stability: experimental. Capabilities: 1.

## `mcp.projection` — The MCP projection of this registry

Who answers `initialize` and with which protocol versions; the transports and the sessions attached over each; the methods served, and that prompts and notifications are not; every tool (or only those with one effect) with its capability, effect and the hints its annotations are derived from; the tools that write the repository; the resources; and whether each client the distribution declares has a configuration here that starts this repository's server. Reads the registry and one file per declared client; writes nothing.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mcp` |
| MCP resource | `majordomus://mcp` |
| HTTP | `GET /api/v1/mcp` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mcp |
| tags | mcp, introspection, projections |

| input | type | required | description |
|---|---|---|---|
| `effect` | object | no | List only the tools with this effect: `read`, `process_state` or
`repository_mutation`. Absent lists every tool. |

Output: `McpProjection`.

