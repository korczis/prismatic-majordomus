+++
title = "peers.list"
description = "Every worker of this repository: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, what it announced, and which checkout it is attached to. A server serves one checkout, so the board of a repository worked on through linked worktrees is gathered: this checkout's board out of memory, every other checkout's from the server its lease names, asked for its own board alone. 'covered' says how many checkouts were covered, 'boards' lists each one with a server, a worker or a board that could not be read (the rest counted in 'omitted'; 'view: full' lists every one), and 'complete' whether every board could be read, so a short board is never mistaken for an empty repository. 'checkouts: this' reads one board and enumerates, probes and asks nothing else. In-memory on every server; gone with the processes."
weight = 150
slug = "peers-list"
[extra]
id = "peers.list"
source = "apps/majordomus-cli/src/capability/builtin/peers.rs"
+++
