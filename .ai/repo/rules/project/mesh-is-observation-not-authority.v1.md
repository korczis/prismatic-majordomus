---
id: project.mesh-is-observation-not-authority
version: 1
kind: rule
title: The mesh observes nodes; it authorizes nothing and holds one registry
description: Mesh discovery is provider-based, signed and canonical — providers emit observations into the one MeshRegistry through the manager's single verification path, no discovery event grants execution or trust by presence, and the mesh is off until a repository declares it.
statement: Every mesh observation passes the manager's one verification path into the one MeshRegistry; a discovery mechanism is an implementation of the provider contract and maintains no peer state of its own; no discovery event alone grants trust, authorization or execution; and no mesh socket opens without an enabled mesh declaration in the repository.
status: active
class: blocking
depends_on: []
tags: [mesh, security, doctrine]

x-majordomus:
  tests: [scripts/ci/mesh-check, test/cases/130_mesh.sh, apps/majordomus-cli/tests/mesh.rs]
---

# Rationale

A LAN is hostile input, and a discovery subsystem fails in two characteristic ways:
transports that each keep their own peer list until the CLI, the API and the Cockpit
disagree about who exists; and presence that quietly becomes permission, where being
heard on a multicast group is treated as being one of us. ADR 0050 closes both: one
registry fed through one verification path, and a trust verdict that labels records
without granting anything — there is no remote execution in this executable for a
trusted node to gain, and whoever adds remote operations later must bring their own
authorization decision rather than inheriting one from discovery. The off-by-default
declaration keeps `SECURITY.md`'s "nothing leaves the machine" true until a person
commits the object that says otherwise.

# Required behaviour

1. UDP sockets and discovery transports live under `apps/majordomus-cli/src/mesh/`
   and nowhere else in the crate.
2. A discovery mechanism implements `mesh::provider::MeshProvider` and hands raw
   bytes to the manager; it parses no envelope, verifies no signature, evaluates no
   trust and keeps no peer records. Adding a mechanism edits no consumer, no surface
   and no registry.
3. `mesh::registry::MeshRegistry` is the only peer store. CLI, HTTP, OpenAPI, MCP
   and Cockpit render it through the `mesh` capability module; none holds a second
   list, and the Cockpit ships no fixture nodes.
4. Every observation is verified in `mesh::manager` — bounds, shape, version,
   staleness, signature — before trust is evaluated, and the default trust policy is
   `deny_unknown`. A trust verdict changes labels and candidate-sharing, never
   permissions.
5. The mesh activates only from an enabled `mesh` declaration (kind `mesh`, schema
   `mesh/v1`) during shared-server startup, never blocks startup, and a provider
   failure degrades that provider alone.
6. An advertisement carries no secret, no credential, no environment value and no
   repository content; repositories appear only as digests.

# Failure behaviour

`scripts/ci/mesh-check` exits 10 naming the file that opens a UDP socket outside
`src/mesh/`, the projection that bypasses the registry, or the declaration drift it
finds; the `mesh-check` gate in `.ai/repo/ci/gates.yaml` runs it for every change to
the crate or to this rule. What a static check cannot see — that a verdict grants
nothing — is held by the unit and integration tests named below and by review.

# Verification

`scripts/ci/mesh-check` (the gate), `cargo test --manifest-path
apps/majordomus-cli/Cargo.toml --lib mesh` (protocol refusals, replay, trust,
dedup, bounded registry, zero-registration synthetic provider), and
`apps/majordomus-cli/tests/mesh.rs` (two runtimes discover each other over loopback
rendezvous and converge to one record per node; a spoofed envelope is refused).
