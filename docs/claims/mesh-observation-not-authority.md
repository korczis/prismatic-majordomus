# A discovered node — trusted or not — gains no execution, no authorization and no write surface; trust labels records under an explicit policy that defaults to deny_unknown

## What it means

Discovery creates awareness, not authority. Being heard on the network — even with a
valid signature, even trusted by policy — grants a node nothing: there is no remote
execution in this executable to gain, the HTTP surface a node advertises is the same
unauthenticated read-only projection it always served, and the one mutating mesh
operation (`mesh.register`) changes the answering process's memory only. Trust is an
explicit, declared policy: `deny_unknown` (the default — a valid unknown node is
recorded, visible, and trusted for nothing), `allowlist` (declared public keys), or
`tofu` (a development convenience every listing names as such). Whoever builds remote
operations later must bring their own authorization decision; ADR 0043 forecloses
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
