<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `server` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.4.0 -->
# Module `server` — Server

The shared server of this checkout and of every other checkout of the same git repository: what each lease says, whether the server it names answers, whether what answers is the code on disk at this executable's version, where each runtime stands and how it got there, and how many peers each one holds; and every Majordomus runtime of this user on this host, read from leases with no checkout open.

Stability: behaviorally_verified. Capabilities: 2.

## `server.runtimes` — Every Majordomus runtime of this user on this host

Every Majordomus runtime of this user on this host, whatever repository it serves, with no checkout open: each checkout with its root and identity, the git repository it belongs to, where its server stands (the same five words as `server.status`, measured against the answering executable, and `live` only when a server answers — a lease whose process is gone is `stale`), the port its lease names, the version it runs (the lease's, else what its executable reports to `--version`), the `serve` and `mcp` processes working on it and whether each holds the lease, and this host's node id. The checkouts come from two places and neither is believed: the disposable index of checkout roots each server adds itself to beside `node.json` (`checkout-roots` in `$XDG_STATE_HOME/majordomus`, else `~/.local/state/majordomus`), and the process table, where every `majordomus serve` and `majordomus mcp` is found by executable path — which is how a runtime older than the index, or one with another state directory, is listed. Every row is re-read from its checkout's lease, so deleting the index loses nothing but speed. Read-only: no process of its own, no registry, nothing started or stopped; peers stay the mesh registry's. The command line answers it without a server (`majordomus runtimes --host`, from any directory), and any running server answers the same reading here.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_runtimes` |
| MCP resource | `majordomus://runtimes` |
| HTTP | `GET /api/v1/runtimes` |
| CLI | `majordomus runtimes` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::server |
| tags | server, lease, host, introspection |

Input: none.

Output: `HostRuntimes`.

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

