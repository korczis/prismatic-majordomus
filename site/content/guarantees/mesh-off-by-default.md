+++
title = "No mesh discovery socket opens until the repository commits an enabled mesh declaration, and a disabled or absent declaration is reported as the reason, not an error"
description = "The repository's standing posture — nothing leaves the machine — survives the existence"
weight = 165
[extra]
claim_id = "mesh-off-by-default"
status = "guaranteed"
source = "docs/claims/mesh-off-by-default.md"
+++
{% raw %}

## What it means

The repository's standing posture — nothing leaves the machine — survives the existence
of the mesh subsystem. A clone with no `.ai/repo/mesh/*.yaml`, or one whose declaration
carries `enabled: false` (this repository's committed default), runs exactly as before:
no UDP socket, no broadcast, no rendezvous request, no node identity file. Turning the
mesh on is a person editing the declaration, having it reviewed, and committing it — an
auditable act in history, never an environment variable or a flag someone forgets was
set.

Off is also an *answer*: `mesh.status` (and `majordomus mesh status`, and the Cockpit's
mesh page) reports `active: false` with the reason — no declaration, declaration
disabled, or what failed — so an operator debugging a silent mesh reads why instead of
guessing.

## How it works

The shared server reads the declaration (kind `mesh-declaration`, schema `mesh/v1`,
source class in `.ai/repo/knowledge/sources.yaml`) during startup, before its workers
take a request, and activates `mesh::MeshRuntime` only from `enabled: true`
(`apps/majordomus-cli/src/shared.rs`). Every other outcome calls `decline(reason)` and
starts nothing. `apps/majordomus-cli/tests/mesh.rs` proves both directions over a real
socket: a disabled declaration answers with its reason and creates no identity file,
and an enabled one activates before the first request lands. ADR 0043 records the
decision and answers the repository's "Intentionally Absent" list point by point.
{% endraw %}
