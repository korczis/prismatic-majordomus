<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `instances` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.2 -->
# Module `instances` — MCP instances from instance registries

Third-party MCP servers on this machine that announce themselves by writing a record (mcp-instance/v1) to a shared registry directory: listed with whether their owner is alive, refused when a record is malformed or carries a token, never deleted, and probed with an MCP ping only on request and only on loopback.

Stability: behaviorally_verified. Capabilities: 1.

## `instances.list` — MCP instances from instance registries

Every record of every registry directory named by the input's registries or MAJORDOMUS_INSTANCE_REGISTRIES (colon-separated): name, kind, owner pid and whether it is alive (live or stale; a stale record is left in place), endpoint, the path of its token file, and the record's other fields. A malformed file, or a record that carries agent_token or token, is refused with the reason and nothing of it is echoed. With probe, each live instance's loopback endpoint is sent an MCP ping with the token read from its token file in memory; a non-loopback endpoint or one that differs from the token file's is not probed. Writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_instances` |
| MCP resource | `majordomus://instances` |
| HTTP | `GET /api/v1/instances` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::instances |
| tags | instances, discovery, mcp, introspection |

| input | type | required | description |
|---|---|---|---|
| `registries` | array | no | The registry directories to read; when empty, the colon-separated MAJORDOMUS_INSTANCE_REGISTRIES. |
| `probe` | boolean | no | Ping each instance's loopback MCP endpoint with the token from its token file (read in memory, never echoed). |

Output: `InstanceList`.

