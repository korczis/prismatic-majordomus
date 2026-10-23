<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `served` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->
# Module `served` — Served deployments

Whether a deployment serves the commit it was meant to: the build identity a site serves, read from outside the way a visitor reads it, judged by commit containment and recorded so that a later reader can re-judge it. Only `served` passes; a site that could not be reached, an identity that could not be read, a build from an uncommitted tree and a served commit this clone does not hold are each an unanswered question, never a yes. Observing reaches the network and is a command of the trusted command line; reading the recorded standing is a read on every surface.

Stability: behaviorally_verified. Capabilities: 3.

## `served.forge` — Ask the forge what it recorded as deployed and released

Asks the forge, through its own command-line tool and with the caller's credential, for the newest deployments to every environment and the newest releases, in one query, and records the answer in the checkout-local state `deploy.observed` reads. Exit 0 when the forge answered and the answer was recorded, 12 when it could not be asked (no tool, no credential, no network, a repository the forge does not know): what was recorded before then stands, with its age.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus served forge` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::served |
| tags | deployment, evidence, forge |

Input: none.

Output: `ForgeObserved`.

## `served.observe` — Observe what a deployment serves

Fetches the build identity a deployment serves (one bounded probe, no shell): for a `deployment` naming an object with a provider, its liveness route at the provider's URL; otherwise `build.json` under the configured base URL. It judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks).

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus served observe` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::served |
| tags | deployment, evidence, verification |

| input | type | required | description |
|---|---|---|---|
| `deployment` | string or null | no | The name the observation is recorded under; `pages` when absent. |
| `url` | string or null | no | The base URL; when absent, the `provider.url` of the deployment object `deployment`
names, else `base_url` of `site/config.toml` (I3822). |
| `identity` | string or null | no | The identity under the base; when absent, the `health.liveness` route of the
deployment object `deployment` names (which answers `commit` and `dirty`), else
`build.json`. |
| `commit` | string or null | no | The commit expected to be served, any revision git resolves; `HEAD` when absent. |
| `timeout_seconds` | integer or null | no | The bound on the probe in seconds; ten when absent. |
| `dry_run` | boolean | no | Judge without recording. Recording is the default, because an observation nobody
kept is a measurement nobody can read. |

Output: `Observed`.

## `served.show` — What each deployment was last seen serving

The newest recorded observation of each deployment, re-judged against a commit (HEAD unless named), and for a deployment the layer declares as an object its status derived from that: `active` only while it serves the commit. A record keeps proving every commit its served build contains and stops proving the moment the commit asked about is not among them; a record that received nothing proves nothing. Asks no network and writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_served` |
| MCP resource | `majordomus://served` |
| HTTP | `GET /api/v1/served` |
| CLI | `majordomus served show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::served |
| tags | deployment, evidence, verification |

| input | type | required | description |
|---|---|---|---|
| `commit` | string or null | no | The commit each record is re-judged against, any revision git resolves; `HEAD` when
absent. |
| `deployment` | string or null | no | Only this deployment. |

Output: `ServedStanding`.

