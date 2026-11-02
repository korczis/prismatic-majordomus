+++
title = "server.status"
description = "Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; where its runtime stands (`runtime`: that word refined by the server's readiness report, its mesh verdict and the last recorded transition into building, mesh_attaching, operational, degraded, stopping, stopped or failed, with the time, reason and recorder of that transition, kept under `.ai/local/state/runtime/` and written only when the state changes — this reader records `stopped` or `failed` for a server it finds gone); how far the tree it serves is from the remote trunk (`trunk`: current, lagging, stale, unjudged or unmeasured against the policy's `checkout.trunk_distance`, as of the last fetch); the lease this process holds when it is the server; and, unless `checkouts` narrows it to this one, every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, its runtime, and the peers its server reports. `checkouts: this` reads one lease and probes one server and enumerates no other checkout, which is what a caller asking only about the checkout it is in should pay. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes."
weight = 202
slug = "server-status"
[extra]
id = "server.status"
source = "apps/majordomus-cli/src/capability/builtin/server.rs"
+++
