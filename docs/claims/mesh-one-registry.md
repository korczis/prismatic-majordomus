# Every discovery observation converges into one bounded registry, deduplicated by node identity across sources, replay-protected per instance, and every surface projects that registry and holds no peers of its own

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
