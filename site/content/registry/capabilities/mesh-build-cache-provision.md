+++
title = "mesh.build_cache.provision"
description = "The host layer of a cache provider as one declared policy: a disk guard on a timer, services that restart always and are last for the OOM killer, a bounded journal and container log, a private bucket with expiry and a hard quota beside the CI cache's own bound, one least-privilege identity per node whose credential is generated on the host into a mode 0600 file outside any repository, and the rustc wrapper that makes the node's own builds use the cache. Without `apply` it plans only: every file, edit and command still owed, from what the host holds, with no secret named. With `apply` it writes what is missing or drifted and runs the commands, as root, refusing without it before touching anything; a second apply changes nothing. A `root` other than `/` is a staging directory: files only, no privilege, no command. Offered on the command line only: it installs services and writes credentials, which nothing reachable over HTTP or MCP may do."
weight = 111
slug = "mesh-build-cache-provision"
[extra]
id = "mesh.build_cache.provision"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
