<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `health`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `health` — Health

Whether what this process serves is healthy, decided by the engines that already decide it: the index's diagnostics, the registry builder, the benchmark projection's coverage and the comparison `generate --check` makes. No check here has an opinion of its own.

Stability: behaviorally_verified. Capabilities: 2.

## `health.coverage` — What backs every capability

For every capability, what documents it, what tests it, what rule is in force over it, what times it and what reaches it — each cell naming the artifact it was resolved from, and every required dimension with nothing behind it reported as a gap. Resolved from the composed graph and the benchmark projection, so nothing here is a second opinion and nothing is filled from a claim.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_coverage` |
| MCP resource | `majordomus://coverage` |
| HTTP | `GET /api/v1/coverage` |
| cache | process, 2 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::health |
| tags | health, introspection, capabilities |

Input: none.

Output: `CoverageMatrix`.

## `health.report` — Health of this process

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
| provenance | builtin majordomus_cli::capability::builtin::health |
| tags | health, introspection |

Input: none.

Output: `Health`.

