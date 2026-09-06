<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `system`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `system` — System

Whether what this process serves is healthy, decided by the engines that already decide it: the index's diagnostics, the registry builder, the benchmark projection's coverage and the comparison `generate --check` makes. No check here has an opinion of its own.

Stability: behaviorally_verified. Capabilities: 1.

## `system.health` — Health of this process

Every dimension of what this process serves — the layer as it was read, the registry, the scope, version control, benchmark coverage, the committed registry manifest and the attached peers — each decided by the engine that owns it, with the command that reproduces the verdict.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_health` |
| MCP resource | `majordomus://health` |
| HTTP | `GET /api/v1/health` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::system |
| tags | system, health, introspection |

Input: none.

Output: `Health`.

