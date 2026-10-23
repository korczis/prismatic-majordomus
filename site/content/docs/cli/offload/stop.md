+++
title = "majordomus offload stop"
description = "Stop an offloaded run as a process group on its host: read its run record, refuse unless the record names this session (or --force), then TERM the group and KILL what is left after the grace period. Exit 2 on a tag that is not one or an ambiguous host, 12 when the host holds no record or the group is not the run's, 13 when the run belongs to another session"
weight = 103
[extra]
route = "/docs/cli/offload/stop/"
command = "majordomus offload stop"
source = "apps/majordomus-cli/src/cli.rs"
+++
