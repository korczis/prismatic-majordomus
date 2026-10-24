+++
title = "executions.protocol"
description = "Where the WebSocket is, how a subscription and a reconnect are expressed, what the server writes, every state an execution may be in with the states it may move to (the transition table the store holds every event to), and the JSON Schema of every message — derived from the Rust types that implement it, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket; this is where that contract lives."
weight = 73
slug = "executions-protocol"
[extra]
id = "executions.protocol"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
