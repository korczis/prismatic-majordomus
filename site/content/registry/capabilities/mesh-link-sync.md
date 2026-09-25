+++
title = "mesh.link.sync"
description = "The replication round a linked runtime posts every heartbeat, signed under its link id with a rising counter: its marks and the events this runtime lacks. Ingested — verified end to end, deduplicated, applied in stream order — and answered, signed, with this runtime's marks and the events the peer lacks. An unknown link or a restarted peer is told to say hello again."
weight = 66
slug = "mesh-link-sync"
[extra]
id = "mesh.link.sync"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
