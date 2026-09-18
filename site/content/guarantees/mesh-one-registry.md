+++
title = "Every discovery observation converges into one bounded registry, deduplicated by node identity across sources, replay-protected per instance, and every surface projects that registry and holds no peers of its own"
description = "A node heard on multicast and handed back by a rendezvous is one record with two"
weight = 182
[extra]
claim_id = "mesh-one-registry"
status = "guaranteed"
source = "docs/claims/mesh-one-registry.md"
+++
{% raw %}

## What it means

A node heard on multicast and handed back by a rendezvous is one record with two
sightings — never two rows with one truth each. The CLI, `/api/v1/mesh/nodes`, the MCP
tools and the Cockpit's mesh page cannot disagree about who exists, because none of
them holds a list: all of them render `mesh::MeshRegistry`. The registry is bounded
(256 records; trusted nodes are never evicted for space, so a flood cannot flush a
fleet), drops replays by per-instance sequence, decays presence after a minute of
silence, and expires records entirely after fifteen minutes. A restart — the same key,
a new instance — updates the record and increments `restarts`; it never duplicates.

## How it works

Deduplication is by node id, which is a digest of the node's Ed25519 public key —
never by address, so a machine that changes IP is still itself. Providers hand raw
bytes to the manager (`mesh::manager`), which owns the one verification path — size,
shape, version, bounds, staleness, signature — and only then touches the registry
(`apps/majordomus-cli/src/mesh/registry.rs`). The registry unit tests prove
convergence, replay drops, restart handling and the eviction bound;
`test/cases/130_mesh.sh` proves the operator path; `apps/majordomus-cli/tests/mesh.rs`
proves it over a real socket. `scripts/ci/mesh-check` (the `mesh-check` gate) refuses a
UDP socket outside `src/mesh/` and a second `MeshRegistry` construction site, so the
one-registry property is machine-held, not remembered.

## How to see it

```
majordomus mesh nodes                    # the registry, one row per node
curl http://127.0.0.1:8741/api/v1/mesh/nodes
scripts/ci/mesh-check                    # the structural half, as CI runs it
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh::registry
```

`two_sources_converge_into_one_record` and `a_replay_is_dropped_and_counted` are the
named unit proofs; `tests/mesh.rs` shows one record surviving a restart over HTTP.

## What it does not cover

The registry does not persist: a server restart forgets what was observed, by design.
And it does not verify — verification happens in the manager before anything reaches
the registry; the registry's own promise is convergence and bounds.

## Why it exists

The characteristic defect of discovery subsystems is one peer list per transport until
the surfaces disagree. One bounded registry, fed through one verification path, is the
repair — and a gate holds it so the property survives contributors who never read the
ADR.
{% endraw %}
