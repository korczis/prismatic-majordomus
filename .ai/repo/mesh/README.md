---
schema: context/v1
id: ai.repo.mesh
kind: context
title: Mesh
description: One canonical object that says whether this repository's servers participate in mesh discovery, over which transports, and whom they trust; absent or disabled means no socket opens.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/mesh-declaration/mesh-declaration.v1.schema.json, share/kinds.yaml, docs/MESH.md]
---

# Mesh

A mesh object states, once, everything the shared server needs to decide whether and how
to discover other running Majordomus instances: the master switch, the multicast group
and port, whether broadcast is a permitted fallback and toward which networks, the
rendezvous endpoints for segments multicast cannot cross, and the trust policy with its
allowlisted keys. The shared server reads it at startup and activates the mesh runtime
from it; nothing else interprets it, and no environment variable overrides it.

The default posture of the repository — nothing leaves the machine — is preserved by
absence: no object here, or `enabled: false`, and not one socket opens. Committing an
enabled object is the operator's explicit, reviewed decision, exactly as a deployment
object is for binding beyond loopback (ADR 0043).

The schema is `mesh/v1` (`share/schemas/majordomus/mesh-declaration/mesh-declaration.v1.schema.json`);
the kind `mesh-declaration` is declared in `share/kinds.yaml`; the invariants are
`project.mesh-is-observation-not-authority`; the architecture is `docs/MESH.md` and ADR
0043. Trust `allow` entries are Ed25519 *public* keys — nothing secret belongs in this
directory, and the node's signing key never leaves the machine it was created on.
