+++
title = "MCP instances from instance registries"
description = "Third-party MCP servers on this machine that announce themselves by writing a record (mcp-instance/v1) to a shared registry directory: listed with whether their owner is alive, refused when a record is malformed or carries a token, never deleted, and probed with an MCP ping only on request and only on loopback."
weight = 29
slug = "instances"
[extra]
id = "instances"
source = "apps/majordomus-cli/src/capability/builtin/instances.rs"
+++
