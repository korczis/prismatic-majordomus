<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `deploy` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `deploy` — Deployment

The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation is a read: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client.

Stability: behaviorally_verified. Capabilities: 3.

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

Every deployment the layer declares, typed: the application, the package and binary shipped, the address the process listens on, the routes a platform polls, the resources, the machine count, the region, the build inputs, the measured budgets and the provider's own facts. An object of the kind this executable cannot read is reported with the reason rather than skipped.

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

