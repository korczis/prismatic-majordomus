<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `environment` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.8.0 -->
# Module `environment` — Repository environment

What this checkout is right now: the project and its version, the repository and its layer, version control, the toolchains it declares, what the layer holds, the workflows a person can run, the provider projections and the local services — one typed snapshot, with a provenance entry for every value in it. The direnv banner, the Cockpit's overview and this route are renderings of the same value.

Stability: behaviorally_verified. Capabilities: 3.

## `environment.explain` — Where an environment value came from

The provenance of the snapshot: for each field, what decided it — a compile-time constant, a file, a command, or the cache — which resolver read it, and how far it can be trusted. Narrow it to one field, or to a prefix, by name.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_environment_explain` |
| HTTP | `GET /api/v1/environment/explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::environment |
| tags | environment, provenance, introspection |

| input | type | required | description |
|---|---|---|---|
| `field` | string or null | no | The field in dotted form (`services.url`, `capabilities.objects`); every field when
absent. |

Output: `EnvironmentProvenance`.

## `environment.preflight` — Whether Majordomus is in force here, and what proves it

One verdict per claim about this checkout — git; the episode, its briefing, the task and the handover; the policy, the rule corpus and the ADRs; the shared server and the MCP, API and Cockpit surfaces it serves; the peer board; recorded test runs, rule enforcement, provider projections, generated documentation and the deployment — each `verified`, `active`, `fresh`, `stale`, `degraded`, `unavailable`, `failed`, `unknown` or `not_applicable`, with the evidence it rests on. A verdict that asserts something is in force cannot be produced without evidence. The same value the command line prints and the entry banner summarises.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_preflight` |
| MCP resource | `majordomus://environment/preflight` |
| HTTP | `GET /api/v1/environment/preflight` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::environment |
| tags | environment, governance, verification, evidence |

| input | type | required | description |
|---|---|---|---|
| `probe` | boolean | no | Ask the loopback address a server of this checkout published, and its peer board.
Off by default: a served request answers from what this process can see, and when
this process holds the checkout's lease that is itself the evidence. |

Output: `Preflight`.

## `environment.status` — The repository environment

One snapshot of this checkout: project identity, repository identity, version control, declared toolchains, what the layer holds counted per kind, the workflows the runner describes, the provider projections against the policy that renders them, and the local services with the address a running server published. Every value carries where it came from.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_environment` |
| MCP resource | `majordomus://environment` |
| HTTP | `GET /api/v1/environment` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::environment |
| tags | environment, repository, introspection |

| input | type | required | description |
|---|---|---|---|
| `probe_services` | boolean | no | Contact the local address a running server published, to say whether it answers.
Off by default: a served request should not open a socket to another server on
behalf of its caller, and the caller usually is that server. |

Output: `RepositoryEnvironment`.

