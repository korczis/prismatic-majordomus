+++
title = "served.observe"
description = "Fetches the build identity a deployment serves (one bounded probe, no shell): for a `deployment` naming an object with a provider, its liveness route at the provider's URL; otherwise `build.json` under the configured base URL. It judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks)."
weight = 193
slug = "served-observe"
[extra]
id = "served.observe"
source = "apps/majordomus-cli/src/capability/builtin/served.rs"
+++
