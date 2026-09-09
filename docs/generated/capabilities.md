<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Capability reference

Every capability this executable ships, as the registry holds it. MCP tools and resources, HTTP routes, the OpenAPI document (`openapi.json` beside this file, and `/openapi.json` when serving), Swagger UI, the command line's `capabilities` commands, the benchmark targets (`benchmarks.md`) and the registry manifest (`registry.json`) are projections of the same entries; nothing below is declared anywhere else.

## Modules

| module | title | stability | capabilities | reference |
|---|---|---|---|---|
| `artifacts` | Generated artifacts | behaviorally_verified | 1 | [`modules/artifacts.md`](modules/artifacts.md) |
| `capabilities` | Capabilities | behaviorally_verified | 3 | [`modules/capabilities.md`](modules/capabilities.md) |
| `commands` | Command graph | implemented | 3 | [`modules/commands.md`](modules/commands.md) |
| `continuity` | Continuity | behaviorally_verified | 1 | [`modules/continuity.md`](modules/continuity.md) |
| `deploy` | Deployment | behaviorally_verified | 3 | [`modules/deploy.md`](modules/deploy.md) |
| `directories` | Directory contracts | behaviorally_verified | 1 | [`modules/directories.md`](modules/directories.md) |
| `distribution` | Distribution | behaviorally_verified | 5 | [`modules/distribution.md`](modules/distribution.md) |
| `environment` | Repository environment | behaviorally_verified | 2 | [`modules/environment.md`](modules/environment.md) |
| `executions` | Executions | behaviorally_verified | 7 | [`modules/executions.md`](modules/executions.md) |
| `graph` | Graphs | behaviorally_verified | 2 | [`modules/graph.md`](modules/graph.md) |
| `health` | Health | behaviorally_verified | 3 | [`modules/health.md`](modules/health.md) |
| `objects` | Objects | behaviorally_verified | 4 | [`modules/objects.md`](modules/objects.md) |
| `peers` | Peers | behaviorally_verified | 2 | [`modules/peers.md`](modules/peers.md) |
| `perf` | Performance | behaviorally_verified | 1 | [`modules/perf.md`](modules/perf.md) |
| `plan` | The plan and its derivations | behaviorally_verified | 8 | [`modules/plan.md`](modules/plan.md) |
| `product` | Product | behaviorally_verified | 5 | [`modules/product.md`](modules/product.md) |
| `quality` | Public API quality | behaviorally_verified | 1 | [`modules/quality.md`](modules/quality.md) |
| `release` | Release | implemented | 2 | [`modules/release.md`](modules/release.md) |
| `repository` | Repository | behaviorally_verified | 3 | [`modules/repository.md`](modules/repository.md) |
| `trace` | Traceability | behaviorally_verified | 3 | [`modules/trace.md`](modules/trace.md) |
| `web` | Web surfaces | behaviorally_verified | 1 | [`modules/web.md`](modules/web.md) |
| `why` | Why | behaviorally_verified | 6 | [`modules/why.md`](modules/why.md) |
| `worktree` | Worktree topology | behaviorally_verified | 4 | [`modules/worktree.md`](modules/worktree.md) |

## Executable capabilities

