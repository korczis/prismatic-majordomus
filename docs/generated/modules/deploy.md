<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `deploy` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `deploy` — Deployment

The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation is a read: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client.

Stability: behaviorally_verified. Capabilities: 4.

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

## `deploy.verify` — What the deployed surfaces are serving

Live verification: every surface the change reaches — the published site, the published release metadata, each active deployment — is asked for the identity it states (the commit, version or tag at its own address) and compared with what this checkout expects. A surface stating an older identity is stale, one that does not answer is unreachable, and neither is a pass: a deploy command that exited 0 with the old revision still live is exactly what this refuses. The request carries no header and the evidence carries no body beyond the fields compared. The one capability of this executable that reaches the network, and it reaches only addresses the repository itself declares.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_deploy_verify` |
| HTTP | `GET /api/v1/deployments/verify` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::deploy |
| tags | deployments, verification, evidence, live |

| input | type | required | description |
|---|---|---|---|
| `expected_commit` | string or null | no | The commit every target is expected to serve. Absent means this checkout's HEAD. |
| `targets` | array or null | no | The targets to ask, by id (`pages`, `release`, a deployment's id). Absent means
every target that applies. |
| `changed` | array or null | no | The changed paths to derive applicability from. Absent means every target that
exists is asked, which is the question after a deployment. |

Output: `DeploymentVerification`.

