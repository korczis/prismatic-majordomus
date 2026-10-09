---
schema: context/v1
id: ai.repo.fleet
kind: context
title: Fleet
description: One canonical object that names the machines running this repository's mesh, the ssh destinations that reach them, and the hub each serves, so that one command brings every machine to the repository's release and forms the mesh.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/fleet-declaration/fleet-declaration.v1.schema.json, share/kinds.yaml, docs/FLEET.md]
---

# Fleet

A fleet object states, once, what the operator needs to bring every machine of the mesh to
the version this repository is at: each machine's mesh node, the ssh destinations that
reach it, and the hub it serves, if any. The mesh declaration next door (`../mesh/`) says
who may link and where the hubs listen; this object says how each machine gets there.

`majordomus fleet plan` reads it and touches nothing. `majordomus fleet status` asks each
machine what it runs. `majordomus fleet rollout` installs the release with the published
installer, fast-forwards each hub's checkout, writes and restarts the hub's service and
verifies that the hub answers at the version and that the mesh sees the node.

The schema is `fleet/v1` (`share/schemas/majordomus/fleet-declaration/fleet-declaration.v1.schema.json`); the kind
`fleet-declaration` is declared in `share/kinds.yaml`; the decision is ADR 0121; the manual is
`docs/FLEET.md`. Nothing secret belongs here: an ssh destination names a machine, and the
key that opens it stays in the operator's ssh configuration.
