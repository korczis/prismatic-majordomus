+++
title = "peers.announce"
description = "Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. A peer may hold several claims at once: name one with 'claim' and it stands beside the others, announce under that name again and it is updated, leave it out and this is the peer's one unnamed claim. Name your claims when one session is doing several things at once — subagents share their parent's session, so an unnamed announcement from each of them would replace the last rather than adding to it. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller."
weight = 68
slug = "peers-announce"
[extra]
id = "peers.announce"
source = "apps/majordomus-cli/src/capability/builtin/peers.rs"
+++
