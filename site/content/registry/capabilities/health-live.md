+++
title = "health.live"
description = "Is this process alive: the cheapest true statement this executable can make about itself, with the version that answered, the checkout it serves (by identity, not path), whether it still holds that checkout's lease and a digest of that lease's token. No filesystem traversal, no index build, no network — this is what a hosting platform polls and what the lease probe asks, and it must cost nothing to say."
weight = 73
slug = "health-live"
[extra]
id = "health.live"
source = "apps/majordomus-cli/src/capability/builtin/health.rs"
+++
