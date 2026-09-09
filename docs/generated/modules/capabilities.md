<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `capabilities` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.2 -->
# Module `capabilities` — Capabilities

The registry seen through itself: every capability with its kind, stability, provenance, exposures, benchmark and cache policy, and one capability in full.

Stability: behaviorally_verified. Capabilities: 3.

## `capabilities.describe` — Describe one capability

One capability by canonical id: its kind, schemas, provenance, stability, exposures, benchmark and cache policy.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_capability` |
| HTTP | `GET /api/v1/capability` |
| CLI | `majordomus capabilities describe` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::capabilities |
| tags | introspection |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The canonical id, e.g. `repository.info` or `rule.majordomus.scope-integrity@1`. |

Output: `Capability`.

## `capabilities.list` — List capabilities

Every capability of this executable and this repository, summarised: kind, module, stability, provenance, the projections it declares, its benchmark and cache policy; the schemas are answered by capabilities.describe.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_capabilities` |
| HTTP | `GET /api/v1/capabilities` |
| CLI | `majordomus capabilities list` |
| cache | process, 16 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::capabilities |
| tags | introspection |

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | Only capabilities of this kind: `query`, `command` or `resource`. |
| `exposure` | string or null | no | Only capabilities exposed through this projection: `mcp`, `http` or `cli`. |

Output: `CapabilityList`.

## `capabilities.projections` — Where each capability is projected

A row per capability with the command line, HTTP route, MCP tool and MCP resource it reaches, whether every exposure it declares is answered by that surface, and the runnable commands no capability claims. Derived from the registry and the clap declaration; nothing is written down.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_projections` |
| HTTP | `GET /api/v1/capabilities/projections` |
| CLI | `majordomus capabilities projections` |
| cache | process, 8 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::capabilities |
| tags | introspection, projections |

| input | type | required | description |
|---|---|---|---|
| `module` | string or null | no | Only capabilities composed in this module. |
| `unmet` | boolean | no | Only the capabilities whose declared exposures are not all answered by their
surface. Empty is the closure the rule asks for. |

Output: `ClosureMatrix`.

