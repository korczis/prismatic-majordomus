# A discovered node gains no execution and no authorization from being heard; only signed traffic from a linked node may change mesh state, and trust labels records under an explicit policy that defaults to deny_unknown

## What it means

Discovery creates awareness, not authority. Being heard on the network — even with a
valid signature, even trusted by policy — grants a node nothing: there is no remote
execution in this executable to gain. The HTTP surface a node advertises answers its reads
to whoever reaches it, unauthenticated; every capability that changes something — the mesh
operations among them (`mesh.register`, `mesh.claim`, the handover, review and session
operations) — is refused to a remote caller unless its input is signed by a node linked
through the trust policy and a signed handshake (`ExecutionPolicy::admits_remote`). Trust is an
explicit, declared policy: `deny_unknown` (the default — a valid unknown node is
recorded, visible, and trusted for nothing), `allowlist` (declared public keys), or
`tofu` (a development convenience every listing names as such). Whoever builds remote
operations later must bring their own authorization decision; ADR 0050 forecloses
inheriting one from discovery.

## How it works

The policy is data on the mesh declaration; evaluation is one pure function
(`apps/majordomus-cli/src/mesh/trust.rs`) the manager calls after signature
verification and before the registry. Its unit tests prove `deny_unknown` observes
without trusting, the allowlist and TOFU semantics, and that a node id reappearing
under a different key is rejected regardless of policy — with node ids derived from
keys, spoofing an identity is a key-possession problem. The integration test proves a
forged envelope is a counted refusal over a real socket. The rule
`project.mesh-is-observation-not-authority` holds the boundaries, and the `mesh-check`
gate enforces its structural half.

## How to see it

```
majordomus mesh nodes --format json | jq '.nodes[].trust'   # every verdict, labelled
majordomus check --rule project.mesh-is-observation-not-authority
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh::trust
```

`deny_unknown_observes_and_never_trusts` and
`a_key_change_under_one_node_id_is_rejected` are the named unit proofs; the
integration test registers a forged envelope and reads the refusal.

## What it does not cover

It does not authenticate the HTTP surface's reads — they are unauthenticated by the
repository's standing design, on loopback by default, and reachable from the network when
the server is bound beyond it. And it
does not promise future remote operations will be safe; it promises they cannot
inherit authorization from discovery.

## Why it exists

Presence quietly becoming permission is how LAN discovery becomes an attack surface.
Naming the boundary as a blocking rule — and defaulting trust to "nobody" — keeps
"who exists" and "who may do what" as different questions with different answers
(ADR 0050).
