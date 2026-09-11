+++
title = "The mesh: discovery of running Majordomus instances"
description = "the mesh: how running Majordomus instances discover each other — Ed25519 node identity, one signed bounded envelope over multicast, broadcast and rendezvous, one registry, deny-unknown trust, the off-by-default declaration, the threat model, and how to operate and extend it"
weight = 47
[extra]
source = "docs/MESH.md"
+++

{% raw %}

How one running Majordomus learns that others exist, and proves which one it is. The
decision is ADR 0043; the invariants are `project.mesh-is-observation-not-authority`;
the implementation is `apps/majordomus-cli/src/mesh/`. This document is the operator's
and contributor's view: what runs, what travels, what is trusted, and what can go wrong.

The one sentence that governs everything here:

> Discovery creates awareness, not authority.

## The shape

```
mesh declaration (.ai/repo/mesh/<id>.yaml, kind mesh, schema mesh/v1)
        │  read once at shared-server startup; absent or disabled ⇒ nothing opens
        ▼
MeshRuntime (on the capability Context of the process)
        │
        ├─ NodeIdentity      ~/.local/state/majordomus/node.json — an Ed25519 keypair,
        │                    created once per user and machine; node id = digest(key)
        ├─ Beacon            this node's signed advertisement, one rising sequence
        │
        ├─ providers         each an implementation of mesh::provider::MeshProvider:
        │     udp_multicast  the preferred LAN mechanism (group, port, TTL declared)
        │     udp_broadcast  a declared fallback: disabled | auto | explicit networks
        │     rendezvous     registers with declared endpoints over plain HTTP
        │                    — providers hear bytes and hand them up; nothing more
        ▼
manager (one thread)         the single verification path, in order:
        │                    size → shape → version → bounds → staleness → signature
        │                    → own-datagram skip → trust policy → registry
        ▼
MeshRegistry                 the one store: deduplicated by node id, replay-protected
        │                    per instance, presence decays (60 s), records expire
        │                    (15 min), bounded (256; trusted never evicted for space)
        ▼
projections                  mesh.status · mesh.nodes · mesh.identity · mesh.doctor ·
                             mesh.register — one declaration each, so the CLI, HTTP,
                             OpenAPI, MCP and the Cockpit cannot disagree
```

## Identity

A node is a key, not an address. The keypair lives in the user's state directory
(`$XDG_STATE_HOME/majordomus/node.json`, else `~/.local/state/majordomus/node.json`),
mode 0600, created the first time a mesh activates on the machine; `majordomus mesh
identity` prints the public half and the path. The node id is a digest of the public
key, so carrying somebody else's id is impossible — the id is never carried at all,
only derived by the verifier. A process run is an *instance*: a machine that restarts
keeps its node id and changes its instance, which is how the registry tells "the same
node came back" (one record, `restarts` incremented) from a duplicate.

Losing the file is losing the identity: a new key is a new node, and allowlists that
named the old key must be updated. Nothing else is stored; the registry itself is
process memory.

## The envelope

One protocol over every transport (`mesh/protocol.rs`): a JSON envelope of at most
1200 bytes, versioned (`v: 1`), carrying the public key, instance, display name,
sequence, timestamp, up to four `host:port` endpoints, transport names, repository
digests and the executable version — signed with Ed25519 over the advertisement's
canonical bytes. Parsing is written for hostile input: bounded before it is read,
refused before it is trusted, and no input of any shape panics (a property test feeds
it arbitrary bytes). Every refusal is counted by reason — oversized, malformed,
version, bounds, stale, signature — and `mesh.status` shows the counters, because a
mesh that hears garbage should say so instead of being quietly empty.

What an advertisement never carries: secrets, credentials, tokens, environment values,
paths, repository content. Repositories appear as the same 32-hex digests
`server.status` already serves; a digest can be recognised by someone who serves the
same repository and inverted by no one.

## Trust

Trust is decided above the protocol, by declared policy, and the default is that no
one is trusted:

<div class="overflow-x-auto" tabindex="0">

| policy | a valid unknown node is | notes |
|---|---|---|
| `deny_unknown` (default) | observed, listed, trusted for nothing | the safe resting state |
| `allowlist` | observed; listed keys are trusted | `trust.allow` holds Ed25519 public keys (hex) |
| `tofu` | trusted on first use | a development convenience; every listing names `tofu` as the reason |

</div>


Under every policy, a node id that reappears under a different key is rejected and
stays listed as rejected — an operator debugging a mesh needs to see what was refused.
And under every policy, trust changes *labels and candidate-sharing only*. There is no
remote execution in this executable for a trusted node to gain; the HTTP surface a
node advertises is the same read-only projection it always served. Whoever builds
remote operations later must bring their own authorization decision (ADR 0043
forecloses inheriting one from discovery).

## Transports

**Multicast** is the preferred LAN mechanism: one socket joined to the declared group
(default `239.255.77.77:7741`, TTL 1 — the local segment and no further), a listener
thread and a jittered announcer. Multicast does not work on every network, and the
provider assumes nothing: a socket that cannot bind (typically another local server
already listening) or a group that cannot be joined is a `Failed` provider status with
the reason, and every other provider runs on.

