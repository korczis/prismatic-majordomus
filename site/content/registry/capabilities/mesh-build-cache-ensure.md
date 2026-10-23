+++
title = "mesh.build_cache.ensure"
description = "For one lockfile the repository tracks (npm node_modules, mix deps, or the cargo build directory), skip an install whose key is already in place; otherwise restore it from this machine's local archive level (~/.cache/majordomus/build-cache) and then from the store this node declares when its own probe finds it healthy and a credential file is present, each only after the archive's SHA-256 matches the digest recorded when it was saved and every entry is judged safe (no absolute name, no `..`, no link leaving the target). On a miss the command runs, and when it succeeds what it left is archived to the local level and the store. A failure of any level is a miss: the command runs exactly as without the cache. npm's command is not run on a hit; mix's is; a cargo build directory is restored only when cold, under cargo's own lock, without incremental state, and never saved above one PUT. A restore and a save respect the build's free-space floor (MAJORDOMUS_MIN_FREE_MB); the local level is held under the policy's build_cache.local_max_gib, oldest first. MAJORDOMUS_BUILD_CACHE=off bypasses every level. Offered on the command line only: it runs a command (ADR 0157, I3952)."
weight = 104
slug = "mesh-build-cache-ensure"
[extra]
id = "mesh.build_cache.ensure"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
