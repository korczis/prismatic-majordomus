<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `server` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->
# Module `server` — Server

The shared server of this checkout and of every other checkout of the same git repository: what each lease says, whether the server it names answers, whether what answers is the code on disk at this executable's version, where each runtime stands and how it got there, and how many peers each one holds.

Stability: behaviorally_verified. Capabilities: 1.

## `server.status` — The shared server, and every server of the repository

Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; where its runtime stands (`runtime`: that word refined by the server's readiness report, its mesh verdict and the last recorded transition into building, mesh_attaching, operational, degraded, stopping, stopped or failed, with the time, reason and recorder of that transition, kept under `.ai/local/state/runtime/` and written only when the state changes — this reader records `stopped` or `failed` for a server it finds gone); how far the tree it serves is from the remote trunk (`trunk`: current, lagging, stale, unjudged or unmeasured against the policy's `checkout.trunk_distance`, as of the last fetch); the lease this process holds when it is the server; and, unless `checkouts` narrows it to this one, every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, its runtime, and the peers its server reports. `checkouts: this` reads one lease and probes one server and enumerates no other checkout, which is what a caller asking only about the checkout it is in should pay. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes.

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

| input | type | required | description |
|---|---|---|---|
| `checkouts` | Checkouts | no | Which checkouts the answer covers. Absent means `repository`. |

Output: `ServerStatus`.

