<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `lifecycle` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `lifecycle` — Session lifecycle

The episode lifecycle as an operator sees it: every open episode of this checkout's store rather than only the one the pointer follows, the episodes that can no longer close themselves, the commit this process is answering about against the commit the repository is on, what each provider's adapter declares it can do, and the tracked records a clone receives. Read from the local half of the layer and from the index; written by nothing here.

Stability: behaviorally_verified. Capabilities: 5.

## `lifecycle.closed` — The records a clone receives

The tracked, durable projection of closed episodes: how many there are, how many closed on this branch, and the newest twenty. This is the only half of the subsystem that survives a clone; everything under .ai/local/ names this machine and travels nowhere.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_lifecycle_closed` |
| HTTP | `GET /api/v1/lifecycle/closed` |
| cache | process, 2 entries, 30s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::lifecycle |
| tags | continuity, session, record |

Input: none.

Output: `ClosedSessions`.

## `lifecycle.episodes` — Every open episode

Every open episode in this checkout's store, with the provider session that owns it, the worktree and branch it opened on, where it stands, and what the ledger last saw it do. `continuity.state` reports the one episode the pointer resolves to, which is right for a briefing and blind to every other window open on the same worktree.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_lifecycle_episodes` |
| MCP resource | `majordomus://lifecycle/episodes` |
| HTTP | `GET /api/v1/lifecycle/episodes` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::lifecycle |
| tags | continuity, session, episode |

Input: none.

Output: `Episodes`.

## `lifecycle.providers` — What each provider's adapter declares

Per provider: the lifecycle events its adapter declares in the provider's own vocabulary, whether it can archive prompts, and which of this repository's enforcement entries name its hook. Declared in share/providers.yaml and in the policy; never inferred from whether a shim happens to be on disk.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_lifecycle_providers` |
| HTTP | `GET /api/v1/lifecycle/providers` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::lifecycle |
| tags | continuity, session, provider |

Input: none.

Output: `ProviderLifecycles`.

## `lifecycle.recovery` — What the store needs somebody to do

Open records that can no longer close themselves, temporary files a killed close left in the tracked sessions section, whether the pointer has been migrated to the per-provider layout, and the arithmetic that says whether every episode the ledger has seen start is accounted for. Every finding carries the command that clears it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_lifecycle_recovery` |
| MCP resource | `majordomus://lifecycle/recovery` |
| HTTP | `GET /api/v1/lifecycle/recovery` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::lifecycle |
| tags | continuity, session, recovery |

Input: none.

Output: `Recovery`.

## `lifecycle.runtime` — The commit this process is answering about

The commit the served index was built at, against the commit the repository is on, read on this call. A server that froze its index at start-up answers every question about a commit from hours ago with nothing in the answer saying so; this is the reading that can tell.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_lifecycle_runtime` |
| HTTP | `GET /api/v1/lifecycle/runtime` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::lifecycle |
| tags | continuity, session, server |

Input: none.

Output: `RuntimeView`.

