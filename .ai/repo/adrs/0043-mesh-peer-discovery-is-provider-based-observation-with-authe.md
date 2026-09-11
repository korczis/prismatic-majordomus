---
schema: adr/v1
id: adr-0043
kind: adr
title: Mesh peer discovery is provider-based observation with authenticated node identity
status: proposed
date: 2026-09-11
tags: [mesh, discovery, security]
related:
  - "file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md"
  - "file:.ai/repo/adrs/0035-a-checkout-s-server-is-one-of-the-repository-s-and-its-state.md"
  - "rule:project.mesh-is-observation-not-authority"
  - "file:docs/MESH.md"
provenance:
  origin: authored
---

# 43. Mesh peer discovery is provider-based observation with authenticated node identity

## Context

ADR 0003 gave one repository one shared server, found through a lease file, and ADR
0035 taught the tool that a repository's checkouts each hold one and `server.status`
ties them together through `git worktree list`. Everything a running Majordomus can
see, it sees through the filesystem and the loopback interface: two machines running
the same repository are invisible to each other, and the operator who runs a fleet
(a laptop, a desktop, a build board) coordinates them by hand.

The peer board (`peers.rs`) already models cooperation correctly — claims, overlaps,
retention — but its scope is one process. What was missing is the layer below it:
*which other running Majordomus instances exist at all*, across processes the
filesystem cannot enumerate.

Anything that answers that question hears packets from a network, and a LAN is
hostile input. The repository also carries standing constraints this decision must
answer rather than erode: `SECURITY.md` says "local only, nothing leaves the
machine" with exactly one declared exception; the "Intentionally Absent" list in
`docs/DESIGN.md` refuses registries of named workers and unbounded stores; and ADR
0032 keeps HTTP clients, TLS and async runtimes out of the crate.

## Decision

A `mesh` subsystem (`apps/majordomus-cli/src/mesh/`), with these load-bearing rules:

1. **Identity is a key.** A node is an Ed25519 keypair created once per user and
   machine, kept under `$XDG_STATE_HOME/majordomus/node.json` (or
   `~/.local/state/...`), mode 0600, never inside a repository. The node id is a
   digest of the public key: claiming an id means holding the key. Hostnames, IP
   addresses, ports and PIDs are runtime attributes, never identity. A process run
   is an instance id; a restart keeps the node and changes the instance.
2. **One protocol.** A compact, versioned, bounded, signed JSON envelope
   (`protocol.rs`), the same over every transport. The parser refuses before it
   trusts — size, shape, version, bounds, staleness, then signature — and never
   panics on any input (property-tested). An advertisement carries public facts
   only: key, instance, endpoints, transport names, repository digests (the same
   32-hex digests `server.status` already serves — a digest discloses no path), a
   version. Never a secret, never repository content, never an environment.
3. **Providers observe; the manager decides.** A discovery mechanism implements one
   contract (`provider.rs`): start, stop, status, and raw bytes up a channel. The
   manager (`manager.rs`) owns the one verification path and the one registry;
   providers parse nothing and hold no peers. UDP multicast (preferred), UDP
   broadcast (a declared fallback, off by default) and rendezvous ship now; mDNS,
   Tailscale, Consul or anything else later is one more implementation of the same
   contract, and no surface changes when it arrives — a synthetic provider in the
   tests proves that.
4. **One registry.** `MeshRegistry` deduplicates by node id across sources: a node
   heard on multicast and handed back by a rendezvous is one record with two
   sightings. Bounded (256 nodes; trusted records never evicted for space), replay-
   protected per instance, presence decays after 60 s, records expire after 15 min.
   In memory, gone with the process. CLI, HTTP, OpenAPI, MCP and Cockpit are
   projections of it and hold nothing of their own.
5. **Discovery creates awareness, not authority.** The default trust policy is
   `deny_unknown`: a valid unknown node is recorded, visible, and trusted for
   nothing. `allowlist` trusts declared keys; `tofu` is a development convenience
   that every listing names as such. And trust gates *nothing but labeling and
   later negotiation*: there is no remote execution for a trusted node to gain,
   because this executable has none.
