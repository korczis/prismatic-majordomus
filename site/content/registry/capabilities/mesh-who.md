+++
title = "mesh.who"
description = "The session behind a process, a path, an issue or an offloaded run (I3956): a pid (or `host:pid` on a declared offload host, read through its ssh path only) is read where it runs — its parent chain up to the provider's process and its messaging socket, its group, start, command and working directory, and the owner variables of its environment — and joined with that host's run records, the repository's board (whose rows carry each worker's address) and the mesh claims; a run tag is the record its host keeps; an issue or a path is every live mesh claim, announcement and run record naming or covering it. Each owner comes with its board peer, issue, address and the facts that named it. A process whose parent is init and whose directory is gone is marked a leftover and named with its owner when its environment or a run record knows it. Nothing is guessed from a name: an answer that cannot name an owner says unknown and lists the evidence it lacked. Reads processes and records; signals nothing."
weight = 139
slug = "mesh-who"
[extra]
id = "mesh.who"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
