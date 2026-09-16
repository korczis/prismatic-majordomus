<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `served` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `served` — Served deployments

Whether a deployment serves the commit it was meant to: the build identity a site serves, read from outside the way a visitor reads it, judged by commit containment and recorded so that a later reader can re-judge it. Only `served` passes; a site that could not be reached, an identity that could not be read, a build from an uncommitted tree and a served commit this clone does not hold are each an unanswered question, never a yes. Observing reaches the network and is a command of the trusted command line; reading the recorded standing is a read on every surface.

Stability: behaviorally_verified. Capabilities: 2.

## `served.observe` — Observe what a deployment serves

Fetches the build identity the site serves (`build.json` under the configured base URL, one bounded probe, no shell), judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks).

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
| `url` | string or null | no | The site's base URL; `base_url` of `site/config.toml` when absent. |
| `identity` | string or null | no | The identity file under the base; `build.json` when absent. |
| `commit` | string or null | no | The commit expected to be served, any revision git resolves; `HEAD` when absent. |
| `timeout_seconds` | integer or null | no | The bound on the probe in seconds; ten when absent. |
| `dry_run` | boolean | no | Judge without recording. Recording is the default, because an observation nobody
kept is a measurement nobody can read. |

Output: `Observed`.

## `served.show` — What each deployment was last seen serving

The newest recorded observation of each deployment, re-judged against a commit (HEAD unless named). A record keeps proving every commit its served build contains and stops proving the moment the commit asked about is not among them; a record that received nothing proves nothing. Asks no network and writes nothing.

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