6. **Off by default, on by declaration.** No socket opens until the repository
   commits a `mesh` object (kind `mesh`, schema `mesh/v1`) with `enabled: true`.
   `SECURITY.md`'s posture is preserved as the *default*; a mesh is an operator's
   explicit, reviewed, versioned decision, exactly like a `deployment` object
   binding beyond loopback. Activation happens in the shared server's startup,
   after the lease and the router: discovery never delays serving, and a provider
   that cannot start is a `Failed` status, not a failed server.
7. **Every Majordomus server is a rendezvous.** `mesh.register` accepts a signed
   envelope, verifies it exactly as a datagram, and answers with the envelopes it
   holds — each verifiable end-to-end on its own signature, so a poisoned
   rendezvous can withhold nodes but cannot invent one. The "mothership" is any
   node the declaration points at; the local registry stays the only truth, and a
   rendezvous failure stretches a backoff and stops nothing else. The client rides
   the existing sixty-line HTTP/1.1 client of the MCP bridge: ADR 0032's "no HTTP
   client dependency, no TLS, no async" stands untouched.

### Against the "Intentionally Absent" list, point by point

- *No daemons*: the mesh runs inside the shared server ADR 0003 already justified,
  on the same lifecycle — it starts with a client and dies with the last peer. No
  new process exists.
- *No registries of named workers*: the mesh registry holds machines' runtimes, not
  workers, roles or personas; it is bounded, in-memory, and dies with the process.
  The peer board remains the only cooperation surface.
- *No unbounded append-only store*: 256 records, 15-minute retention, counters.
  Nothing persists but the one identity file.
- *Local only, no telemetry*: unchanged by default. A mesh datagram exists only
  after a person commits `enabled: true`, and then carries the public facts listed
  above, signed, to the network segment the declaration names. Nothing is ever
  sent to any third party; there is no phone-home endpoint to have.

## Alternatives rejected

- **Extending the `peers` module.** A session peer (an MCP client of one process)
  and a mesh node (another machine's runtime) are different things with different
  lifecycles; one word for both is how five conflicting truths start. The board
  stays; the mesh feeds it context, not rows.
- **mDNS/DNS-SD now.** It would drag a dependency or a second protocol
  implementation for a benefit the multicast provider already gives on the same
  segments. It remains one provider-contract implementation away.
- **A central mothership service.** A single mandatory rendezvous is a single
  point of failure and a canonical database of peer truth somewhere else — both
  explicitly refused. Any node serves; endpoints are a list; LAN discovery works
  with zero of them.
- **HMAC with a shared secret.** A secret that every node holds is a secret the
  protocol must never broadcast yet every node must somehow receive; public-key
  signatures need no such channel and make spoofing a key-possession problem.
- **Unsigned discovery with trust at connect time.** An unsigned advertisement can
  poison listings and steer later connections; signing at the source costs one
  dependency and removes the class.

## Consequences

- The crate gains its first cryptography dependency, `ed25519-dalek` (signatures
  only), and `getrandom` for the two moments entropy is needed. Both carry their
  justification in `Cargo.toml`.
- A per-user state file exists for the first time (`node.json`). Losing it is
  losing the node's identity — a new key is a new node, and allowlists must be
  updated. `mesh.identity` and `mesh doctor` name the path.
- The trust model authenticates *nodes*, not repositories: any holder of a valid
  key can be observed. Authorization stays where it always was — nowhere, because
  the mesh grants nothing and the HTTP surface it advertises is the same read-only
  projection it always served. Whoever later builds remote *operations* must bring
  their own authorization decision; this ADR forecloses inheriting one from
  discovery.
- Multicast does not cross segments; operators with routed fleets must declare
  rendezvous endpoints (plain HTTP — a private network or an overlay like a
  tailnet is assumed; there is deliberately no TLS in this executable).
- `mesh.status`, `mesh.nodes`, `mesh.identity`, `mesh.doctor` and `mesh.register`
  are registry capabilities like any other: typed, benchmarked, projected to every
  transport from one declaration, and the rule
  `project.mesh-is-observation-not-authority` holds the boundaries.