**Broadcast** is a declared fallback, never on by default: `disabled` | `auto` (the
limited broadcast address) | `explicit` (the directed broadcast addresses listed). The
same envelope, the same port; it transmits, and listens only when it is the sole UDP
provider (two sockets cannot share the port).

**Rendezvous** crosses the segments multicast cannot. Every Majordomus server *is* a
rendezvous: `POST /api/v1/mesh/register` verifies a presented envelope exactly as a
datagram, records it as a rendezvous observation, and answers with its own envelope
and up to 32 candidates — each carried verbatim as signed, so a candidate verifies
end-to-end at the consumer and a poisoned rendezvous can withhold nodes but cannot
invent one. The client registers with each declared endpoint on its interval, backs
off exponentially (up to ×8) when one is unreachable, and never hammers. The transport
is plain HTTP through the same sixty-line client the MCP bridge uses: there is
deliberately no TLS in this executable, so rendezvous endpoints belong on a private
network or an overlay (a tailnet, a WireGuard mesh) that provides the transport
security itself.

## Lifecycle

The mesh runs inside the shared server (ADR 0003) and nowhere else: it starts after
the lease and the router — discovery never delays serving — and dies with the process.
Startup is idempotent (a second activation is a no-op), never blocks on the network,
and a disabled or malformed declaration leaves the runtime inactive with the reason on
`mesh.status`. Multiple shells attaching to the repository share the one server and
therefore the one mesh; no daemon exists, no per-terminal socket is ever opened.

A record's life: observed → (trust verdict) → present while advertisements arrive →
absent after 60 s of silence → expired out of the registry after 15 min. Sequence
numbers are per instance; a replayed datagram is dropped and counted.

## Operating it

```
majordomus mesh doctor        # every prerequisite, proved on this machine alone
majordomus mesh identity      # this machine's node id and public key (for allowlists)
majordomus mesh status        # the running server's mesh: providers, tallies, refusals
majordomus mesh nodes         # the observed nodes, one row each
```

Enable it by editing the repository's declaration (`.ai/repo/mesh/majordomus.yaml`):
set `enabled: true`, choose the trust policy — `deny_unknown` plus `allow:` keys from
`mesh identity` on each of your machines is the recommended shape — commit, and
restart the server (`majordomus serve stop && majordomus serve ensure`). The committed
default is `enabled: false`: the repository's "nothing leaves the machine" posture
holds until a person decides otherwise, and that decision is a reviewed commit, not an
environment variable.

Troubleshooting is `mesh doctor` first (it names the broken prerequisite), then
`mesh status` on each machine: a provider's `Failed` detail says what its socket could
not do; the refusal counters say what arrived and why it was dropped (a rising
`signature` count is somebody malformed or hostile; `stale` is clock skew beyond
300 s; `self_heard` is normal — a node hears its own multicast); `trust` on a node
row says what the policy decided and why.

## Threat model

<div class="overflow-x-auto" tabindex="0">

| threat | answer |
|---|---|
| forged advertisement | Ed25519 signature over the advertisement; an unsigned or mis-signed packet is a counted refusal |
| spoofing a known node id | the id is derived from the key by the verifier, never carried; a different key is a different id, and a registry that somehow held the id already rejects the key change |
| replay | per-instance sequence numbers; at-or-below the highest seen is dropped and counted |
| stale/future packets | a ±300 s timestamp window |
| oversized / malformed / wrong-version packets | bounded before parsing; refused by shape and version; the parser never panics (property-tested) |
| flooding the registry | bounded table (256), longest-unseen untrusted evicted first, trusted records never evicted for space — a flood cannot flush your fleet |
| poisoned rendezvous | candidates travel as signed envelopes and verify at the consumer; a rendezvous can withhold, not invent; its failure backs off and stops nothing else |
| a LAN attacker advertising MCP/HTTP endpoints | endpoints are advisory hints, identity is the key, and no endpoint is ever contacted with credentials — there are none to send; connecting to an advertised endpoint yields the same unauthenticated read-only surface as any loopback probe |
| discovery as a foothold | discovery grants nothing: no execution, no authorization, no write surface exists behind it (`project.mesh-is-observation-not-authority`, held by `scripts/ci/mesh-check` and the tests it names) |
| exfiltration via advertisements | the envelope's fields are enumerated and bounded; no secret, path, environment or content field exists; repositories are digests |

</div>


What the mesh does **not** defend against: an attacker on the same segment can see
that a Majordomus exists, its display name, its advertised endpoints and its
repository digests (presence disclosure is inherent to discovery — the off-by-default
declaration exists exactly for environments where that is unacceptable); and it does
not encrypt traffic (there is no TLS in the executable; run rendezvous over an
overlay that encrypts, or stay on multicast within a trusted segment).

## Extending it

A new discovery mechanism — mDNS, a tailnet's API, Consul, a cloud registry —
implements `mesh::provider::MeshProvider` (start, status, raw bytes up the channel)
and is handed to the runtime. That is the entire registration: the manager verifies,
the registry converges, and every surface shows the new source's observations without
a line of change — the test
`a_synthetic_provider_reaches_the_registry_with_zero_registration` holds exactly this.
{% endraw %}
