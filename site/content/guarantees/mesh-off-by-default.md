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

## How to see it

```
majordomus mesh status          # inactive, with the reason, in this repository
majordomus mesh doctor          # the declaration check names the committed default
grep enabled .ai/repo/mesh/majordomus.yaml
```

`apps/majordomus-cli/tests/mesh.rs` (`a_disabled_declaration_opens_nothing_and_says_why`)
is the executable form: a server over a disabled declaration answers `active: false`
with the reason and creates no identity file.

## What it does not cover

It does not police what an *enabled* mesh discloses — that is the protocol's bound
(no secrets, no paths, digests only) and the trust policy's business. And it cannot
stop an operator from enabling the mesh on a network they should not; it only makes
that a visible, committed decision.

## Why it exists

The repository's security posture is "nothing leaves the machine", and a discovery
subsystem is exactly the kind of feature that erodes such a posture silently. Making
absence the default and enablement a reviewed commit keeps the posture true until a
person decides otherwise (ADR 0043).
{% endraw %}
