+++
title = "mesh.build_cache.env"
description = "What the process that starts the sccache server should have in its environment on this machine, derived and never typed: the local disk as the first level and the selected store as the second (SCCACHE_MULTILEVEL_CHAIN=disk,s3, the bucket, the endpoint, the region, whether TLS is used, a key prefix of rustc's host triple, the credential FILE path and its profile) only when sccache has the multi-level chain (measured on a throwaway server, not assumed), a credential file of mode 0600 holding the profile exists, and the store answered its readiness route now; otherwise no variable at all, with the one condition that did not hold. sccache fixes its chain at server start and fails the compile when a level is down then, so a store is added only to a server that is starting. No key is ever in the answer, only the credential file's path (ADR 0157, I2673)."
weight = 118
slug = "mesh-build-cache-env"
[extra]
id = "mesh.build_cache.env"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
