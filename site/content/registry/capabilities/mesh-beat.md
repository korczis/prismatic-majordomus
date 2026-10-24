+++
title = "mesh.beat"
description = "Record that a session of this runtime is making progress, with the phase it is in (`waiting on CI run 123`) and optionally the running process it speaks for. A session that has beaten lives by its beats: its exclusive claims are suspended — they refuse nobody — once no beat arrives within `cooperation.session_ttl_seconds`, and hold again at the next. A worker's MCP requests other than `ping` beat for it; this is the way for a long-running process that sends none. A beat within a quarter of the ttl with an unchanged phase writes nothing and answers the last one. Writes this runtime's journal only."
weight = 108
slug = "mesh-beat"
[extra]
id = "mesh.beat"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
