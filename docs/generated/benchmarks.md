<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the benchmark projection of the canonical capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Benchmark targets and coverage

Every externally callable operation is a benchmark target, derived from the registry: each executable capability directly and on every transport its exposure declares, with the cases its input type provides, plus the transports' own operations. Nothing below is listed by hand; `majordomus bench coverage` computes the same table live, `majordomus bench` times it, and `capabilities validate` fails when a requirement is missing.

## Coverage

| scope | required | covered | missing | waived |
|---|---|---|---|---|
| direct | 82 | 79 | 0 | 3 |
| http | 82 | 79 | 0 | 3 |
| mcp | 80 | 77 | 0 | 3 |
| system | 13 | 13 | 0 | 0 |
| total | 257 | 248 | 0 | 9 |

## Capabilities

| capability | module | kind | cache | direct | mcp | http | cases |
|---|---|---|---|---|---|---|---|
| `artifacts.list` | artifacts | query | process, 8 entries, 5s | covered | covered | covered | `all`, `one-encoding` |
| `capabilities.describe` | capabilities | query | — | covered | covered | covered | `repository-info` |
| `capabilities.list` | capabilities | query | process, 16 entries | covered | covered | covered | `all`, `queries` |
| `capabilities.projections` | capabilities | query | process, 8 entries | covered | covered | covered | `all`, `unmet` |
| `commands.get` | commands | query | — | covered | covered | covered | `worktree-status` |
| `commands.graph` | commands | query | — | covered | covered | covered | `default` |
| `commands.list` | commands | query | — | covered | covered | covered | `all`, `read-only`, `one-program` |
| `continuity.state` | continuity | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `deploy.check` | deploy | query | process, 2 entries, 5s | covered | covered | covered | `default` |
| `deploy.get` | deploy | query | — | covered | covered | covered | `first-deployment` |
| `deploy.list` | deploy | query | — | covered | covered | covered | `default` |
| `design.contrast` | design | query | — | covered | covered | covered | `default` |
| `design.explain` | design | query | — | covered | covered | covered | `role`, `state` |
| `design.system` | design | query | — | covered | covered | covered | `default` |
| `design.tokens` | design | query | — | covered | covered | covered | `all`, `roles` |
| `devtask.issue` | devtask | query | — | covered | covered | covered | `records-only`, `with-git` |
| `devtask.milestone` | devtask | query | — | covered | covered | covered | `first-milestone` |
| `directories.list` | directories | query | process, 8 entries, 5s | covered | covered | covered | `whole-tree`, `one-directory`, `effective-everywhere`, `owed` |
| `distribution.artifact` | distribution | query | — | covered | covered | covered | `first-published-target` |
| `distribution.build` | distribution | query | — | covered | covered | covered | `default` |
| `distribution.model` | distribution | query | — | covered | covered | covered | `default` |
| `distribution.releases` | distribution | query | — | covered | covered | covered | `default` |
| `distribution.status` | distribution | query | — | covered | covered | covered | `default` |
| `environment.explain` | environment | query | — | covered | covered | covered | `all`, `one-field` |
| `environment.status` | environment | query | — | covered | covered | covered | `default` |
| `executions.cancel` | executions | command | — | waived | waived | waived | — |
| `executions.demonstrate` | executions | query | — | covered | covered | covered | `immediate` |
| `executions.events` | executions | query | — | waived | waived | waived | — |
| `executions.get` | executions | query | — | waived | waived | waived | — |
| `executions.list` | executions | query | — | covered | covered | covered | `recent` |
| `executions.protocol` | executions | query | — | covered | covered | covered | `default` |
| `executions.start` | executions | command | — | covered | covered | covered | `demonstrate` |
| `graph.get` | graph | query | process, 16 entries | covered | covered | covered | `registry`, `layer`, `rules`, `adrs`, `use-cases`, `why`, `product`, `composed` |
| `graph.list` | graph | query | — | covered | covered | covered | `default` |
| `health.live` | health | query | — | covered | — | covered | `default` |
| `health.ready` | health | query | — | covered | — | covered | `default` |
| `health.report` | health | query | process, 4 entries, 5s | covered | covered | covered | `default` |
| `objects.get` | objects | query | — | covered | covered | covered | `first-object`, `repository` |
| `objects.list` | objects | query | — | covered | covered | covered | `all`, `first-kind` |
| `objects.search` | objects | query | process, 64 entries | covered | covered | covered | `common-word`, `no-hit` |
| `objects.verify` | objects | query | — | covered | covered | covered | `bounded` |
| `obligations.closure` | obligations | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `obligations.vocabulary` | obligations | query | process, 2 entries | covered | covered | covered | `default` |
| `peers.announce` | peers | command | — | covered | covered | covered | `default` |
| `peers.list` | peers | query | — | covered | covered | covered | `default` |
| `perf.counters` | perf | query | — | covered | covered | covered | `default` |
| `plan.issues` | plan | query | — | covered | covered | covered | `all`, `ready`, `one-milestone` |
| `plan.model` | plan | query | — | covered | covered | covered | `default` |
| `plan.next` | plan | query | — | covered | covered | covered | `whole-plan`, `one-milestone` |
| `plan.record` | plan | query | — | covered | covered | covered | `issue`, `milestone` |
| `plan.roadmap` | plan | query | — | covered | covered | covered | `default` |
| `plan.status` | plan | query | — | covered | covered | covered | `whole-plan`, `one-milestone` |
| `plan.validate` | plan | query | — | covered | covered | covered | `default` |
| `plan.waves` | plan | query | — | covered | covered | covered | `whole-plan`, `one-milestone` |
| `product.feature` | product | query | process, 64 entries | covered | covered | covered | `first-feature` |
| `product.features` | product | query | process, 32 entries | covered | covered | covered | `all`, `featured`, `by-area` |
| `product.matrix` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `product.providers` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `product.validate` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `quality.report` | quality | query | process, 8 entries, 10s | covered | covered | covered | `default` |
| `release.changelog` | release | query | — | covered | covered | covered | `all`, `one-version` |
| `release.version` | release | query | — | covered | covered | covered | `default` |
| `repository.info` | repository | query | — | covered | covered | covered | `default` |
| `repository.scope` | repository | query | — | covered | covered | covered | `default` |
| `repository.scope_classify` | repository | query | — | covered | covered | covered | `layer-file`, `local-half`, `secret`, `undeclared`, `first-object` |
| `server.status` | server | query | — | covered | covered | covered | `repository`, `this-checkout` |
| `session_domain.identity` | session_domain | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `session_domain.machine` | session_domain | query | process, 1 entries, 600s | covered | covered | covered | `default` |
| `trace.commit` | trace | query | — | covered | covered | covered | `head` |
| `trace.issue` | trace | query | — | covered | covered | covered | `first-issue` |
| `trace.report` | trace | query | — | covered | covered | covered | `default`, `ten` |
| `web.surfaces` | web | query | process, 2 entries, 5s | covered | covered | covered | `default` |
| `why.areas` | why | query | process, 4 entries | covered | covered | covered | `default` |
| `why.audiences` | why | query | process, 4 entries | covered | covered | covered | `default` |
| `why.diagnose` | why | query | process, 32 entries | covered | covered | covered | `three-moments` |
| `why.list` | why | query | process, 32 entries | covered | covered | covered | `all`, `by-audience`, `search` |
| `why.moment` | why | query | process, 64 entries | covered | covered | covered | `first-moment` |
| `why.validate` | why | query | process, 2 entries | covered | covered | covered | `default` |
| `worktree.inspect` | worktree | query | — | covered | covered | covered | `feature-branch` |
| `worktree.migration_plan` | worktree | query | — | covered | covered | covered | `default` |
| `worktree.status` | worktree | query | — | covered | covered | covered | `default`, `primary-checkout` |
| `worktree.topology` | worktree | query | — | covered | covered | covered | `default` |

