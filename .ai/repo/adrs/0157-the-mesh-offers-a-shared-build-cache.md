---
schema: adr/v1
id: adr-0157
kind: adr
title: A mesh node may offer a shared build cache, peers discover and select it, and compilation never depends on it
status: proposed
date: 2026-10-13
tags:
  - mesh
  - build
  - cache
  - capability
related:
  - file:.ai/repo/adrs/0055-disk-is-the-fifth-quantity-reclaimed-by-predicate-and-bounded-before-a-build.md
  - file:.ai/repo/adrs/0139-worktrees-share-what-cargo-compiles-on-the-way-and-keep-thei.md
  - file:.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md
  - file:docs/MESH.md
  - file:docs/OFFLOAD.md
provenance:
  origin: authored
---

# 157. A mesh node may offer a shared build cache, peers discover and select it, and compilation never depends on it

## Context

On 2026-10-13 the cluster could not derive: both declared offload hosts were under their
10 GiB free-space minimum (7 and 1), the owner's Mac sat at 100% disk and was cleaned by
hand, and a derive of one documentation pull request cost thirteen minutes of compiler time
three times in two hours, each time from a cold cache. Every machine compiles the same
Rust for the same commits, each into a cache of its own: ADR 0139 shares what cargo
compiles between the worktrees of one machine and says, in terms, that `sccache` "stays
optional and is not part of this decision". Nothing shares compilation between machines.

The owner's order, the same day, names the shape: one node with a large disk (an aarch64
Jetson with 783 GB free) holds a shared object store, every node uses its local disk first
and that store second, and the result is verified and validated, not assumed.

## Decision

1. **A capability, not a host.** A node may *offer* `build-cache`: `protocol: s3`,
   `provider` (MinIO first), `scope: mesh`, a non-secret endpoint, a bucket, a priority,
   capacity and headroom, and health. Nothing canonical names a machine; the first
   provider is whichever node offers it. Clients ask for `build-cache/s3`.
2. **Offered at run time, not committed.** The repository's declaration holds policy only —
   which providers and protocols are accepted, bucket naming, expiry, the headroom floor.
   An endpoint is a fact about a running node and travels as a signed, expiring mesh
   record (ADR 0044's board, the journal's liveness): a provider that stops is stale
   within its lifetime and nobody edits a client. A private address is never written to
   a tracked file or a published page.
3. **Selection is a pure function** of the offers, the policy and the measured latency:
   healthy first, then scope, priority, headroom, round-trip time, and the node id as the
   last, deterministic tie-break. It answers with the chosen offer *and why*, including
   the candidates it passed over. With no offer it answers "none", and that is a normal
   answer.
4. **The client is derived, never configured by hand.** `majordomus env` emits the sccache
   configuration for the selected offer — the local disk as the first level, the offer as
   the second — and `.envrc` stays the thin adapter it is (`project.envrc-is-an-adapter`).
   With no offer, or a sccache that cannot do two levels, the client is local-only, and
   with no sccache at all the build is exactly what it was.
5. **The cache is acceleration, never a dependency.** A provider that is down, slow,
   full, or refusing a credential must leave the build correct and not much slower: the
   client falls back to the local level, the failure is a diagnostic naming the cause
   without a secret, and a probe is bounded (the 80 ms budget of `environment/services.rs`;
   unknown, never "down").
6. **Nothing portable is assumed.** The cache key is sccache's: toolchain, target and
   flags. A hit across architectures is not claimed; the proof of a shared hit is made
   between two nodes of one platform, and where there are not two, it says so.
7. **Secrets stay out of the model.** The offer, the selection, the CLI, the API, the MCP
   tool, the UI and the docs carry no key. A client credential is provisioned onto a node
   with the owner's SSH access into a file of mode 0600 outside the repository, one key
   per node that can be revoked, and a write without one is refused by the store.

## Consequences

- The slowest thing a node does — compile Rust it already compiled elsewhere — becomes a
  read from a disk on the LAN, for every node that has the same toolchain and target.
- A new provider is a record, not a code change, and a second provider is ranked, not
  configured.
- A new operation family (read of offers and selection, provisioning, verification) joins
  the registry by `capability!` and is projected to the CLI, MCP, HTTP and OpenAPI by
  `majordomus generate`, as `docs/CAPABILITIES.md` requires.
- The mesh's advertisement datagram is size-bound; the offer rides the journal as an
  expiring record and not the UDP advertisement.

## Alternatives rejected

- **A committed list of endpoints.** It is a second registry that rots at the first
  address change, and it publishes a private address.
- **Distributed compilation** (`sccache-dist`, icecream). It moves compilers across
  machines of one platform and is a much larger trust and failure surface than a cache.
- **Making the Jetson mandatory, or naming it.** A provider that cannot be absent is not
  a cache; and the first host is an accident of which disk was large.
- **Writing `~/.cargo/config.toml` on peers.** It is a user's file. The derived
  configuration is exported through the environment, and a missing sccache is a
  diagnostic, not a silent edit.

## Verification

The model and the selection are proved by unit tests (discovery, determinism, no offer,
stale offer). The client derivation, the fallback with the provider stopped and the
refusal of an unauthenticated write are proved by `test/cases/3011_a_mesh_build_cache.sh`
against a stub S3 endpoint. The claim that a second node hits what the first wrote is
proved on two real nodes of one platform, or reported as not proved. Acceptance is the
contract of the pack the owner filed
(`majordomus-jetson-mesh-cache-claude-pack/contracts/acceptance.md`), item by item.
