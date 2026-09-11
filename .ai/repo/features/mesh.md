---
schema: feature/v1
id: mesh
kind: feature
title: Running instances find each other, and prove who they are
short_title: Mesh
headline: A running Majordomus can discover the others — over multicast, broadcast or a rendezvous — and every observation is signed, converged into one registry, and trusted for nothing by default.
summary: A node is an Ed25519 keypair kept per user and machine; one signed, versioned, bounded envelope travels over every discovery transport; providers only observe while the manager owns the single verification path and the single registry; trust is an explicit policy that defaults to deny_unknown; and nothing opens a socket until the repository commits an enabled mesh declaration. CLI, HTTP, OpenAPI, MCP and the Cockpit render the same runtime state.
status: draft
weight: 45
featured: false
areas: [coordination]
modules: [mesh]
rules: [project.mesh-is-observation-not-authority]
docs: [docs/MESH.md]
adrs: [adr-0043]
claims: [mesh-observation-not-authority, mesh-one-registry, mesh-off-by-default]
use_cases: []
cockpit: [mesh]
related: [coordination]
tags: [mesh, discovery, network, security]
---

## What it does

With an enabled `mesh` declaration in the repository, the shared server announces its
existence — a compact envelope signed by the machine's node key — over UDP multicast,
optionally over broadcast, and to any rendezvous endpoints the declaration names, and
listens for the same from others. Every datagram heard anywhere passes one verification
path (bounds, shape, version, staleness, signature) and lands in one registry,
deduplicated by node identity: the same node heard on two transports is one record with
two sightings, and a restart is the same node with a new instance, never a duplicate.
`majordomus mesh status`, `mesh nodes`, `mesh identity` and `mesh doctor` render it on
the command line; `/api/v1/mesh*`, the MCP tools and the Cockpit's mesh page render the
same runtime state. Any Majordomus server is a rendezvous for any other: `mesh.register`
verifies a presented envelope like any datagram and answers with candidates that verify
end-to-end on their own signatures.

## What it does not do

It grants nothing. A discovered node — even a trusted one — gains no execution, no
authorization and no access; trust labels records and shapes candidate-sharing, and
whoever builds remote operations later must bring their own authorization decision. It
sends nothing until a person commits `enabled: true`, advertises no secret, no path and
no repository content (repositories travel as digests), and persists nothing but the one
identity file under the user's state directory — the registry is process memory and dies
with the server.
