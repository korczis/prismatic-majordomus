+++
title = "instances.list"
description = "Every record of every registry directory named by the input's registries or MAJORDOMUS_INSTANCE_REGISTRIES (colon-separated): name, kind, owner pid and whether it is alive (live or stale; a stale record is left in place), endpoint, the path of its token file, and the record's other fields. A malformed file, or a record that carries agent_token or token, is refused with the reason and nothing of it is echoed. With probe, each live instance's loopback endpoint is sent an MCP ping with the token read from its token file in memory; a non-loopback endpoint or one that differs from the token file's is not probed. Writes nothing."
weight = 90
slug = "instances-list"
[extra]
id = "instances.list"
source = "apps/majordomus-cli/src/capability/builtin/instances.rs"
+++
