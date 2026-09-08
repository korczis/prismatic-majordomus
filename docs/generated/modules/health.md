<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `health` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `health` — Health

Whether what this process serves is healthy, decided by the engines that already decide it: the index's diagnostics, the registry builder, the benchmark projection's coverage and the comparison `generate --check` makes. No check here has an opinion of its own.

Stability: behaviorally_verified. Capabilities: 3.

## `health.live` — Liveness

Is this process alive: the cheapest true statement this executable can make about itself, with the version that answered. No filesystem traversal, no index build, no network — this is what a hosting platform polls, and it must cost nothing to say.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| HTTP | `GET /api/v1/live` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::health |
| tags | health, deployment |

Input: none.

Output: `Liveness`.

## `health.ready` — Readiness

Can this process serve traffic: the registry and the index it built at start-up, already resident, and how the layer read. Only local initialisation — never an external provider, a database or another service, because a readiness check that probes a dependency fails a deployment for something that is not this process.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| HTTP | `GET /api/v1/ready` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::health |
| tags | health, deployment |

Input: none.

Output: `Readiness`.

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

