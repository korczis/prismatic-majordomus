+++
title = "served.observe"
description = "Fetches the build identity the site serves (`build.json` under the configured base URL, one bounded probe, no shell), judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks)."
weight = 97
slug = "served-observe"
[extra]
id = "served.observe"
source = "apps/majordomus-cli/src/capability/builtin/served.rs"
+++
