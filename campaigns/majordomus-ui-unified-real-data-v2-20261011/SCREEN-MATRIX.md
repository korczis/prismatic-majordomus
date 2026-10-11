# Screen-to-real-data matrix

| Screen | Image | Modules | Candidate canonical sources (verify live) |
|---|---|---|---|
| Dashboard | reference/01-dashboard-overview.png and 02 alternative | hero, real KPIs, activity feed, workflow, trend charts, PR/mesh/roadmap summaries | dashboard.overview; repository.info; health.report; plan/PR/mesh/evidence capabilities |
| PR Drain | reference/03-pr-drain.png | queue, real PR selected detail, checks, dependency graph, worker panel, repair status | existing PR integration queue/planner/drain and GitHub projection |
| Roadmap overview | reference/04-roadmap-overview.png | living Gantt, dependencies, focus areas and upcoming milestones | canonical project milestones/intents/issues and release records |
| Issues & PRs | reference/05-issues-and-prs.png | real list/detail, checks, commits, comments, linked issue and milestone | GitHub-connected issue/PR read capabilities and canonical plan |
| Mesh | reference/06-mesh.png | topology, selected peer, tasks, logs, live state | mesh registry/discovery/peers/tasks/capabilities and recorded events |
| CI/CD | reference/07-ci-cd.png | runs, stage graph, timings, environments, artifacts, cache charts, logs | CI model, run evidence, release/deployment records |
| Capabilities | reference/08-capabilities.png | registry catalog, dependency graph, schema runner, test/proof links | Rust capability! registry and graph |
| Evidence | reference/09-evidence.png | evidence list, links, coverage, findings, status charts | evidence ledger and claim/gate links |
| Roadmap details | reference/10-roadmap-detail.png | timeline, selected milestone details, goals, proof, release plan | same milestone projection as overview, not second source |
| Documentation | reference/11-documentation.png | doc tree, search, live OpenAPI references, quickstart, examples | existing docs hierarchy, commands registry, OpenAPI, site topology |

The 11th reference is the second dashboard composition; it should NOT create a duplicate canonical Dashboard model. The images are **illustrative fiction**. Every displayed object must be discovered from actual sources. Missing backend data means honest N/A with a path to diagnose, not invented metrics.
