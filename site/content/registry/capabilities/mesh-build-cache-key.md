+++
title = "mesh.build_cache.key"
description = "Every lockfile the repository tracks — Cargo.lock, mix.lock, package-lock.json — as the thing it makes cacheable (a cargo build directory, mix dependencies, npm node_modules), each with its key: a schema version, the platform (the toolchain's own host triple, or `any` for platform-independent source), the toolchain as its executables state it, and the digest of every input and environment variable that changes what the install leaves. Where the entry is restored to is a separate field, so two checkouts with one lockfile share one entry. A kind whose toolchain is absent says why and has no key (ADR 0157, I3950)."
weight = 105
slug = "mesh-build-cache-key"
[extra]
id = "mesh.build_cache.key"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
