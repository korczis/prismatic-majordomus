---
schema: adr/v1
id: adr-0137
kind: adr
title: Init declares the mesh, closed until a machine is enrolled
status: proposed
date: 2026-10-11
tags:
  - mesh
  - init
  - trust
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
  - file:.ai/repo/adrs/0059-the-mesh-is-on-for-this-repository-and-every-session-start-h.md
  - file:share/skeleton/ai/repo/mesh/mesh.yaml
  - file:apps/majordomus-cli/src/mesh/enrol.rs
  - file:apps/majordomus-cli/src/mesh/manager.rs
  - test:test/cases/01_init.sh
---

# 137. Init declares the mesh, closed until a machine is enrolled

## Context

ADR 0050 kept a new repository off the mesh by absence. The skeleton shipped no mesh
declaration, so no socket could open anywhere until an operator wrote one by hand, including
a trust list of public keys. That made the default safe and the mesh unusable out of the box.
A repository had to write a YAML file in a shape it had never seen, find each machine's key
with `majordomus mesh identity`, and list it, before two of its own sessions could cooperate.
The owner's order of 2026-10-10 was that the mesh must work out of the box.

Absence was the only thing keeping the default safe, because an enabled declaration
activated the mesh on every machine that read it, listed or not. A clone of a public
repository on a contributor's machine announced itself on that machine's network and dialled
the committed hubs. I2261 (PR #941) closed that: under `deny_unknown`, a machine whose own key
is not in `trust.allow` opens no socket at all.

## Decision

`majordomus init` seeds `.ai/repo/mesh/mesh.yaml`: enabled, multicast on the local segment
only (TTL 1), no broadcast, no rendezvous, `deny_unknown`, and an empty allowlist.

Under I2261's rule that declaration opens nothing on any machine. The one step that joins a
machine is `majordomus mesh enrol`, a typed capability (`mesh.enrol`, effect
`repository_mutation`). It:

- creates this machine's node identity if there is none,
- adds its public key to `trust.allow` under a comment naming the machine, idempotently,
- leaves every other line and comment of the declaration as it was,
- refuses an edit that would not parse back as a declaration before anything is written.

The operator commits the change, and every enrolled machine that pulls it trusts the new
one.

## Consequences

- The guarantee `mesh-off-by-default` holds in a stronger form. No mesh socket opens until the
  repository commits an enabled declaration **and** that declaration lists the machine's key.
  The absence of a file is no longer what keeps a machine off; the trust list is.
- A repository that wants no mesh sets `enabled: false` or deletes the file. A repository
  initialised before this decision gains the declaration with `majordomus init --extend`.
- `majordomus mesh doctor` fails its `trust` check on a machine that is not yet enrolled,
  naming the key and the remedy. The repository's `majordomus doctor` does not run it, so a
  freshly initialised repository does not fail its first doctor.
- Enrolment is a change to a tracked file, made by a person who works on that machine and
  reviewed like any other commit. Nothing enrols a machine on its own.
