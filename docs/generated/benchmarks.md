<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the benchmark projection of the canonical capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Benchmark targets and coverage

Every externally callable operation is a benchmark target, derived from the registry: each executable capability directly and on every transport its exposure declares, with the cases its input type provides, plus the transports' own operations. Nothing below is listed by hand; `majordomus bench coverage` computes the same table live, `majordomus bench` times it, and `capabilities validate` fails when a requirement is missing.

## Coverage

| scope | required | covered | missing | waived |
|---|---|---|---|---|
| direct | 11 | 11 | 0 | 0 |
| http | 11 | 11 | 0 | 0 |
| mcp | 11 | 11 | 0 | 0 |
| system | 9 | 9 | 0 | 0 |
| total | 42 | 42 | 0 | 0 |

## Capabilities

| capability | module | kind | cache | direct | mcp | http | cases |
|---|---|---|---|---|---|---|---|
| `capabilities.describe` | capabilities | query | — | covered | covered | covered | `repository-info` |
| `capabilities.list` | capabilities | query | process, 16 entries | covered | covered | covered | `all`, `queries` |
| `objects.get` | objects | query | — | covered | covered | covered | `first-object`, `repository` |
| `objects.list` | objects | query | — | covered | covered | covered | `all`, `first-kind` |
| `objects.search` | objects | query | process, 64 entries | covered | covered | covered | `common-word`, `no-hit` |
| `peers.announce` | peers | command | — | covered | covered | covered | `default` |
| `peers.list` | peers | query | — | covered | covered | covered | `default` |
| `perf.counters` | perf | query | — | covered | covered | covered | `default` |
| `repository.info` | repository | query | — | covered | covered | covered | `default` |
| `repository.scope` | repository | query | — | covered | covered | covered | `default` |
| `repository.scope_classify` | repository | query | — | covered | covered | covered | `layer-file`, `local-half`, `secret`, `undeclared`, `first-object` |

## System targets

| key | transport | measures |
|---|---|---|
| `system.mcp.process_cold` | mcp | spawn a majordomus mcp process, initialize, first tools/list |
| `system.mcp.initialize` | mcp | initialize on a running process |
| `system.mcp.ping` | mcp | ping: the protocol round trip with nothing behind it |
| `system.mcp.tools_list` | mcp | tools/list |
| `system.mcp.resources_list` | mcp | resources/list |
| `system.mcp.resources_read` | mcp | resources/read of the first declarative resource |
| `system.http.index` | http | GET / |
| `system.http.openapi` | http | GET /openapi.json |
| `system.http.docs` | http | GET /docs (the Swagger UI shell) |

Cache modes: a cached capability is measured cold (the cache cleared before every sample) and warm (the same input repeated); the direct transport reports the handler invocations of each. Evidence: `.ai/local/benchmarks/` for local runs, `.ai/repo/benchmarks/rust/` for the accepted baselines and the regression policy.
