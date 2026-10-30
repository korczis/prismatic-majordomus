+++
title = "dashboard.cluster"
description = "Who works where and how far each milestone is, as one reading and one rendering: every machine of the mesh with its runtimes, active sessions, claims held and what its sessions say they are doing (`mesh.peers`), the progress of every open milestone (`plan.status`), and the one-line brief with the integration queue, the derive lock and the mesh's links (`brief.session`). It holds no data of its own and names, in `notes`, each source that did not answer. `lines` is the page as console text, cut to the width asked, and is what the command line prints and the /ui panel draws. Composed by the server holding this checkout's lease; the command line asks it. Writes nothing."
weight = 23
slug = "dashboard-cluster"
[extra]
id = "dashboard.cluster"
source = "apps/majordomus-cli/src/capability/builtin/dashboard.rs"
+++
