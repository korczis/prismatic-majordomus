<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `environment` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `environment` — Repository environment

What this checkout is right now: the project and its version, the repository and its layer, version control, the toolchains it declares, what the layer holds, the workflows a person can run, the provider projections and the local services — one typed snapshot, with a provenance entry for every value in it. The direnv banner, the Cockpit's overview and this route are renderings of the same value.

Stability: behaviorally_verified. Capabilities: 2.

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

