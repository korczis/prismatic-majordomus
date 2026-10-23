<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `deploy` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->
# Module `deploy` — Deployment

The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation over HTTP and MCP is a read; the one command, assembling the tree a provider receives, writes outside the checkout and is a command line only: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client.

Stability: behaviorally_verified. Capabilities: 5.

## `deploy.check` — Would these deployments work

Every refusal the declared deployments earn locally: a health route no capability registers, a package or binary the workspace does not produce, a build input that does not resolve, more machines running than exist, a hosted process that would bind loopback. Each names the file, the key, the value observed and the correction. Nothing here contacts the provider.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_deploy_check` |
| HTTP | `GET /api/v1/deployments/check` |
| cache | process, 2 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployment, diagnostics |

Input: none.

Output: `DeploymentCheck`.

## `deploy.get` — One deployment

One deployment by its identity, typed, with the repository-relative file it was read from.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_deployment` |
| HTTP | `GET /api/v1/deployment` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployment |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The deployment's `id`. |

Output: `DeploymentView`.

## `deploy.list` — Deployments

Every deployment the layer declares, typed: the application, the package and binary shipped, the address the process listens on, the routes a platform polls, the resources, the machine count, the region, the build inputs and the measured budgets. An object of the kind this executable cannot read is reported with the reason rather than skipped.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_deployments` |
| MCP resource | `majordomus://deployments` |
| HTTP | `GET /api/v1/deployments` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployment |

Input: none.

Output: `DeploymentList`.

## `deploy.observed` — What was deployed, and where

Every deployment the forge recorded, to every environment it has one in — GitHub Pages among them — with the commit, the ref, who created it, how it ended, where it is served and the run that did it; every environment described by its newest deployment; and every release the forge published. Read from the record `majordomus served forge` wrote when it last asked the forge: nothing here reaches the network, and a checkout that has never asked answers `observed: false` with what to run.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_deployments_observed` |
| HTTP | `GET /api/v1/deployments/observed` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployment, delivery |

Input: none.

Output: `ObservedDeployments`.

## `deploy.slug` — Assemble the tree a provider receives

Writes, into a directory outside the checkout that is absent or empty, the tree the deployment's provider receives (ADR 0219): the object's build inputs except the package the executable is compiled from, read from git at one commit (HEAD unless named); the provider's generated files of that commit at the slug's root, each held to what the object renders and the hooks made executable; the provider's buildpack line; the executable built at that commit; and a record naming the deployment, the commit, the executable's digest and the provider. Refuses an out directory inside the checkout or not empty, a deployment that names no provider, a build input the commit does not track, a provider file the object does not render, and two files that would land on one path. Changes nothing in the repository and reaches no network; pushing the slug is the provider workflow's.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus deploy slug` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployment, delivery |

| input | type | required | description |
|---|---|---|---|
| `deployment` | string or null | no | The deployment's `id`; when absent, the one deployment that names a provider. |
| `commit` | string or null | no | The commit every file is read at, any revision git resolves; `HEAD` when absent. |
| `executable` | string | yes | The executable built at that commit. |
| `out` | string | yes | The directory the slug is written into: outside the checkout, absent or empty. |
| `site` | string or null | no | The site built at that commit (`scripts/site-build --serve`), required exactly when the
deployment declares `build.site`: the object says the slug carries one, this says where
the build left it. |

Output: `Slug`.

