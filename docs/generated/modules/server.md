<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `server` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `server` — Server

The shared server of this checkout and of every other checkout of the same git repository: what each lease says, whether the server it names answers, whether what answers is the code on disk at this executable's version, and how many peers each one holds.

Stability: behaviorally_verified. Capabilities: 1.

## `server.status` — The shared server, and every server of the repository

Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; the lease this process holds when it is the server; and every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, and the peers its server reports. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_server` |
| MCP resource | `majordomus://server` |
| HTTP | `GET /api/v1/server` |
| CLI | `majordomus serve status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::server |
| tags | server, lease, coordination, introspection |

Input: none.

Output: `ServerStatus`.

