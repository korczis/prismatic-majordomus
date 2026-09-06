+++
title = "peers.announce"
description = "Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller."
weight = 6
slug = "peers-announce"
[extra]
id = "peers.announce"
source = "apps/majordomus-cli/src/capability/builtin/peers.rs"
+++
