<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `connect` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Module `connect` — Attaching a client

How each client reaches this repository's one shared MCP server: the launcher every configuration names, the Streamable HTTP endpoint of the server that is running, the configuration this checkout holds for a client that reads a file, and the vendor's own procedure — with this checkout's values filled in — for one that keeps it inside the application.

Stability: behaviorally_verified. Capabilities: 1.

## `connect.list` — How a client attaches to this repository

One entry per client the distribution ships an adapter for, with where its configuration lives, whether this checkout carries it, and what to put in front of it. The clients are the providers; nothing here is a list of vendors.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_connect` |
| MCP resource | `majordomus://connect` |
| HTTP | `GET /api/v1/connect` |
| CLI | `majordomus connect` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::connect |
| tags | connect, mcp, providers |

| input | type | required | description |
|---|---|---|---|
| `client` | string or null | no | One provider id (`chatgpt`, `claude-code`, `codex`, ...). Absent means every client. |

Output: `ConnectReport`.

