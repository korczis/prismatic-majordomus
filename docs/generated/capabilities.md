<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Capability reference

Every capability this executable ships, as the registry holds it. MCP tools and resources, HTTP routes, the OpenAPI document (`openapi.json` beside this file, and `/openapi.json` when serving), Swagger UI, the command line's `capabilities` commands, the benchmark targets (`benchmarks.md`) and the registry manifest (`registry.json`) are projections of the same entries; nothing below is declared anywhere else.

## Modules

| module | title | stability | capabilities | reference |
|---|---|---|---|---|
| `capabilities` | Capabilities | behaviorally_verified | 2 | [`modules/capabilities.md`](modules/capabilities.md) |
| `objects` | Objects | behaviorally_verified | 3 | [`modules/objects.md`](modules/objects.md) |
| `peers` | Peers | behaviorally_verified | 2 | [`modules/peers.md`](modules/peers.md) |
| `perf` | Performance | behaviorally_verified | 1 | [`modules/perf.md`](modules/perf.md) |
| `repository` | Repository | behaviorally_verified | 3 | [`modules/repository.md`](modules/repository.md) |

## Executable capabilities

| id | module | kind | stability | MCP tool | MCP resource | HTTP | CLI | cache | benchmark |
|---|---|---|---|---|---|---|---|---|---|
| `capabilities.describe` | `capabilities` | query | behaviorally_verified | `majordomus_capability` | — | `GET /api/v1/capability` | `majordomus capabilities describe` | — | required |
| `capabilities.list` | `capabilities` | query | behaviorally_verified | `majordomus_capabilities` | — | `GET /api/v1/capabilities` | `majordomus capabilities list` | process, 16 entries | required |
| `objects.get` | `objects` | query | behaviorally_verified | `majordomus_get` | — | `GET /api/v1/object` | — | — | required |
| `objects.list` | `objects` | query | behaviorally_verified | `majordomus_list` | — | `GET /api/v1/objects` | — | — | required |
| `objects.search` | `objects` | query | behaviorally_verified | `majordomus_search` | — | `GET /api/v1/search` | — | process, 64 entries | required |
| `peers.announce` | `peers` | command | behaviorally_verified | `majordomus_announce` | — | `POST /api/v1/peers/announce` | — | — | required |
| `peers.list` | `peers` | query | behaviorally_verified | `majordomus_peers` | — | `GET /api/v1/peers` | — | — | required |
| `perf.counters` | `perf` | query | behaviorally_verified | `majordomus_perf` | — | `GET /api/v1/perf` | — | — | required |
| `repository.info` | `repository` | query | behaviorally_verified | `majordomus_repository` | `majordomus://repository` | `GET /api/v1/repository` | — | — | required |
| `repository.scope` | `repository` | query | behaviorally_verified | `majordomus_scope` | `majordomus://scope` | `GET /api/v1/scope` | `majordomus scope` | — | required |
| `repository.scope_classify` | `repository` | query | behaviorally_verified | `majordomus_scope_classify` | — | `GET /api/v1/scope/classify` | `majordomus scope classify` | — | required |

## Declarative resources

Every object of the repository's AI layer is a capability of kind `resource` with the id `<kind>.<identity>` (`rule.majordomus.scope-integrity@1`, `prompt.continue`, `document.docs/CLI.md`), exposed as the MCP resource `majordomus://<kind>/<identity>` and read over HTTP through `objects.get`; its module is its kind. They are not listed here: they are the repository's, not the executable's, and `majordomus capabilities list --kind resource` answers for the repository at hand. Kinds present in this repository at generation: `claim`, `context`, `document`, `implementation`, `issue`, `milestone`, `policy`, `profile`, `prompt`, `rule`, `test`.

## Infrastructure routes

The HTTP projection's own routes, not capabilities: `/`, `/openapi.json`, `/docs`, `/mcp`. `/docs` is a Swagger UI shell that loads `/openapi.json`; it embeds no specification. `/mcp` is MCP over HTTP on the shared server.
