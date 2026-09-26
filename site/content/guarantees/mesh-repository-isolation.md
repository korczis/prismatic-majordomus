+++
title = "Runtimes serving different repositories never form a cooperative link, even when they discover each other"
description = "Two Majordomus servers on the same network, same version, same trusted keys — one serving"
weight = 185
[extra]
claim_id = "mesh-repository-isolation"
status = "guaranteed"
source = "docs/claims/mesh-repository-isolation.md"
+++
{% raw %}

## What it means

Two Majordomus servers on the same network, same version, same trusted keys — one serving
repository X, one serving repository Y — hear each other's advertisements and may list each
other as observed nodes. They never link: each hello is refused as `repository_mismatch`,
the refusal is listed on both sides, and no session, claim, handover or review of one
repository reaches the other. A server of X on another machine does link.

## How it works

A repository's mesh identity is derived from its root commits — content every clone
shares and no unrelated repository has — or declared as `cooperation.repository` in the
mesh declaration, and never from a path, an address or a host name
(`apps/majordomus-cli/src/mesh/repository.rs`). The handshake compares the identities in
both directions, and every event carries its repository identity, so an event of another
repository is refused at ingest even if something relayed it.

## How to see it

```
majordomus mesh doctor                      # the repository check names this runtime's identity
curl http://127.0.0.1:8741/api/v1/mesh/cooperation | jq '.repository, .refused'
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation runtimes_of_different_repositories
```

`two_clones_at_different_paths_are_one_repository_and_another_is_not` proves the identity;
`runtimes_of_different_repositories_never_link_and_say_why` proves the isolation between
two server processes; `test/mesh-lab/run` (scenario `repository_isolation`) proves it
between separate Linux nodes that discovered each other by multicast.

## What it does not cover

A fork shares its origin's root commits and is the same repository to the mesh unless one
side declares its own `cooperation.repository`. A shallow clone has no reliable root and must
declare one; the doctor says so.

## Why it exists

A local network routinely holds several repositories' servers. Cooperation that crossed
repository boundaries by default would let one project's claims block another's work and
hand one project's context to another's sessions.
{% endraw %}
