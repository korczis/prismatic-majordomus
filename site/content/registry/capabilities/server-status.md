+++
title = "server.status"
description = "Where this checkout's server stands — absent, starting, ready, outdated or stale — measured against what this executable would serve; the lease this process holds when it is the server; and, unless `checkouts` narrows it to this one, every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, and the peers its server reports. `checkouts: this` reads one lease and probes one server and enumerates no other checkout, which is what a caller asking only about the checkout it is in should pay. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes."
weight = 73
slug = "server-status"
[extra]
id = "server.status"
source = "apps/majordomus-cli/src/capability/builtin/server.rs"
+++
