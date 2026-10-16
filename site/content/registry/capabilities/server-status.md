+++
title = "server.status"
description = "Where this checkout's server stands — absent, starting, ready, outdated, skewed or stale — measured against what this executable would serve; the lease this process holds when it is the server; and, unless `checkouts` narrows it to this one, every checkout git registers for the repository, the primary first, each with its lease, its standing and the reason, and the peers its server reports. Each lease carries the share directory its server serves, measured on every call: the version its `version.txt` states, whether it holds `ui/`, and `skewed` with the reason when it states another version than the executable of that server or lacks `ui/` or `cockpit/`, the state in which every asset of those pages answers 404; a server that answers at this executable's version over such a share stands `skewed` rather than `ready`. `checkouts: this` reads one lease and probes one server and enumerates no other checkout, which is what a caller asking only about the checkout it is in should pay. Read from the lease files and the servers on every call; nothing is cached, because the leases are written by other processes."
weight = 184
slug = "server-status"
[extra]
id = "server.status"
source = "apps/majordomus-cli/src/capability/builtin/server.rs"
+++
