+++
title = "A runtime that stops beating — crashed, killed or partitioned — expires on its peers within the declared expiry, its claims stop excluding, and its restart reconnects as the same runtime without a duplicate"
description = "Presence is current, never \"seen once this morning\". A runtime killed without a shutdown"
weight = 226
[extra]
claim_id = "mesh-liveness-expiry"
status = "guaranteed"
source = "docs/claims/mesh-liveness-expiry.md"
+++
{% raw %}

## What it means

Presence is current, never "seen once this morning". A runtime killed without a shutdown
releases nothing, yet within `cooperation.expiry_seconds` its link is `expired` on every
peer, its sessions are `expired`, and its exclusive claims no longer refuse anyone. When it
restarts in the same checkout with the same node key it is the same runtime — a new
instance of it — and it reconnects: its peers list it once, with `restarts` counted, and
the claims of its previous run stay expired rather than coming back to life.

## How it works

Every heartbeat a runtime raises its own beat; sync rounds relay every stream's beat with
how long ago the sender saw it rise, so liveness is computed on each runtime's own monotonic
clock and no two machines' wall clocks are compared
(`apps/majordomus-cli/src/mesh/journal.rs`). A link with no exchange for half the expiry is
`unreachable`, past the expiry `expired` and dropped; the dialer backs off exponentially,
capped at the expiry, and says hello again when the peer answers
(`apps/majordomus-cli/src/mesh/cooperation.rs`). A reloaded journal restores events but no
freshness, so nothing is alive until it beats again.

A live runtime does not let a stalled worker's exclusive claims refuse anyone (I3990). While a
session works — an MCP request other than `ping`, an announcement, a claim, or `majordomus
mesh beat --phase "waiting on CI"` from a long-running process — its runtime writes a
`session_beat` carrying the session's ttl (`cooperation.session_ttl_seconds`, default 600);
every runtime ages the newest one on its own clock, and an exclusive claim whose session has
not beaten within the ttl (plus the expiry, on a runtime other than the holder's) reads
`suspended`: it refuses nobody until its session beats again. Nothing is closed and nothing is
taken over — the session and its advisory claims are untouched, and fencing a resumed holder
waits for an epoch (I3174). A session that never beat — a command-line claim, a runtime older
than the rule — keeps the stream rule alone.

## How to see it

```
majordomus mesh peers                       # link state and "beat Ns ago" per runtime
curl 'http://127.0.0.1:8741/api/v1/mesh/state?view=full' | jq '.streams'
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation a_killed_runtime
```

`a_killed_runtime_expires_and_its_restart_reconnects_as_the_same_runtime` kills a server
process with SIGKILL and restarts it; `test/mesh-lab/run` (scenarios `process_crash_recovery`
and `network_partition`) kills a container and disconnects one from the network.
`test/cases/3990_a_claim_lapses_when_its_session_stops_progressing.sh` serves a runtime, takes
an exclusive claim over MCP, keeps it with tool calls, then only pings until the claim reads
`suspended` while the runtime beats on, its advisory claim and a command-line claim still hold,
a second exclusive claimer is admitted, and a claim kept by `mesh beat --phase "waiting on CI"`
holds.

## What it does not cover

Expiry is bounded by the declared expiry plus one heartbeat per relay hop; it is not
instantaneous. A runtime that is alive but unreachable from one peer and reachable from
another stays alive through the relay, by design.

## Why it exists

A claim that outlives its holder blocks work forever; a claim that is released by a timeout
on one machine but not another is two truths. Liveness from beats that expire on every
runtime's own clock is what makes a dead holder's claims end everywhere without anyone
releasing them. A holder must stop excluding when its worker stops, not only when its server
does: on 2026-10-22 an executor waiting on nobody held the integration lease for hours while
its server beat, and the owner ordered that no work may stall on a peer that waits on a
dialog, crashed or hung.
{% endraw %}
