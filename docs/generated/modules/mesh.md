<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `mesh` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `mesh` — Mesh

The nodes this process discovered on the mesh: authenticated observations of other running Majordomus instances, converged into one registry, with the providers that heard them and the trust the policy assigned. Observation, never authority: a listed node can execute nothing here.

Stability: experimental. Capabilities: 5.

## `mesh.doctor` — The mesh self-check

Every prerequisite proved on this machine alone: the declaration parses, the identity loads, a UDP socket binds, the multicast group joins, broadcast enables, and the protocol signs, encodes, parses and verifies end to end in memory. Deterministic, no second node required.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_doctor` |
| HTTP | `GET /api/v1/mesh/doctor` |
| CLI | `majordomus mesh doctor` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, diagnostics |

Input: none.

Output: `MeshDoctorReport`.

## `mesh.identity` — This machine's node identity

The node identity kept under the user's state directory, public half only: node id, public key, display name. The signing key appears in no projection. Absent is an answer, not an error — the identity is created when a mesh first activates.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_identity` |
| HTTP | `GET /api/v1/mesh/identity` |
| CLI | `majordomus mesh identity` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, identity |

Input: none.

Output: `MeshIdentityReport`.

## `mesh.nodes` — The discovered nodes

Every node this process has observed, deduplicated by node identity across every discovery source, in node-id order: identity, trust, presence, endpoints, capabilities, repositories, provenance and versions. In memory, gone with the process.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh_nodes` |
| HTTP | `GET /api/v1/mesh/nodes` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

Input: none.

Output: `NodeList`.

## `mesh.register` — Register with this node's mesh

Present one signed envelope; it is verified exactly as a datagram — bounds, staleness, signature, trust policy — and recorded as a rendezvous observation when it holds. The answer carries this node's own envelope and the candidates its registry holds, each verifiable end to end on its own signature. Registration grants nothing: the caller becomes a record, never an authorization. Changes this process's memory only.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_mesh_register` |
| HTTP | `POST /api/v1/mesh/register` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

| input | type | required | description |
|---|---|---|---|
| `envelope` | object | yes | The envelope. Verified here exactly as a datagram would be — bounds, staleness,
signature — before anything is recorded. |

Output: `RegisterAnswer`.

## `mesh.status` — The mesh, at a glance

Whether the mesh runs in this process and why not when it does not; this node's public identity; every discovery provider with its state and counters; the registry's tallies; and the refused datagrams by reason. The one status every surface shows.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_mesh` |
| MCP resource | `majordomus://mesh` |
| HTTP | `GET /api/v1/mesh` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::mesh |
| tags | mesh, coordination, discovery |

Input: none.

Output: `MeshStatus`.

