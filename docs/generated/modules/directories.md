<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `directories`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `directories` — Directory contracts

The layer's directories as a hierarchy: the contract each one declares, what it owes and which contract said so, and the chain that applies to it once inheritance is resolved.

Stability: behaviorally_verified. Capabilities: 1.

## `directories.list` — The layer's directory contracts

Every directory of the layer the index knows, with the contract it declares, whether it owes one and which contract decided, and — for a named path, or when asked for everywhere — the effective chain composed from the root down, least specific first.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_directories` |
| MCP resource | `majordomus://directories` |
| HTTP | `GET /api/v1/directories` |
| cache | process, 8 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::directories |
| tags | directories, context, introspection |

| input | type | required | description |
|---|---|---|---|
| `path` | string or null | no | One directory, repository-relative and inside the layer. Its effective chain is
always resolved. Absent means every directory of the layer. |
| `effective` | boolean or null | no | Resolve the effective chain for every directory, not only for a named one. |
| `state` | object | no | Only directories in this state: `documented`, `exempt` or `owed`. |

Output: `DirectoryReport`.

