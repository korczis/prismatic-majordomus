+++
title = "mesh.link.hello"
description = "The link handshake another runtime posts: a signed hello carrying its runtime card, a fresh nonce and its protocol range. Refused, typed, when malformed, oversized, mis-signed, stale, replayed, of an unsupported protocol, of another repository, untrusted, or this runtime itself; otherwise answered with a signed welcome: this runtime's card, a link id, its marks and the echoed nonce. Changes this process's link table only."
weight = 65
slug = "mesh-link-hello"
[extra]
id = "mesh.link.hello"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
