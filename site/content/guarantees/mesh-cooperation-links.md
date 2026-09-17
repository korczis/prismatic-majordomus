+++
title = "Two runtimes of one repository link only through a signed, replay-protected handshake that checks protocol, repository identity and trust, and every refusal is typed and visible"
description = "Being heard on the network is not being linked. A runtime that wants to cooperate"
weight = 182
[extra]
claim_id = "mesh-cooperation-links"
status = "guaranteed"
source = "docs/claims/mesh-cooperation-links.md"
+++
{% raw %}

## What it means

Being heard on the network is not being linked. A runtime that wants to cooperate
presents a signed hello — its key, runtime, instance, repository identity, version,
features, a fresh nonce and the link protocol range it speaks — and the answering runtime
links it only if the signature verifies, the timestamp is fresh, the nonce has not been
used, the protocol ranges meet, the peer is not itself, the repository identity is its own,
and the key is trusted by the declared policy. The dialer applies the same checks to the
signed welcome it gets back. Anything else is a refusal with a code
(`malformed`, `oversized`, `signature`, `stale`, `replay`, `protocol_unsupported`,
`self_link`, `repository_mismatch`, `untrusted`, `capacity`, and for a sync round
`not_active` or `unknown_link`), counted, and listed on
`mesh.cooperation` with its detail — never a silent dropped socket.

## How it works

The handshake and every later sync round are `Signed` messages: canonical JSON under a
per-message signing domain, so a signature over a hello can never be replayed as a sync.
`apps/majordomus-cli/src/mesh/link.rs` defines the messages and the refusal codes;
`apps/majordomus-cli/src/mesh/cooperation.rs` (`accept_hello`, `dial`, `accept_sync`,
`sync_with`) is the one admission path, served as `POST /api/v1/mesh/link/hello` and
`/api/v1/mesh/link/sync` by the mesh capability module. After the handshake, each sync
round carries the link id and a rising counter, so a replayed round is refused too, and a
restarted peer is told to say hello again.

## How to see it

```
majordomus mesh peers                     # linked runtimes and each link's state
curl http://127.0.0.1:8741/api/v1/mesh/cooperation | jq '.peers, .refused'
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation
```

`two_runtimes_link_and_share_sessions_claims_reviews_and_a_handover` links two separate
server processes over TCP; `malformed_forged_and_future_link_messages_are_typed_refusals_over_http`
posts garbage, a forged card and a future protocol range and reads each refusal back.

## What it does not cover

It does not encrypt: the executable has no TLS, so links belong on a private network or an
overlay (a tailnet, WireGuard) that provides confidentiality. It authenticates keys, not
people: whoever holds an allowed node key is that node.

## Why it exists

Discovery tells a runtime that a key exists somewhere. Cooperation lets that key's claims
exclude your work and its handovers land in your checkout, so admission has to prove more:
that the key is answering now, serves your repository, speaks your protocol, and is one you
trust.
{% endraw %}