| id | module | kind | stability | MCP tool | MCP resource | HTTP | CLI | cache | benchmark |
|---|---|---|---|---|---|---|---|---|---|
| `artifacts.list` | `artifacts` | query | behaviorally_verified | `majordomus_artifacts` | `majordomus://artifacts` | `GET /api/v1/artifacts` | — | process, 8 entries, 5s | required |
| `capabilities.describe` | `capabilities` | query | behaviorally_verified | `majordomus_capability` | — | `GET /api/v1/capability` | `majordomus capabilities describe` | — | required |
| `capabilities.list` | `capabilities` | query | behaviorally_verified | `majordomus_capabilities` | — | `GET /api/v1/capabilities` | `majordomus capabilities list` | process, 16 entries | required |
| `capabilities.projections` | `capabilities` | query | behaviorally_verified | `majordomus_projections` | — | `GET /api/v1/capabilities/projections` | `majordomus capabilities projections` | process, 8 entries | required |
| `commands.get` | `commands` | query | implemented | `majordomus_command` | — | `GET /api/v1/command` | — | — | required |
| `commands.graph` | `commands` | query | implemented | `majordomus_command_graph` | — | `GET /api/v1/commands/graph` | — | — | required |
| `commands.list` | `commands` | query | implemented | `majordomus_commands` | `majordomus://commands` | `GET /api/v1/commands` | — | — | required |
| `continuity.state` | `continuity` | query | behaviorally_verified | `majordomus_continuity` | `majordomus://continuity` | `GET /api/v1/continuity` | — | process, 2 entries, 2s | required |
| `deploy.check` | `deploy` | query | behaviorally_verified | `majordomus_deploy_check` | — | `GET /api/v1/deployments/check` | — | process, 2 entries, 5s | required |
| `deploy.get` | `deploy` | query | behaviorally_verified | `majordomus_deployment` | — | `GET /api/v1/deployment` | — | — | required |
| `deploy.list` | `deploy` | query | behaviorally_verified | `majordomus_deployments` | `majordomus://deployments` | `GET /api/v1/deployments` | — | — | required |
| `directories.list` | `directories` | query | behaviorally_verified | `majordomus_directories` | `majordomus://directories` | `GET /api/v1/directories` | — | process, 8 entries, 5s | required |
| `distribution.artifact` | `distribution` | query | behaviorally_verified | `majordomus_artifact` | — | `GET /api/v1/distribution/artifact` | `majordomus distribution artifact` | — | required |
| `distribution.build` | `distribution` | query | behaviorally_verified | `majordomus_build` | — | `GET /api/v1/distribution/build` | `majordomus distribution build` | — | required |
| `distribution.model` | `distribution` | query | behaviorally_verified | `majordomus_distribution` | — | `GET /api/v1/distribution` | `majordomus distribution show` | — | required |
| `distribution.releases` | `distribution` | query | behaviorally_verified | `majordomus_releases` | — | `GET /api/v1/distribution/releases` | `majordomus distribution releases` | — | required |
| `distribution.status` | `distribution` | query | behaviorally_verified | `majordomus_install_status` | — | `GET /api/v1/distribution/status` | `majordomus distribution status` | — | required |
| `environment.explain` | `environment` | query | behaviorally_verified | `majordomus_environment_explain` | — | `GET /api/v1/environment/explain` | — | — | required |
| `environment.status` | `environment` | query | behaviorally_verified | `majordomus_environment` | `majordomus://environment` | `GET /api/v1/environment` | — | process, 4 entries, 3s | required |
| `executions.cancel` | `executions` | command | behaviorally_verified | `majordomus_execution_cancel` | — | `POST /api/v1/executions/cancel` | `majordomus executions cancel` | — | waived (transient_state) |
| `executions.demonstrate` | `executions` | query | behaviorally_verified | `majordomus_demonstrate_execution` | — | `GET /api/v1/executions/demonstrate` | — | — | required |
| `executions.events` | `executions` | query | behaviorally_verified | `majordomus_execution_events` | — | `GET /api/v1/executions/events` | `majordomus executions events` | — | waived (transient_state) |
| `executions.get` | `executions` | query | behaviorally_verified | `majordomus_execution` | — | `GET /api/v1/executions/get` | `majordomus executions show` | — | waived (transient_state) |
| `executions.list` | `executions` | query | behaviorally_verified | `majordomus_executions` | `majordomus://executions` | `GET /api/v1/executions` | `majordomus executions list` | — | required |
| `executions.protocol` | `executions` | query | behaviorally_verified | `majordomus_execution_protocol` | `majordomus://executions/protocol` | `GET /api/v1/executions/protocol` | `majordomus executions protocol` | — | required |
| `executions.start` | `executions` | command | behaviorally_verified | `majordomus_execution_start` | — | `POST /api/v1/executions/start` | `majordomus run` | — | required |
| `graph.get` | `graph` | query | behaviorally_verified | `majordomus_graph` | — | `GET /api/v1/graph` | — | process, 16 entries | required |
| `graph.list` | `graph` | query | behaviorally_verified | `majordomus_graphs` | `majordomus://graphs` | `GET /api/v1/graphs` | — | — | required |
| `health.live` | `health` | query | behaviorally_verified | — | — | `GET /api/v1/live` | — | — | required |
| `health.ready` | `health` | query | behaviorally_verified | — | — | `GET /api/v1/ready` | — | — | required |
| `health.report` | `health` | query | behaviorally_verified | `majordomus_health` | `majordomus://health` | `GET /api/v1/health` | — | process, 4 entries, 5s | required |
| `objects.get` | `objects` | query | behaviorally_verified | `majordomus_get` | — | `GET /api/v1/object` | — | — | required |
| `objects.list` | `objects` | query | behaviorally_verified | `majordomus_list` | — | `GET /api/v1/objects` | — | — | required |
| `objects.search` | `objects` | query | behaviorally_verified | `majordomus_search` | — | `GET /api/v1/search` | — | process, 64 entries | required |
| `objects.verify` | `objects` | query | behaviorally_verified | `majordomus_verify_objects` | — | `GET /api/v1/objects/verify` | — | — | required |
| `peers.announce` | `peers` | command | behaviorally_verified | `majordomus_announce` | — | `POST /api/v1/peers/announce` | — | — | required |
| `peers.list` | `peers` | query | behaviorally_verified | `majordomus_peers` | — | `GET /api/v1/peers` | — | — | required |
| `perf.counters` | `perf` | query | behaviorally_verified | `majordomus_perf` | — | `GET /api/v1/perf` | — | — | required |
| `plan.issues` | `plan` | query | behaviorally_verified | `majordomus_plan_issues` | — | `GET /api/v1/plan/issues` | — | — | required |
| `plan.model` | `plan` | query | behaviorally_verified | `majordomus_plan` | `majordomus://plan` | `GET /api/v1/plan` | — | — | required |
| `plan.next` | `plan` | query | behaviorally_verified | `majordomus_plan_next` | — | `GET /api/v1/plan/next` | — | — | required |
| `plan.record` | `plan` | query | behaviorally_verified | `majordomus_plan_record` | — | `GET /api/v1/plan/record` | — | — | required |
| `plan.roadmap` | `plan` | query | behaviorally_verified | `majordomus_plan_roadmap` | — | `GET /api/v1/plan/roadmap` | — | — | required |
| `plan.status` | `plan` | query | behaviorally_verified | `majordomus_plan_status` | — | `GET /api/v1/plan/status` | — | — | required |
| `plan.validate` | `plan` | query | behaviorally_verified | `majordomus_plan_validate` | — | `GET /api/v1/plan/validate` | — | — | required |
| `plan.waves` | `plan` | query | behaviorally_verified | `majordomus_plan_waves` | — | `GET /api/v1/plan/waves` | — | — | required |
| `product.feature` | `product` | query | behaviorally_verified | `majordomus_feature` | — | `GET /api/v1/product/feature` | `majordomus product show` | process, 64 entries | required |
| `product.features` | `product` | query | behaviorally_verified | `majordomus_features` | `majordomus://product` | `GET /api/v1/product/features` | `majordomus product list` | process, 32 entries | required |
| `product.matrix` | `product` | query | behaviorally_verified | `majordomus_product_matrix` | `majordomus://product/matrix` | `GET /api/v1/product/matrix` | `majordomus product matrix` | process, 2 entries | required |
| `product.providers` | `product` | query | behaviorally_verified | `majordomus_providers` | `majordomus://product/providers` | `GET /api/v1/product/providers` | `majordomus product providers` | process, 2 entries | required |
| `product.validate` | `product` | query | behaviorally_verified | `majordomus_product_validate` | — | `GET /api/v1/product/validate` | `majordomus product validate` | process, 2 entries | required |
| `quality.report` | `quality` | query | behaviorally_verified | `majordomus_quality` | `majordomus://quality` | `GET /api/v1/quality` | `majordomus quality report` | process, 8 entries, 10s | required |
| `release.changelog` | `release` | query | implemented | `majordomus_changelog` | `majordomus://changelog` | `GET /api/v1/changelog` | — | — | required |
| `release.version` | `release` | query | implemented | `majordomus_release_version` | — | `GET /api/v1/release/version` | — | — | required |
| `repository.info` | `repository` | query | behaviorally_verified | `majordomus_repository` | `majordomus://repository` | `GET /api/v1/repository` | — | — | required |
| `repository.scope` | `repository` | query | behaviorally_verified | `majordomus_scope` | `majordomus://scope` | `GET /api/v1/scope` | `majordomus scope` | — | required |
| `repository.scope_classify` | `repository` | query | behaviorally_verified | `majordomus_scope_classify` | — | `GET /api/v1/scope/classify` | — | — | required |
| `trace.commit` | `trace` | query | behaviorally_verified | `majordomus_trace_commit` | — | `GET /api/v1/trace/commit` | — | — | required |
| `trace.issue` | `trace` | query | behaviorally_verified | `majordomus_trace_issue` | — | `GET /api/v1/trace/issue` | — | — | required |
| `trace.report` | `trace` | query | behaviorally_verified | `majordomus_traceability` | `majordomus://traceability` | `GET /api/v1/trace` | — | — | required |
| `web.surfaces` | `web` | query | behaviorally_verified | `majordomus_web_surfaces` | `majordomus://web` | `GET /api/v1/web/surfaces` | — | process, 2 entries, 5s | required |
| `why.areas` | `why` | query | behaviorally_verified | `majordomus_why_areas` | `majordomus://why/areas` | `GET /api/v1/why/areas` | `majordomus why areas` | process, 4 entries | required |
| `why.audiences` | `why` | query | behaviorally_verified | `majordomus_why_audiences` | `majordomus://why/audiences` | `GET /api/v1/why/audiences` | `majordomus why audiences` | process, 4 entries | required |
| `why.diagnose` | `why` | query | behaviorally_verified | `majordomus_why_diagnose` | — | `GET /api/v1/why/diagnose` | `majordomus why diagnose` | process, 32 entries | required |
| `why.list` | `why` | query | behaviorally_verified | `majordomus_why` | `majordomus://why` | `GET /api/v1/why` | `majordomus why list` | process, 32 entries | required |
| `why.moment` | `why` | query | behaviorally_verified | `majordomus_why_moment` | — | `GET /api/v1/why/moment` | `majordomus why show` | process, 64 entries | required |
| `why.validate` | `why` | query | behaviorally_verified | `majordomus_why_validate` | — | `GET /api/v1/why/validate` | `majordomus why validate` | process, 2 entries | required |
| `worktree.inspect` | `worktree` | query | behaviorally_verified | `majordomus_worktree_inspect` | — | `GET /api/v1/worktrees/inspect` | `majordomus worktree inspect` | — | required |
| `worktree.migration_plan` | `worktree` | query | behaviorally_verified | `majordomus_worktree_migration_plan` | — | `GET /api/v1/worktrees/migration` | `majordomus worktree migrate` | — | required |
| `worktree.status` | `worktree` | query | behaviorally_verified | `majordomus_worktree_status` | — | `GET /api/v1/worktrees/status` | `majordomus worktree status` | — | required |
| `worktree.topology` | `worktree` | query | behaviorally_verified | `majordomus_worktrees` | `majordomus://worktrees` | `GET /api/v1/worktrees` | `majordomus worktree topology` | — | required |

## Declarative resources

Every object of the repository's AI layer is a capability of kind `resource` with the id `<kind>.<identity>` (`rule.majordomus.scope-integrity@1`, `prompt.continue`, `document.docs/CLI.md`), exposed as the MCP resource `majordomus://<kind>/<identity>` and read over HTTP through `objects.get`; its module is its kind. They are not listed here: they are the repository's, not the executable's, and `majordomus capabilities list --kind resource` answers for the repository at hand. Kinds present in this repository at generation: `adr`, `application`, `area`, `audience`, `claim`, `command`, `context`, `deployment`, `distribution-model`, `document`, `feature`, `implementation`, `issue`, `knowledge`, `milestone`, `moment`, `policy`, `profile`, `prompt`, `release-record`, `rule`, `scope`, `session`, `skill`, `taxonomy`, `test`, `use-case`, `workspace`.

## Infrastructure routes

The HTTP projection's own routes, not capabilities: `/`, `/api/v1`, `/openapi.json`, `/swagger`, `/mcp`, `/events`, `/cockpit`. `/swagger` is a Swagger UI shell that loads `/openapi.json`; it embeds no specification. `/docs/` is this repository's own documentation, and `/mcp` is MCP over HTTP on the shared server.