## System targets

| key | transport | measures |
|---|---|---|
| `system.mcp.process_cold` | mcp | spawn a majordomus mcp process, initialize, first tools/list |
| `system.mcp.initialize` | mcp | initialize on a running process |
| `system.mcp.ping` | mcp | ping: the protocol round trip with nothing behind it |
| `system.mcp.tools_list` | mcp | tools/list |
| `system.mcp.resources_list` | mcp | resources/list |
| `system.mcp.resources_read` | mcp | resources/read of the first declarative resource |
| `system.http.index` | http | GET / (the topology as JSON) |
| `system.http.home` | http | GET / with Accept: text/html (the home page, rendered from the topology) |
| `system.http.openapi` | http | GET /openapi.json |
| `system.http.swagger` | http | GET /swagger (the Swagger UI shell) |
| `system.http.cockpit_overview` | http | GET /cockpit (the Cockpit's landing page, server-rendered) |
| `system.http.cockpit_capabilities` | http | GET /cockpit/capabilities (every capability as a table: the widest page) |
| `system.http.cockpit_graph` | http | GET /cockpit/graphs/registry (a page whose content is a derived graph) |

Cache modes: a cached capability is measured cold (the cache cleared before every sample) and warm (the same input repeated); the direct transport reports the handler invocations of each. Evidence: `.ai/local/benchmarks/` for local runs, `.ai/repo/benchmarks/rust/` for the accepted baselines and the regression policy.
