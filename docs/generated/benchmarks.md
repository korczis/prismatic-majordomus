<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the benchmark projection of the canonical capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.8 -->
# Benchmark targets and coverage

Every externally callable operation is a benchmark target, derived from the registry: each executable capability directly and on every transport its exposure declares, with the cases its input type provides, plus the transports' own operations. Nothing below is listed by hand; `majordomus bench coverage` computes the same table live, `majordomus bench` times it, and `capabilities validate` fails when a requirement is missing.

## Coverage

| scope | required | covered | missing | waived |
|---|---|---|---|---|
| direct | 220 | 181 | 0 | 39 |
| http | 211 | 181 | 0 | 30 |
| mcp | 205 | 176 | 0 | 29 |
| system | 14 | 14 | 0 | 0 |
| total | 650 | 552 | 0 | 98 |

## Capabilities

| capability | module | kind | cache | direct | mcp | http | cases |
|---|---|---|---|---|---|---|---|
| `artifacts.list` | artifacts | query | process, 8 entries, 5s | covered | covered | covered | `all`, `one-encoding` |
| `blackboard.snapshot` | blackboard | query | — | covered | covered | covered | `this-checkout`, `addressed` |
| `brief.session` | brief | query | — | covered | covered | covered | `default` |
| `capabilities.describe` | capabilities | query | process, 512 entries | covered | covered | covered | `repository-info` |
| `capabilities.list` | capabilities | query | process, 16 entries | covered | covered | covered | `all`, `queries` |
| `capabilities.projections` | capabilities | query | process, 8 entries | covered | covered | covered | `all`, `unmet` |
| `commands.get` | commands | query | — | covered | covered | covered | `worktree-status` |
| `commands.graph` | commands | query | — | covered | covered | covered | `default` |
| `commands.list` | commands | query | — | covered | covered | covered | `all`, `read-only`, `one-program` |
| `commit.history` | commit | query | — | covered | covered | covered | `whole-history` |
| `commit.plan` | commit | query | — | covered | covered | covered | `default` |
| `commit.scopes` | commit | query | — | covered | covered | covered | `default` |
| `commit.validate` | commit | query | — | covered | covered | covered | `conventional`, `not-conventional`, `merge` |
| `continuity.device` | continuity | command | — | waived | waived | waived | — |
| `continuity.plan` | continuity | query | — | covered | covered | covered | `default`, `by-prefix` |
| `continuity.publish` | continuity | command | — | waived | waived | waived | — |
| `continuity.records` | continuity | query | — | covered | covered | covered | `default` |
| `continuity.resume` | continuity | command | — | waived | waived | waived | — |
| `continuity.state` | continuity | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `continuity.status` | continuity | query | — | covered | covered | covered | `default` |
| `continuity.sync` | continuity | command | — | waived | waived | waived | — |
| `convergence.report` | convergence | query | — | covered | covered | covered | `default` |
| `dashboard.overview` | dashboard | query | — | covered | covered | covered | `default` |
| `delivery.feature` | delivery | query | — | waived | waived | waived | — |
| `delivery.report` | delivery | query | — | waived | waived | waived | — |
| `delivery.trends` | delivery | query | — | covered | — | covered | `default` |
| `deploy.check` | deploy | query | process, 2 entries, 5s | covered | covered | covered | `default` |
| `deploy.get` | deploy | query | — | covered | covered | covered | `first-deployment` |
| `deploy.list` | deploy | query | — | covered | covered | covered | `default` |
| `deploy.observed` | deploy | query | — | covered | covered | covered | `default` |
| `deploy.slug` | deploy | command | — | waived | — | — | — |
| `design.contrast` | design | query | — | covered | covered | covered | `default` |
| `design.explain` | design | query | — | covered | covered | covered | `role`, `state` |
| `design.system` | design | query | — | covered | covered | covered | `default` |
| `design.tokens` | design | query | — | covered | covered | covered | `all`, `roles` |
| `devcontext.compile` | devcontext | query | process, 16 entries | covered | covered | covered | `no-seed`, `issue`, `milestone`, `intent`, `source-ranges` |
| `devcontext.explain` | devcontext | query | process, 8 entries | covered | covered | covered | `policy`, `absent` |
| `devcontext.policy` | devcontext | query | — | covered | covered | covered | `default` |
| `devtask.issue` | devtask | query | — | covered | covered | covered | `records-only`, `with-git` |
| `devtask.milestone` | devtask | query | — | covered | covered | covered | `first-milestone` |
| `directories.list` | directories | query | process, 8 entries, 5s | covered | covered | covered | `whole-tree`, `one-directory`, `effective-everywhere`, `owed` |
| `distribution.artifact` | distribution | query | — | covered | covered | covered | `first-published-target` |
| `distribution.build` | distribution | query | — | covered | covered | covered | `default` |
| `distribution.model` | distribution | query | — | covered | covered | covered | `default` |
| `distribution.releases` | distribution | query | process, 1 entries, per generation | covered | covered | covered | `default` |
| `distribution.status` | distribution | query | — | covered | covered | covered | `default` |
| `economics.check` | economics | query | — | covered | covered | covered | `default` |
| `economics.explain` | economics | query | process, 32 entries, 5s | covered | covered | covered | `primary` |
| `economics.runs` | economics | query | process, 16 entries, 5s | covered | covered | covered | `all`, `narrowed` |
| `economics.summary` | economics | query | process, 8 entries, 5s | covered | covered | covered | `default`, `narrowed` |
| `economics.tasks` | economics | query | — | covered | covered | covered | `all`, `window` |
| `editors.status` | editors | query | — | covered | covered | covered | `default` |
| `entity.kinds` | entity | query | — | covered | covered | covered | `default` |
| `entity.show` | entity | query | — | covered | covered | covered | `first-object` |
| `environment.explain` | environment | query | — | covered | covered | covered | `all`, `one-field` |
| `environment.preflight` | environment | query | — | covered | covered | covered | `sealed` |
| `environment.status` | environment | query | — | covered | covered | covered | `default` |
| `episodes.attach` | episodes | command | — | covered | covered | covered | `default` |
| `episodes.detach` | episodes | command | — | covered | covered | covered | `default` |
| `episodes.list` | episodes | query | — | covered | covered | covered | `default` |
| `evidence.claim` | evidence | query | process, 32 entries, per checkout state | covered | covered | covered | `first-claim` |
| `evidence.record` | evidence | command | — | waived | — | — | — |
| `evidence.report` | evidence | query | process, 8 entries, per checkout state | covered | covered | covered | `all`, `findings` |
| `evidence.stamp` | evidence | query | — | waived | — | — | — |
| `evidence.test` | evidence | query | — | covered | covered | covered | `first-test` |
| `executions.cancel` | executions | command | — | waived | waived | waived | — |
| `executions.demonstrate` | executions | query | — | covered | covered | covered | `immediate` |
| `executions.events` | executions | query | — | waived | waived | waived | — |
| `executions.get` | executions | query | — | waived | waived | waived | — |
| `executions.list` | executions | query | — | covered | covered | covered | `recent` |
| `executions.protocol` | executions | query | — | covered | covered | covered | `default` |
| `executions.report` | executions | command | — | waived | — | waived | — |
| `executions.start` | executions | command | — | covered | covered | covered | `demonstrate` |
| `gates.completion` | gates | query | — | covered | covered | covered | `this-task`, `one-source-file`, `a-document`, `everything-on-demand` |
| `gates.model` | gates | query | process, 4 entries, 5s | covered | covered | covered | `default` |
| `gates.policy` | gates | query | process, 4 entries, 5s | covered | covered | covered | `default` |
| `gates.state` | gates | query | — | covered | covered | covered | `default` |
| `graph.get` | graph | query | process, 16 entries | covered | covered | covered | `registry`, `layer`, `rules`, `adrs`, `use-cases`, `why`, `product`, `composed` |
| `graph.list` | graph | query | — | covered | covered | covered | `default` |
| `guard.edit` | guard | query | — | covered | covered | covered | `in-repository`, `outside` |
| `health.live` | health | query | — | covered | — | covered | `default` |
| `health.ready` | health | query | — | covered | — | covered | `default` |
| `health.report` | health | query | process, 4 entries, 5s | covered | covered | covered | `default` |
| `implement.issue` | implement | command | — | waived | — | — | — |
| `integration.cleanup` | integration | query | — | covered | covered | covered | `default` |
| `integration.events` | integration | query | — | covered | covered | covered | `default` |
| `integration.explain` | integration | query | — | covered | covered | covered | `a-number` |
| `integration.prove_dry_run` | integration | query | — | waived | waived | waived | — |
| `integration.queue` | integration | query | — | covered | covered | covered | `default` |
| `integration.timings` | integration | query | — | covered | covered | covered | `default` |
| `integration.trains` | integration | query | — | covered | covered | covered | `default` |
| `intent_realization.explain` | intent_realization | query | — | covered | covered | covered | `first-intent` |
| `intent_realization.work` | intent_realization | query | — | covered | covered | covered | `every-intent`, `first-intent` |
| `intents.binding` | intents | query | — | covered | covered | covered | `first-issue` |
| `intents.coverage` | intents | query | — | covered | covered | covered | `default` |
| `intents.list` | intents | query | process, 2 entries, per checkout state | covered | covered | covered | `default` |
| `intents.preflight` | intents | query | — | covered | covered | covered | `first-issue` |
| `intents.propose` | intents | command | — | waived | waived | waived | — |
| `intents.record` | intents | query | — | covered | covered | covered | `first-intent` |
| `intents.validate` | intents | query | — | covered | covered | covered | `default` |
| `knowledge_base.candidates` | knowledge_base | query | — | covered | covered | covered | `default` |
| `knowledge_base.record` | knowledge_base | query | — | covered | covered | covered | `first-record` |
| `knowledge_base.status` | knowledge_base | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `layout.check` | layout | query | — | covered | covered | covered | `default` |
| `lifecycle.closed` | lifecycle | query | process, 2 entries, 30s | covered | covered | covered | `default` |
| `lifecycle.episodes` | lifecycle | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `lifecycle.providers` | lifecycle | query | process, 2 entries | covered | covered | covered | `default` |
| `lifecycle.recovery` | lifecycle | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `lifecycle.runtime` | lifecycle | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `mcp.projection` | mcp | query | — | covered | covered | covered | `every-tool`, `writers` |
| `mesh.build_cache.ensure` | mesh | command | — | waived | — | — | — |
| `mesh.build_cache.env` | mesh | query | — | waived | waived | waived | — |
| `mesh.build_cache.key` | mesh | query | — | waived | waived | waived | — |
| `mesh.build_cache.provision` | mesh | command | — | waived | — | — | — |
| `mesh.claim` | mesh | command | — | waived | waived | waived | — |
| `mesh.cooperation` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.doctor` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.enrol` | mesh | command | — | waived | waived | waived | — |
| `mesh.events` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.firewall` | mesh | query | — | waived | waived | waived | — |
| `mesh.firewall.apply` | mesh | command | — | waived | — | — | — |
| `mesh.handover.consume` | mesh | command | — | waived | waived | waived | — |
| `mesh.handover.publish` | mesh | command | — | waived | waived | waived | — |
| `mesh.identity` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.link.hello` | mesh | command | — | covered | — | covered | `malformed` |
| `mesh.link.sync` | mesh | command | — | covered | — | covered | `malformed` |
| `mesh.nodes` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.peer` | mesh | query | — | covered | covered | covered | `unknown` |
| `mesh.peers` | mesh | query | — | covered | covered | covered | `summary`, `full` |
| `mesh.register` | mesh | command | — | covered | covered | covered | `refused` |
| `mesh.release` | mesh | command | — | waived | waived | waived | — |
| `mesh.review.answer` | mesh | command | — | waived | waived | waived | — |
| `mesh.review.request` | mesh | command | — | waived | waived | waived | — |
| `mesh.review.withdraw` | mesh | command | — | waived | waived | waived | — |
| `mesh.session.close` | mesh | command | — | waived | waived | waived | — |
| `mesh.session.open` | mesh | command | — | waived | waived | waived | — |
| `mesh.state` | mesh | query | — | covered | covered | covered | `summary`, `full` |
| `mesh.status` | mesh | query | — | covered | covered | covered | `default` |
| `mesh.verify` | mesh | command | — | waived | waived | waived | — |
| `models.list` | models | query | process, 4 entries, 5s | covered | covered | covered | `default`, `narrowed` |
| `models.route` | models | query | — | covered | covered | covered | `default` |
| `monitor.stream` | monitor | query | — | covered | covered | covered | `local-first-page`, `local-resumed` |
| `objects.get` | objects | query | — | covered | covered | covered | `first-object`, `repository` |
| `objects.list` | objects | query | — | covered | covered | covered | `all`, `first-kind` |
| `objects.search` | objects | query | process, 64 entries | covered | covered | covered | `common-word`, `no-hit` |
| `objects.verify` | objects | query | — | covered | covered | covered | `bounded` |
| `obligations.closure` | obligations | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `obligations.vocabulary` | obligations | query | process, 2 entries | covered | covered | covered | `default` |
| `observatory.capture` | observatory | query | — | covered | covered | covered | `default` |
| `observatory.events` | observatory | query | — | covered | covered | covered | `newest`, `after a cursor` |
| `observatory.replay` | observatory | query | — | covered | covered | covered | `default` |
| `pack.plan` | pack | query | — | covered | covered | covered | `default-profile`, `chatgpt` |
| `pack.verify` | pack | query | — | covered | covered | covered | `absent` |
| `peers.announce` | peers | command | — | covered | covered | covered | `default` |
| `peers.list` | peers | query | — | covered | covered | covered | `default` |
| `perf.counters` | perf | query | — | covered | covered | covered | `default` |
| `plan.delivery` | plan | query | — | covered | covered | covered | `every-issue`, `one-issue-at-head` |
| `plan.drift` | plan | query | — | covered | covered | covered | `every-issue`, `one-issue-at-head-as-trunk-gate` |
| `plan.flight` | plan | query | — | covered | covered | covered | `where-the-work-is`, `one-milestone-inspected` |
| `plan.issues` | plan | query | process, 16 entries, per generation | covered | covered | covered | `all`, `ready`, `one-milestone` |
| `plan.model` | plan | query | — | covered | covered | covered | `default` |
| `plan.next` | plan | query | process, 16 entries, per generation | covered | covered | covered | `whole-plan`, `one-milestone` |
| `plan.record` | plan | query | — | covered | covered | covered | `issue`, `milestone` |
| `plan.roadmap` | plan | query | process, 4 entries, per generation | covered | covered | covered | `default` |
| `plan.status` | plan | query | process, 16 entries, per generation | covered | covered | covered | `whole-plan`, `one-milestone` |
| `plan.transition` | plan | command | — | covered | covered | covered | `refused-by-status` |
| `plan.validate` | plan | query | — | covered | covered | covered | `default` |
| `plan.waves` | plan | query | — | covered | covered | covered | `whole-plan`, `one-milestone` |
| `product.domains` | product | query | process, 4 entries | covered | covered | covered | `stable`, `any` |
| `product.feature` | product | query | process, 64 entries | covered | covered | covered | `first-feature` |
| `product.features` | product | query | process, 32 entries | covered | covered | covered | `all`, `featured`, `by-area`, `by-domain` |
| `product.matrix` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `product.providers` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `product.validate` | product | query | process, 2 entries | covered | covered | covered | `default` |
| `quality.report` | quality | query | process, 8 entries, 10s | covered | covered | covered | `default` |
| `quality.rustdoc` | quality | query | process, 8 entries, 10s | covered | covered | covered | `default`, `missing-pages-summary` |
| `reasoning.advisors` | reasoning | query | — | covered | covered | covered | `default` |
| `reasoning.check` | reasoning | query | — | covered | covered | covered | `default` |
| `reasoning.explain` | reasoning | query | — | waived | waived | waived | — |
| `reasoning.plan` | reasoning | query | — | covered | covered | covered | `material` |
| `reasoning.record` | reasoning | command | — | covered | covered | covered | `refused` |
| `reasoning.status` | reasoning | query | — | covered | covered | covered | `current`, `all` |
| `recover.orphans` | recover | command | — | covered | covered | covered | `check-twelve-hours`, `check-one-second` |
| `release.analysis` | release | query | — | waived | waived | waived | — |
| `release.changelog` | release | query | — | covered | covered | covered | `all`, `one-version` |
| `release.currency` | release | query | — | waived | waived | waived | — |
| `release.next` | release | query | — | waived | waived | waived | — |
| `release.version` | release | query | process, 2 entries, per checkout state | covered | covered | covered | `default` |
| `repository.info` | repository | query | process, 1 entries, per generation | covered | covered | covered | `default` |
| `repository.scope` | repository | query | — | covered | covered | covered | `default` |
| `repository.scope_classify` | repository | query | — | covered | covered | covered | `layer-file`, `local-half`, `secret`, `undeclared`, `first-object` |
| `rules.proves` | rules | query | — | covered | covered | covered | `suite` |
| `rules.report` | rules | query | process, 8 entries, per checkout state | covered | covered | covered | `all`, `findings`, `blocking` |
| `rules.show` | rules | query | process, 32 entries, per checkout state | covered | covered | covered | `first` |
| `served.forge` | served | command | — | waived | — | — | — |
| `served.observe` | served | command | — | waived | — | — | — |
| `served.show` | served | query | — | covered | covered | covered | `head`, `pages-against-head` |
| `server.status` | server | query | — | covered | covered | covered | `repository`, `this-checkout` |
| `session_domain.identity` | session_domain | query | process, 2 entries, 2s | covered | covered | covered | `default` |
| `session_domain.machine` | session_domain | query | process, 1 entries, 600s | covered | covered | covered | `default` |
| `shell.check` | shell | query | — | covered | covered | covered | `default` |
| `skills.explain` | skills | query | — | covered | covered | covered | `first-skill` |
| `skills.status` | skills | query | — | covered | covered | covered | `default` |
| `skills.verify` | skills | query | — | covered | covered | covered | `default` |
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
| `worktree.trunk` | worktree | query | — | covered | covered | covered | `default`, `primary-checkout` |
| `worktree.uncommitted` | worktree | query | — | covered | covered | covered | `policy-threshold`, `one-hour` |

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
| `system.http.live.index` | http | GET / over the live view a served process uses (the per-request git stamp and surface resolution) |

## UI pages

Every screen and panel of the UI's own route list, read with `GET` and `Accept: text/html`: a screen over the pinned context and over the live view a served process answers from (which pays the per-request git stamp), a panel over the pinned context only. Each is measured cold (the capability cache cleared before every sample) and warm. A deferred panel is one the UI pulls in after the screen, because its source is slow; only the `full` profile measures it, with `cold_spawns` samples.

| key | route | view | deferred |
|---|---|---|---|
| `system.http.ui.index` | `/ui` | pinned | — |
| `system.http.ui.command` | `/ui/command` | pinned | — |
| `system.http.ui.idd` | `/ui/idd` | pinned | — |
| `system.http.ui.roadmap` | `/ui/roadmap` | pinned | — |
| `system.http.ui.work` | `/ui/work` | pinned | — |
| `system.http.ui.issues` | `/ui/issues` | pinned | — |
| `system.http.ui.prs` | `/ui/prs` | pinned | — |
| `system.http.ui.capabilities` | `/ui/capabilities` | pinned | — |
| `system.http.ui.skills` | `/ui/skills` | pinned | — |
| `system.http.ui.mesh` | `/ui/mesh` | pinned | — |
| `system.http.ui.peers` | `/ui/peers` | pinned | — |
| `system.http.ui.delivery` | `/ui/delivery` | pinned | — |
| `system.http.ui.deployments` | `/ui/deployments` | pinned | — |
| `system.http.ui.trends` | `/ui/trends` | pinned | — |
| `system.http.ui.evidence` | `/ui/evidence` | pinned | — |
| `system.http.ui.knowledge` | `/ui/knowledge` | pinned | — |
| `system.http.ui.docs` | `/ui/docs` | pinned | — |
| `system.http.ui.governance` | `/ui/governance` | pinned | — |
| `system.http.ui.observatory` | `/ui/observatory` | pinned | — |
| `system.http.live.ui.index` | `/ui` | live | — |
| `system.http.live.ui.command` | `/ui/command` | live | — |
| `system.http.live.ui.idd` | `/ui/idd` | live | — |
| `system.http.live.ui.roadmap` | `/ui/roadmap` | live | — |
| `system.http.live.ui.work` | `/ui/work` | live | — |
| `system.http.live.ui.issues` | `/ui/issues` | live | — |
| `system.http.live.ui.prs` | `/ui/prs` | live | — |
| `system.http.live.ui.capabilities` | `/ui/capabilities` | live | — |
| `system.http.live.ui.skills` | `/ui/skills` | live | — |
| `system.http.live.ui.mesh` | `/ui/mesh` | live | — |
| `system.http.live.ui.peers` | `/ui/peers` | live | — |
| `system.http.live.ui.delivery` | `/ui/delivery` | live | — |
| `system.http.live.ui.deployments` | `/ui/deployments` | live | — |
| `system.http.live.ui.trends` | `/ui/trends` | live | — |
| `system.http.live.ui.evidence` | `/ui/evidence` | live | — |
| `system.http.live.ui.knowledge` | `/ui/knowledge` | live | — |
| `system.http.live.ui.docs` | `/ui/docs` | live | — |
| `system.http.live.ui.governance` | `/ui/governance` | live | — |
| `system.http.live.ui.observatory` | `/ui/observatory` | live | — |
| `system.http.ui.panels.pull_requests` | `/ui/panels/pull-requests` | pinned | yes |
| `system.http.ui.panels.delivery` | `/ui/panels/delivery` | pinned | yes |
| `system.http.ui.panels.drain` | `/ui/panels/drain` | pinned | yes |
| `system.http.ui.panels.standing` | `/ui/panels/standing` | pinned | yes |
| `system.http.ui.panels.shared_scope` | `/ui/panels/shared-scope` | pinned | yes |

Cache modes: a cached capability is measured cold (the cache cleared before every sample) and warm (the same input repeated); the direct transport reports the handler invocations of each. Evidence: `.ai/local/benchmarks/` for local runs, `.ai/repo/benchmarks/rust/` for the accepted baselines and the regression policy.
