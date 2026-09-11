+++
title = "episodes.attach"
description = "Open an execution episode for the calling client, or resume the one it already had. 'external_id' is the client's own durable name for the sitting — its conversation or thread id — and must survive a reconnect: a client that comes back under the same identity is given its own episode again, under whatever peer id it now has, rather than a second one. This server's peer id will not do; it is handed out per connection. After this the client's own traffic keeps the episode alive, losing the connection detaches rather than closes it, and it is closed by majordomus_session_detach, by the server stopping, or by the reaper once the reattach grace has passed. The repository's episode is opened by the same command a provider hook runs, and what it reported is in the answer. Needs an MCP session: over plain HTTP there is no connection to bind to."
weight = 26
slug = "episodes-attach"
[extra]
id = "episodes.attach"
source = "apps/majordomus-cli/src/capability/builtin/episodes.rs"
+++
