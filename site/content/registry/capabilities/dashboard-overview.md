+++
title = "dashboard.overview"
description = "Is it healthy, what changed, what is broken, what needs action — each answered by cards read out of `health.report`, `release.version`, `plan.status`, `worktree.status`, `plan.next`, `continuity.state` and `peers.list` (this checkout's board), asked once each through the executor. Every card carries its source capability, input and RFC 6901 pointer, so asking the source the same question yields the same value; a source that cannot answer makes its cards `unknown`, never `ok`. Nothing here rebuilds canonical state or asks anything beyond this process and its checkout."
weight = 14
slug = "dashboard-overview"
[extra]
id = "dashboard.overview"
source = "apps/majordomus-cli/src/capability/builtin/dashboard.rs"
+++
